pub const SUGGEST_AFTER_HELP: &str = "\
When to use:
  Get search strategy suggestions based on a task description.
  Useful for LLM integration to determine which commands to run.

Examples:
  commandindexdev suggest --for \"add authentication feature\"
  commandindexdev suggest --for \"fix login bug\" --format json
  commandindexdev suggest --for \"refactor database layer\" --format path";

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use crate::cli::search::SearchContext;
use crate::indexer::knowledge::{DocSubtype, KnowledgeRelation, extract_issue_numbers};
use crate::indexer::reader::IndexReaderWrapper;
use crate::indexer::symbol_store::SymbolStore;
use crate::output::{self, OutputFormat, SuggestResult, SuggestStep};
use crate::search::hybrid::rrf_merge_files;
use crate::search::ranking;
use crate::search::semantic;

/// バイナリ名の定数化（DRY: 一箇所管理）
const BINARY_NAME: &str = "commandindexdev";

/// 入力バリデーションの最大文字数
const MAX_INPUT_LENGTH: usize = 500;

/// BM25検索のデフォルトlimit
const BM25_SEARCH_LIMIT: usize = 20;

/// セマンティックフォールバック時の検索上限
const SEMANTIC_FALLBACK_LIMIT: usize = 20;

/// ファイル単位dedupの上限
const DEDUP_FILE_LIMIT: usize = 5;

/// ナレッジグラフ参照時の最大Issue番号数
const MAX_ISSUE_NUMBERS: usize = 3;

/// ナレッジグラフからのIssue単位最大ドキュメント数
const MAX_KG_DOCS_PER_ISSUE: usize = 4;

/// suggestコマンド用のKGドキュメントDTO
struct SuggestKgDoc {
    issue_number: String,
    file_path: String,
    relation: KnowledgeRelation,
    doc_subtype: DocSubtype,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SuggestError {
    /// 入力バリデーションエラー（空文字、空白のみ、長さ上限超過）
    InvalidInput(String),
    /// インデックス未構築エラー
    IndexNotFound(String),
    /// 検索エラー
    Reader(crate::indexer::reader::ReaderError),
    /// 出力エラー
    Output(crate::output::OutputError),
}

impl fmt::Display for SuggestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            Self::IndexNotFound(msg) => write!(f, "Index not found: {msg}"),
            Self::Reader(e) => write!(f, "Search error: {e}"),
            Self::Output(e) => write!(f, "Output error: {e}"),
        }
    }
}

impl std::error::Error for SuggestError {}

impl From<crate::indexer::reader::ReaderError> for SuggestError {
    fn from(e: crate::indexer::reader::ReaderError) -> Self {
        Self::Reader(e)
    }
}

impl From<crate::output::OutputError> for SuggestError {
    fn from(e: crate::output::OutputError) -> Self {
        Self::Output(e)
    }
}

// ---------------------------------------------------------------------------
// Input validation & sanitization
// ---------------------------------------------------------------------------

/// 入力バリデーション: 空文字、空白のみ、500文字超過、制御文字を拒否
pub(crate) fn validate_input(input: &str) -> Result<String, SuggestError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SuggestError::InvalidInput(
            "task description must not be empty".to_string(),
        ));
    }
    if trimmed.len() > MAX_INPUT_LENGTH {
        return Err(SuggestError::InvalidInput(format!(
            "task description too long (max {MAX_INPUT_LENGTH} characters)"
        )));
    }
    // 制御文字チェック（ASCII 0x00-0x1F, 0x7F）
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(SuggestError::InvalidInput(
            "task description must not contain control characters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// コマンド引数用のシェルクオート（シングルクオートで囲み、内部のシングルクオートをエスケープ）
pub(crate) fn shell_quote(input: &str) -> String {
    // シングルクオートで囲む。内部のシングルクオートは '\'' に置換
    let escaped = input.replace('\'', "'\\''");
    format!("'{escaped}'")
}

// ---------------------------------------------------------------------------
// Strategy building
// ---------------------------------------------------------------------------

/// BM25結果が存在する場合の戦略生成
///
/// `store` が `Some` の場合はセマンティック検索ステップも追加する。
/// `None` の場合はBM25ベースのステップのみ生成する（W2対応）。
fn build_strategy(
    emb_store: Option<&crate::embedding::store::EmbeddingStore>,
    entry_files: &[(String, f32)],
    original_query: &str,
) -> SuggestResult {
    let mut steps = Vec::new();

    // トップファイルに対する context / related / impact ステップ（重複排除: 1回だけ取得）
    if let Some((top_file, _)) = entry_files.first() {
        let quoted_path = shell_quote(top_file);
        steps.push(SuggestStep {
            command: format!("{BINARY_NAME} context -- {quoted_path} --max-files 10"),
            reason: "Get AI context pack for the most relevant file".to_string(),
        });
        steps.push(SuggestStep {
            command: format!(
                "{BINARY_NAME} search --related {quoted_path} --format json --limit 10"
            ),
            reason: "Find files related to the top result".to_string(),
        });
        steps.push(SuggestStep {
            command: format!("{BINARY_NAME} impact -- {quoted_path} --format json"),
            reason: "Analyze impact of changes to the top result".to_string(),
        });
    }

    // semantic search (条件付き) — 元の task description を使う
    let has_embeddings = maybe_add_semantic_step(&mut steps, emb_store, original_query);

    // 追加のエントリーファイルがあれば context を追加
    for (file, _) in entry_files.iter().skip(1).take(2) {
        let quoted_path = shell_quote(file);
        steps.push(SuggestStep {
            command: format!("{BINARY_NAME} context -- {quoted_path} --max-files 5"),
            reason: "Get context for additional relevant file".to_string(),
        });
    }

    SuggestResult {
        query: String::new(), // Will be overwritten by run_suggest
        has_embeddings,
        matched_issues: Vec::new(),
        strategy: steps,
    }
}

/// BM25結果が0件の場合のフォールバック戦略
///
/// `has_embeddings` — embeddings.db 上にデータが存在するか。
/// semantic fallback 失敗時でも、DB にデータがあれば true を維持する。
fn build_fallback_strategy(has_embeddings: bool) -> SuggestResult {
    let steps = vec![
        SuggestStep {
            command: format!("{BINARY_NAME} status --detail"),
            reason: "Check index status and coverage".to_string(),
        },
        SuggestStep {
            command: format!("{BINARY_NAME} search \"<your keywords>\" --format json"),
            reason: "Try a broader keyword search".to_string(),
        },
    ];
    SuggestResult {
        query: String::new(), // Will be overwritten by run_suggest
        has_embeddings,
        matched_issues: Vec::new(),
        strategy: steps,
    }
}

/// Embedding構築済みの場合のみsemantic検索ステップを追加
/// Returns whether embeddings are available.
fn maybe_add_semantic_step(
    steps: &mut Vec<SuggestStep>,
    emb_store: Option<&crate::embedding::store::EmbeddingStore>,
    query: &str,
) -> bool {
    if let Some(store) = emb_store
        && let Ok(count) = store.count()
        && count > 0
    {
        let quoted = shell_quote(query);
        steps.push(SuggestStep {
            command: format!("{BINARY_NAME} search --semantic {quoted} --limit 5"),
            reason: "Semantic search for related documents".to_string(),
        });
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Knowledge graph integration
// ---------------------------------------------------------------------------

/// ナレッジグラフからIssue関連文書を取得する。
/// symbols.db が存在しない場合や、マッチするIssueがない場合は空のVecを返す。
fn query_knowledge_graph(ctx: &SearchContext, issue_numbers: &[String]) -> Vec<SuggestKgDoc> {
    if issue_numbers.is_empty() {
        return Vec::new();
    }

    let db_path = ctx.symbol_db_path();
    if !db_path.exists() {
        return Vec::new();
    }

    // SymbolStore::open() はループ外で1回だけ実行する（DB接続コスト削減）
    let store = match SymbolStore::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[suggest] knowledge graph open failed: {e}");
            return Vec::new();
        }
    };

    let mut all_docs = Vec::new();
    for issue_num in issue_numbers {
        // 個別Issueのエラー時はそのIssueをスキップし、他のIssueの処理を継続する
        match store.find_documents_by_issue(issue_num) {
            Ok(entries) => {
                for entry in entries {
                    all_docs.push(SuggestKgDoc {
                        issue_number: issue_num.clone(),
                        file_path: entry.file_path,
                        relation: entry.relation,
                        doc_subtype: entry.doc_subtype,
                    });
                }
            }
            Err(e) => {
                eprintln!("[suggest] knowledge graph query failed for issue {issue_num}: {e}");
                continue;
            }
        }
    }
    all_docs
}

/// ナレッジグラフドキュメントをフィルタリング・Issue単位制限する。
///
/// 1. modifies / has_progress / has_review(StageReview) を除外
/// 2. relation_priority でソート
/// 3. Issue単位にグルーピングし MAX_KG_DOCS_PER_ISSUE 件に制限
///
/// issue_numbersの順序でIssueをグルーピングすることで、入力順を維持する。
fn filter_and_limit_kg_docs(
    docs: Vec<SuggestKgDoc>,
    issue_numbers: &[String],
) -> Vec<SuggestKgDoc> {
    // Step 1: フィルタリング
    let mut filtered: Vec<SuggestKgDoc> = docs
        .into_iter()
        .filter(|d| match d.relation {
            KnowledgeRelation::Modifies => false,
            KnowledgeRelation::HasProgress => false,
            KnowledgeRelation::HasReview => {
                matches!(
                    d.doc_subtype,
                    DocSubtype::IssueReview | DocSubtype::DesignReview
                )
            }
            KnowledgeRelation::HasDesign | KnowledgeRelation::HasWorkplan => true,
        })
        .collect();

    // Step 2: KnowledgeRelation::priority() でソート（sort_by は安定ソート）
    filtered.sort_by(|a, b| a.relation.priority().cmp(&b.relation.priority()));

    // Step 3: Issue単位グルーピング + 上限制御
    let mut issue_groups: HashMap<String, Vec<SuggestKgDoc>> = HashMap::new();
    for doc in filtered {
        issue_groups
            .entry(doc.issue_number.clone())
            .or_default()
            .push(doc);
    }

    let mut result = Vec::new();
    for issue_num in issue_numbers {
        if let Some(docs) = issue_groups.remove(issue_num) {
            result.extend(docs.into_iter().take(MAX_KG_DOCS_PER_ISSUE));
        }
    }
    result
}

/// ナレッジグラフ結果を戦略ステップとして先頭に挿入する。
fn prepend_knowledge_steps(
    strategy: &mut Vec<SuggestStep>,
    kg_docs: &[SuggestKgDoc],
    matched_issues: &[String],
) {
    let mut kg_steps = Vec::new();
    // Issue番号ごとの issue コマンドステップ
    for issue_num in matched_issues {
        kg_steps.push(SuggestStep {
            command: format!("{BINARY_NAME} issue {issue_num} --format json"),
            reason: format!("Get knowledge graph documents for Issue #{issue_num}"),
        });
    }
    // 各文書の context ステップ
    for doc in kg_docs {
        let quoted_path = shell_quote(&doc.file_path);
        kg_steps.push(SuggestStep {
            command: format!("{BINARY_NAME} context -- {quoted_path} --max-files 5"),
            reason: format!(
                "Get context for Issue #{} related document",
                doc.issue_number
            ),
        });
    }
    // 先頭に挿入
    kg_steps.append(strategy);
    *strategy = kg_steps;
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// suggest サブコマンド実行
pub fn run_suggest(
    for_task: &str,
    format: OutputFormat,
    index_path: Option<&Path>,
) -> Result<(), SuggestError> {
    // 1. 入力バリデーション
    let query = validate_input(for_task)?;

    // 2. インデックス解決（SearchContext::new() を直接使用）
    let base_path =
        std::env::current_dir().map_err(|e| SuggestError::IndexNotFound(e.to_string()))?;
    let ctx = SearchContext::new(&base_path, index_path)
        .map_err(|e| SuggestError::IndexNotFound(e.to_string()))?;

    // 3. リソースオープン（1回のみ）
    let index_dir = ctx.index_dir();
    if !index_dir.exists() {
        return Err(SuggestError::IndexNotFound(
            "Run `commandindex index` first.".to_string(),
        ));
    }
    let reader = IndexReaderWrapper::open(&index_dir)?;

    // EmbeddingStore はオプショナル: DBが存在しない場合もBM25ベースで戦略を返す（W2対応）
    let emb_store = {
        let db_path = ctx.embeddings_db_path();
        if db_path.exists() {
            crate::embedding::store::EmbeddingStore::open(&db_path).ok()
        } else {
            None
        }
    };

    // 4. Issue番号抽出（重複排除・上限制御）
    let issue_numbers: Vec<String> = {
        let nums = extract_issue_numbers(&query);
        let mut seen = HashSet::new();
        nums.into_iter()
            .filter(|n| seen.insert(n.clone()))
            .take(MAX_ISSUE_NUMBERS)
            .collect()
    };

    // 5. ナレッジグラフ参照（Issue番号がある場合のみ）
    let kg_docs = query_knowledge_graph(&ctx, &issue_numbers);

    // 5.5. フィルタリング・Issue単位制限
    let kg_docs = filter_and_limit_kg_docs(kg_docs, &issue_numbers);

    // 6. BM25検索 → ファイル単位dedup
    let bm25_results = reader
        .search(&query, BM25_SEARCH_LIMIT)
        .map_err(SuggestError::Reader)?;
    let bm25_files = ranking::aggregate_by_file(bm25_results, BM25_SEARCH_LIMIT);
    let bm25_files = ranking::apply_file_type_weight(bm25_files, DEDUP_FILE_LIMIT * 3);

    // 7. セマンティック検索を常に試行
    let semantic_files = match semantic::query_semantic(
        &ctx.embeddings_db_path(),
        &ctx.config,
        &query,
        SEMANTIC_FALLBACK_LIMIT,
    ) {
        Ok(Some(results)) => {
            let files = ranking::aggregate_similarity_by_file(results, DEDUP_FILE_LIMIT * 3);
            Some(ranking::apply_file_type_weight(files, DEDUP_FILE_LIMIT * 3))
        }
        Ok(None) => None,
        Err(e) => {
            eprintln!("[suggest] semantic search failed: {e}");
            None
        }
    };

    // 8. 結果統合
    let entry_files = match semantic_files {
        Some(ref sem) if !bm25_files.is_empty() => {
            rrf_merge_files(&bm25_files, sem, DEDUP_FILE_LIMIT)
        }
        Some(mut sem) if bm25_files.is_empty() => {
            sem.truncate(DEDUP_FILE_LIMIT);
            sem
        }
        _ => {
            let mut files = bm25_files;
            files.truncate(DEDUP_FILE_LIMIT);
            files
        }
    };

    // 9. 戦略生成
    let has_embeddings = emb_store
        .as_ref()
        .and_then(|s| s.count().ok())
        .is_some_and(|c| c > 0);

    let mut result = if entry_files.is_empty() {
        build_fallback_strategy(has_embeddings)
    } else {
        build_strategy(emb_store.as_ref(), &entry_files, &query)
    };
    result.query = query;

    // 10. ナレッジグラフステップを戦略先頭に挿入
    prepend_knowledge_steps(&mut result.strategy, &kg_docs, &issue_numbers);
    result.matched_issues = issue_numbers;

    // 11. 出力
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    output::format_suggest_results(&result, format, &mut writer)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_input tests ---

    #[test]
    fn validate_input_empty_string_rejected() {
        let result = validate_input("");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"), "Error should mention empty: {err}");
    }

    #[test]
    fn validate_input_whitespace_only_rejected() {
        let result = validate_input("   ");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"), "Error should mention empty: {err}");
    }

    #[test]
    fn validate_input_too_long_rejected() {
        let long_input = "a".repeat(501);
        let result = validate_input(&long_input);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("too long"),
            "Error should mention too long: {err}"
        );
    }

    #[test]
    fn validate_input_control_chars_rejected() {
        let result = validate_input("hello\x00world");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("control characters"),
            "Error should mention control characters: {err}"
        );
    }

    #[test]
    fn validate_input_normal_string_accepted() {
        let result = validate_input("add authentication feature");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "add authentication feature");
    }

    #[test]
    fn validate_input_trims_whitespace() {
        let result = validate_input("  hello world  ");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn validate_input_max_length_accepted() {
        let input = "a".repeat(500);
        let result = validate_input(&input);
        assert!(result.is_ok());
    }

    // --- shell_quote tests ---

    #[test]
    fn shell_quote_wraps_in_single_quotes() {
        let input = "src/auth.rs";
        let result = shell_quote(input);
        assert_eq!(result, "'src/auth.rs'");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        let input = "it's a test";
        let result = shell_quote(input);
        assert_eq!(result, "'it'\\''s a test'");
    }

    #[test]
    fn shell_quote_handles_shell_metacharacters() {
        let input = "hello\"world;rm -rf /";
        let result = shell_quote(input);
        // All characters preserved but safely quoted
        assert_eq!(result, "'hello\"world;rm -rf /'");
    }

    #[test]
    fn shell_quote_handles_empty_string() {
        let result = shell_quote("");
        assert_eq!(result, "''");
    }

    #[test]
    fn shell_quote_handles_spaces() {
        let input = "a file with spaces.rs";
        let result = shell_quote(input);
        assert_eq!(result, "'a file with spaces.rs'");
    }

    // --- build_fallback_strategy tests ---

    #[test]
    fn fallback_strategy_has_valid_commands() {
        let result = build_fallback_strategy(false);
        assert!(!result.strategy.is_empty(), "Fallback should have steps");
        for step in &result.strategy {
            assert!(
                step.command.starts_with(BINARY_NAME),
                "Command should start with binary name: {}",
                step.command
            );
            assert!(!step.reason.is_empty(), "Each step should have a reason");
        }
    }

    #[test]
    fn fallback_strategy_has_no_embeddings() {
        let result = build_fallback_strategy(false);
        assert!(!result.has_embeddings);
    }

    // --- format tests ---

    #[test]
    fn format_human_output() {
        let result = SuggestResult {
            query: "test query".to_string(),
            has_embeddings: false,
            matched_issues: vec![],
            strategy: vec![
                SuggestStep {
                    command: "cmd1".to_string(),
                    reason: "reason1".to_string(),
                },
                SuggestStep {
                    command: "cmd2".to_string(),
                    reason: "reason2".to_string(),
                },
            ],
        };
        let mut buf = Vec::new();
        output::format_suggest_results(&result, OutputFormat::Human, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Suggested search strategy:"));
        assert!(output.contains("1. cmd1 (reason1)"));
        assert!(output.contains("2. cmd2 (reason2)"));
    }

    #[test]
    fn format_json_output() {
        let result = SuggestResult {
            query: "test query".to_string(),
            has_embeddings: true,
            matched_issues: vec![],
            strategy: vec![SuggestStep {
                command: "cmd1".to_string(),
                reason: "reason1".to_string(),
            }],
        };
        let mut buf = Vec::new();
        output::format_suggest_results(&result, OutputFormat::Json, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["query"], "test query");
        assert_eq!(parsed["has_embeddings"], true);
        assert!(parsed["strategy"].is_array());
        assert_eq!(parsed["strategy"][0]["command"], "cmd1");
    }

    #[test]
    fn format_path_output() {
        let result = SuggestResult {
            query: "test".to_string(),
            has_embeddings: false,
            matched_issues: vec![],
            strategy: vec![
                SuggestStep {
                    command: "cmd1 arg".to_string(),
                    reason: "r1".to_string(),
                },
                SuggestStep {
                    command: "cmd2 arg".to_string(),
                    reason: "r2".to_string(),
                },
            ],
        };
        let mut buf = Vec::new();
        output::format_suggest_results(&result, OutputFormat::Path, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "cmd1 arg");
        assert_eq!(lines[1], "cmd2 arg");
    }

    // --- prepend_knowledge_steps tests ---

    #[test]
    fn test_prepend_knowledge_steps_with_docs() {
        let mut strategy = vec![SuggestStep {
            command: "existing_cmd".to_string(),
            reason: "existing_reason".to_string(),
        }];
        let kg_docs = vec![SuggestKgDoc {
            issue_number: "42".to_string(),
            file_path: "docs/design.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
        }];
        let matched_issues = vec!["42".to_string()];

        prepend_knowledge_steps(&mut strategy, &kg_docs, &matched_issues);

        // Should have 3 steps: issue cmd, context cmd, existing cmd
        assert_eq!(strategy.len(), 3);
        assert!(
            strategy[0].command.contains("issue 42"),
            "First step should be issue command: {}",
            strategy[0].command
        );
        assert!(
            strategy[1].command.contains("context"),
            "Second step should be context command: {}",
            strategy[1].command
        );
        assert_eq!(strategy[2].command, "existing_cmd");
    }

    #[test]
    fn test_prepend_knowledge_steps_empty() {
        let mut strategy = vec![SuggestStep {
            command: "existing_cmd".to_string(),
            reason: "existing_reason".to_string(),
        }];
        let kg_docs: Vec<SuggestKgDoc> = vec![];
        let matched_issues: Vec<String> = vec![];

        prepend_knowledge_steps(&mut strategy, &kg_docs, &matched_issues);

        // Strategy should be unchanged
        assert_eq!(strategy.len(), 1);
        assert_eq!(strategy[0].command, "existing_cmd");
    }

    #[test]
    fn test_prepend_knowledge_steps_multiple_issues() {
        let mut strategy = vec![SuggestStep {
            command: "existing_cmd".to_string(),
            reason: "existing_reason".to_string(),
        }];
        let kg_docs = vec![
            SuggestKgDoc {
                issue_number: "10".to_string(),
                file_path: "docs/design-10.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "20".to_string(),
                file_path: "docs/plan-20.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];
        let matched_issues = vec!["10".to_string(), "20".to_string()];

        prepend_knowledge_steps(&mut strategy, &kg_docs, &matched_issues);

        // 2 issue steps + 2 context steps + 1 existing = 5
        assert_eq!(strategy.len(), 5);
        assert!(strategy[0].command.contains("issue 10"));
        assert!(strategy[1].command.contains("issue 20"));
        assert!(strategy[2].command.contains("context"));
        assert!(strategy[3].command.contains("context"));
        assert_eq!(strategy[4].command, "existing_cmd");
    }

    // --- Issue number extraction tests ---

    #[test]
    fn test_issue_number_dedup() {
        use crate::indexer::knowledge::extract_issue_numbers;

        // Input with duplicate issue numbers
        let query = "Issue #42 and #42 again and #100";
        let nums = extract_issue_numbers(query);
        let mut seen = HashSet::new();
        let unique: Vec<String> = nums
            .into_iter()
            .filter(|n| seen.insert(n.clone()))
            .take(MAX_ISSUE_NUMBERS)
            .collect();

        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0], "42");
        assert_eq!(unique[1], "100");
    }

    #[test]
    fn test_issue_number_max_limit() {
        use crate::indexer::knowledge::extract_issue_numbers;

        // Input with more than MAX_ISSUE_NUMBERS issue numbers
        let query = "Issues #1 #2 #3 #4 #5";
        let nums = extract_issue_numbers(query);
        let mut seen = HashSet::new();
        let unique: Vec<String> = nums
            .into_iter()
            .filter(|n| seen.insert(n.clone()))
            .take(MAX_ISSUE_NUMBERS)
            .collect();

        assert_eq!(
            unique.len(),
            MAX_ISSUE_NUMBERS,
            "Should be limited to {MAX_ISSUE_NUMBERS}"
        );
    }

    // --- matched_issues JSON serialization tests ---

    #[test]
    fn test_matched_issues_json_skip_when_empty() {
        let result = SuggestResult {
            query: "test".to_string(),
            has_embeddings: false,
            matched_issues: vec![],
            strategy: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            parsed.get("matched_issues").is_none(),
            "matched_issues should be omitted when empty, got: {json}"
        );
    }

    #[test]
    fn test_matched_issues_json_present_when_nonempty() {
        let result = SuggestResult {
            query: "test".to_string(),
            has_embeddings: false,
            matched_issues: vec!["42".to_string(), "100".to_string()],
            strategy: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let issues = parsed
            .get("matched_issues")
            .expect("matched_issues should be present");
        assert!(issues.is_array());
        let arr = issues.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "42");
        assert_eq!(arr[1], "100");
    }

    // --- filter_and_limit_kg_docs tests ---

    #[test]
    fn test_filter_removes_modifies() {
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "src/main.rs".to_string(),
                relation: KnowledgeRelation::Modifies,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_path, "design.md");
    }

    #[test]
    fn test_filter_removes_has_progress() {
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "progress.md".to_string(),
                relation: KnowledgeRelation::HasProgress,
                doc_subtype: DocSubtype::ProgressReport,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_path, "design.md");
    }

    #[test]
    fn test_filter_keeps_issue_review_removes_stage_review() {
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "issue-review.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::IssueReview,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design-review.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::DesignReview,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "stage-review.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::StageReview,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert_eq!(result.len(), 2);
        let paths: Vec<&str> = result.iter().map(|d| d.file_path.as_str()).collect();
        assert!(paths.contains(&"issue-review.md"));
        assert!(paths.contains(&"design-review.md"));
        assert!(!paths.contains(&"stage-review.md"));
    }

    #[test]
    fn test_filter_keeps_design_and_workplan() {
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "workplan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_limits_per_issue() {
        // Create 6 docs for one issue, should be limited to MAX_KG_DOCS_PER_ISSUE (4)
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "workplan.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "issue-review.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::IssueReview,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design-review.md".to_string(),
                relation: KnowledgeRelation::HasReview,
                doc_subtype: DocSubtype::DesignReview,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "design2.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "workplan2.md".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                doc_subtype: DocSubtype::WorkPlan,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert_eq!(result.len(), MAX_KG_DOCS_PER_ISSUE);
    }

    #[test]
    fn test_filter_empty_after_all_filtered() {
        let docs = vec![
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "src/main.rs".to_string(),
                relation: KnowledgeRelation::Modifies,
                doc_subtype: DocSubtype::DesignPolicy,
            },
            SuggestKgDoc {
                issue_number: "1".to_string(),
                file_path: "progress.md".to_string(),
                relation: KnowledgeRelation::HasProgress,
                doc_subtype: DocSubtype::ProgressReport,
            },
        ];
        let issues = vec!["1".to_string()];
        let result = filter_and_limit_kg_docs(docs, &issues);
        assert!(result.is_empty());
    }

    #[test]
    fn test_kg_relation_priority_order() {
        assert!(
            KnowledgeRelation::HasDesign.priority() < KnowledgeRelation::HasWorkplan.priority()
        );
        assert!(
            KnowledgeRelation::HasWorkplan.priority() < KnowledgeRelation::HasReview.priority()
        );
        assert!(
            KnowledgeRelation::HasReview.priority() < KnowledgeRelation::HasProgress.priority()
        );
        assert!(KnowledgeRelation::HasProgress.priority() < KnowledgeRelation::Modifies.priority());
    }
}

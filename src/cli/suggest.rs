pub const SUGGEST_AFTER_HELP: &str = "\
When to use:
  Get search strategy suggestions based on a task description.
  Useful for LLM integration to determine which commands to run.

Examples:
  commandindexdev suggest --for \"add authentication feature\"
  commandindexdev suggest --for \"fix login bug\" --format json
  commandindexdev suggest --for \"refactor database layer\" --format path";

use std::fmt;
use std::path::Path;

use crate::cli::search::SearchContext;
use crate::indexer::reader::IndexReaderWrapper;
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

    // 4. BM25検索 → ファイル単位dedup
    let bm25_results = reader
        .search(&query, BM25_SEARCH_LIMIT)
        .map_err(SuggestError::Reader)?;
    let bm25_files = ranking::aggregate_by_file(bm25_results, BM25_SEARCH_LIMIT);
    let bm25_files = ranking::apply_file_type_weight(bm25_files, DEDUP_FILE_LIMIT * 3);

    // 5. セマンティック検索を常に試行
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

    // 6. 結果統合
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

    // 7. 戦略生成
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

    // 8. 出力
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
}

pub const SUGGEST_AFTER_HELP: &str = "\
When to use:
  Get search strategy suggestions based on a task description.
  Useful for LLM integration to determine which commands to run.

Examples:
  commandindexdev suggest --for \"add authentication feature\"
  commandindexdev suggest --for \"fix login bug\" --format json
  commandindexdev suggest --for \"refactor database layer\" --format path";

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::cli::search::SearchContext;
use crate::indexer::reader::{IndexReaderWrapper, SearchResult};
use crate::output::{self, OutputFormat, SuggestResult, SuggestStep};

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

/// テストファイルのスコア係数（BM25スコアに乗算、1.0未満は減衰）
const TEST_FILE_WEIGHT: f32 = 0.3;

/// ドキュメント/レポートファイルのスコア係数
const DOC_FILE_WEIGHT: f32 = 0.5;

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
// BM25 search & dedup
// ---------------------------------------------------------------------------

/// BM25検索を実行してファイルパスのリストを返す
fn search_entry_files(
    reader: &IndexReaderWrapper,
    query: &str,
) -> Result<Vec<(String, f32)>, SuggestError> {
    let results = reader.search(query, BM25_SEARCH_LIMIT)?;
    let deduped = deduplicate_by_file(results, BM25_SEARCH_LIMIT);
    Ok(apply_file_type_weight(deduped, DEDUP_FILE_LIMIT))
}

/// BM25検索結果をファイル単位に正規化・重複排除
fn deduplicate_by_file(results: Vec<SearchResult>, limit: usize) -> Vec<(String, f32)> {
    let mut file_scores: HashMap<String, f32> = HashMap::new();
    for result in results {
        let entry = file_scores.entry(result.path.clone()).or_insert(0.0);
        *entry = entry.max(result.score);
    }
    let mut sorted: Vec<(String, f32)> = file_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
}

// ---------------------------------------------------------------------------
// File type weight
// ---------------------------------------------------------------------------

/// テストファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準（セパレータ付きパターンで誤検知を防止）:
/// - ファイル名が "_test." / ".test." / "_spec." / ".spec." パターンを含む
/// - ファイル名が "test_" で始まる（test_helper等）
/// - パスに "/tests/" または "/__tests__/" ディレクトリを含む
fn is_test_file(lower_path: &str) -> bool {
    let file_name = Path::new(lower_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // セパレータ付きパターンで誤検知防止（contest.rs, latest.rs等を除外）
    file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
        || file_name.starts_with("test_")
        || lower_path.contains("/tests/")
        || lower_path.starts_with("tests/")
        || lower_path.contains("/__tests__/")
        || lower_path.starts_with("__tests__/")
}

/// ドキュメント/レポートファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準:
/// - パスに "dev-reports/" を含む（プロジェクト固有の判定基準）
/// - プロジェクトルート直下の定型ドキュメント（readme.md, changelog.md等）
///
/// 注意: src/配下の.mdファイルはナレッジとして有用なため、一律減衰しない
fn is_doc_file(lower_path: &str) -> bool {
    // プロジェクト固有のレポートディレクトリ
    if lower_path.contains("dev-reports/") {
        return true;
    }

    // .mdファイルのうち、docs/配下またはルート直下の定型ドキュメントのみ
    if lower_path.ends_with(".md") {
        let file_name = Path::new(lower_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        // ルート直下の定型ドキュメント（ディレクトリ区切りがない＝ルート直下）
        let is_root_doc = !lower_path.contains('/')
            && matches!(
                file_name,
                "readme.md" | "changelog.md" | "contributing.md" | "license.md" | "claude.md"
            );
        // docs/ ディレクトリ配下
        let is_docs_dir = lower_path.contains("/docs/") || lower_path.starts_with("docs/");
        return is_root_doc || is_docs_dir;
    }

    false
}

/// ファイルパスからスコア係数を判定する
///
/// - テストファイル: TEST_FILE_WEIGHT (0.3)
/// - ドキュメント/レポート: DOC_FILE_WEIGHT (0.5)
/// - ソースコードファイル: 1.0（調整なし）
fn file_type_weight_factor(path: &str) -> f32 {
    // パス区切り文字を正規化（Windows `\` → `/`）してOS非依存にする
    let lower = path.to_lowercase().replace('\\', "/");
    if is_test_file(&lower) {
        TEST_FILE_WEIGHT
    } else if is_doc_file(&lower) {
        DOC_FILE_WEIGHT
    } else {
        1.0
    }
}

/// BM25スコアにファイル種別ごとの係数を適用し、再ソート・truncateする
fn apply_file_type_weight(files: Vec<(String, f32)>, limit: usize) -> Vec<(String, f32)> {
    let mut weighted: Vec<(String, f32)> = files
        .into_iter()
        .map(|(path, score)| {
            let factor = file_type_weight_factor(&path);
            (path, score * factor)
        })
        .collect();
    weighted.sort_by(|a, b| b.1.total_cmp(&a.1));
    weighted.truncate(limit);
    weighted
}

// ---------------------------------------------------------------------------
// Semantic fallback
// ---------------------------------------------------------------------------

/// BM25が0件の場合にセマンティック検索でファイルを取得する。
///
/// embedding 関連のすべてのエラーは eprintln でログ出力し `None` を返す
/// (graceful degradation)。ログにはクエリ文字列を含めない。
fn try_semantic_fallback(ctx: &SearchContext, query: &str) -> Option<Vec<(String, f32)>> {
    // 1. EmbeddingStore を開く
    let emb_db_path = ctx.embeddings_db_path();
    if !emb_db_path.exists() {
        return None;
    }
    let store = match crate::embedding::store::EmbeddingStore::open(&emb_db_path) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[suggest] semantic fallback: embedding store unavailable");
            return None;
        }
    };

    // 2. embedding が存在するか確認
    match store.count() {
        Ok(0) | Err(_) => return None,
        Ok(_) => {}
    }

    // 3. プロバイダー生成 & クエリ埋め込み生成
    let provider = match crate::embedding::create_provider(&ctx.config.embedding) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[suggest] semantic fallback: embedding provider unavailable");
            return None;
        }
    };

    let query_embeddings = match provider.embed(&[query.to_string()]) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("[suggest] semantic fallback: embedding generation failed");
            return None;
        }
    };

    let query_embedding = query_embeddings.first()?;

    // 4. 類似度検索
    let results = match store.search_similar(query_embedding, SEMANTIC_FALLBACK_LIMIT) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[suggest] semantic fallback: similarity search failed");
            return None;
        }
    };

    if results.is_empty() {
        return None;
    }

    // 5. ファイル単位 dedup（similarity → score として使用）
    let pairs: Vec<(String, f32)> = results
        .into_iter()
        .map(|r| (r.file_path, r.similarity))
        .collect();
    let deduped = deduplicate_by_file_pairs(pairs, DEDUP_FILE_LIMIT);
    Some(deduped)
}

/// (path, score) ペアのリストをファイル単位で重複排除し、上位 limit 件を返す。
fn deduplicate_by_file_pairs(pairs: Vec<(String, f32)>, limit: usize) -> Vec<(String, f32)> {
    let mut file_scores: HashMap<String, f32> = HashMap::new();
    for (path, score) in pairs {
        let entry = file_scores.entry(path).or_insert(0.0);
        *entry = entry.max(score);
    }
    let mut sorted: Vec<(String, f32)> = file_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
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
    let entry_files = search_entry_files(&reader, &query)?;

    // 5. 戦略生成（BM25 0件時はセマンティックフォールバックを試行）
    let has_embeddings = emb_store
        .as_ref()
        .and_then(|s| s.count().ok())
        .is_some_and(|c| c > 0);

    let mut result = if entry_files.is_empty() {
        // BM25 0件 → セマンティック検索にフォールバック
        if let Some(semantic_files) = try_semantic_fallback(&ctx, &query) {
            build_strategy(emb_store.as_ref(), &semantic_files, &query)
        } else {
            build_fallback_strategy(has_embeddings)
        }
    } else {
        build_strategy(emb_store.as_ref(), &entry_files, &query)
    };
    result.query = query;

    // 6. 出力
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

    // --- deduplicate_by_file tests ---

    #[test]
    fn dedup_removes_duplicates_keeps_max_score() {
        let results = vec![
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 1.0,
            },
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 10,
                score: 2.0,
            },
            SearchResult {
                path: "b.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 0.5,
            },
        ];
        let deduped = deduplicate_by_file(results, 10);
        assert_eq!(deduped.len(), 2);
        // a.rs should have max score 2.0 and be first
        assert_eq!(deduped[0].0, "a.rs");
        assert!((deduped[0].1 - 2.0).abs() < f32::EPSILON);
        assert_eq!(deduped[1].0, "b.rs");
    }

    #[test]
    fn dedup_respects_limit() {
        let results = vec![
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 3.0,
            },
            SearchResult {
                path: "b.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 2.0,
            },
            SearchResult {
                path: "c.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 1.0,
            },
        ];
        let deduped = deduplicate_by_file(results, 2);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].0, "a.rs");
        assert_eq!(deduped[1].0, "b.rs");
    }

    #[test]
    fn dedup_empty_input() {
        let deduped = deduplicate_by_file(vec![], 10);
        assert!(deduped.is_empty());
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

    // --- is_test_file tests ---

    #[test]
    fn is_test_file_detects_separator_patterns() {
        assert!(is_test_file("foo_test.ts"));
        assert!(is_test_file("foo.test.ts"));
        assert!(is_test_file("foo_spec.py"));
        assert!(is_test_file("foo.spec.tsx"));
    }

    #[test]
    fn is_test_file_detects_test_prefix() {
        assert!(is_test_file("test_helper.rs"));
        assert!(is_test_file("test_utils.ts"));
    }

    #[test]
    fn is_test_file_detects_tests_directory() {
        assert!(is_test_file("tests/unit/foo.rs"));
        assert!(is_test_file("__tests__/bar.ts"));
    }

    #[test]
    fn is_test_file_ignores_non_test_files() {
        assert!(!is_test_file("src/auth.rs"));
        assert!(!is_test_file("src/contest.rs"));
        assert!(!is_test_file("src/latest.rs"));
    }

    #[test]
    fn is_test_file_empty_path() {
        assert!(!is_test_file(""));
    }

    // --- is_doc_file tests ---

    #[test]
    fn is_doc_file_detects_dev_reports() {
        assert!(is_doc_file("dev-reports/review.json"));
        assert!(is_doc_file("dev-reports/design/policy.md"));
    }

    #[test]
    fn is_doc_file_detects_docs_directory() {
        assert!(is_doc_file("docs/guide.md"));
    }

    #[test]
    fn is_doc_file_detects_root_docs() {
        assert!(is_doc_file("readme.md"));
        assert!(is_doc_file("changelog.md"));
    }

    #[test]
    fn is_doc_file_ignores_nested_root_doc_names() {
        // ルート直下でないreadme.md/changelog.mdは対象外
        assert!(!is_doc_file("src/readme.md"));
        assert!(!is_doc_file("guide/changelog.md"));
    }

    #[test]
    fn is_doc_file_ignores_source_markdown() {
        assert!(!is_doc_file("src/notes.md"));
    }

    #[test]
    fn is_doc_file_ignores_source_files() {
        assert!(!is_doc_file("src/main.rs"));
    }

    // --- file_type_weight_factor tests ---

    #[test]
    fn file_type_weight_factor_values() {
        assert!(
            (file_type_weight_factor("src/foo_test.ts") - TEST_FILE_WEIGHT).abs() < f32::EPSILON
        );
        assert!(
            (file_type_weight_factor("dev-reports/review.json") - DOC_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
        assert!((file_type_weight_factor("src/main.rs") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn file_type_weight_factor_normalizes_windows_paths() {
        // Windows形式のパス区切りでもテストファイルとして判定される
        assert!(
            (file_type_weight_factor("tests\\unit\\foo.rs") - TEST_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
        assert!(
            (file_type_weight_factor("dev-reports\\review.json") - DOC_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
    }

    // --- apply_file_type_weight tests ---

    #[test]
    fn apply_file_type_weight_reorders() {
        let input = vec![
            ("src/foo_test.ts".to_string(), 2.0),
            ("src/main.rs".to_string(), 1.5),
        ];
        let result = apply_file_type_weight(input, 10);
        assert_eq!(result[0].0, "src/main.rs");
        assert_eq!(result[1].0, "src/foo_test.ts");
    }

    #[test]
    fn apply_file_type_weight_truncates() {
        let input = vec![
            ("a.rs".to_string(), 3.0),
            ("b.rs".to_string(), 2.0),
            ("c.rs".to_string(), 1.0),
        ];
        let result = apply_file_type_weight(input, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn apply_file_type_weight_empty_input() {
        let result = apply_file_type_weight(vec![], 10);
        assert!(result.is_empty());
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

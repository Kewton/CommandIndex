pub const DIFF_AFTER_HELP: &str = "\
When to use:
  Detect potential conflicts or overlapping concerns between two files.
  Useful for code review and refactoring planning.

Examples:
  commandindexdev diff src/auth.rs src/session.rs --format json
  commandindexdev diff file_a.md file_b.md
  commandindexdev diff src/a.rs src/b.rs --limit 50";

use std::collections::HashSet;
use std::path::Path;

use crate::cli::search::SearchError;
use crate::indexer::reader::IndexReaderWrapper;
use crate::indexer::symbol_store::SymbolStore;
use crate::output::{self, DiffResult, OutputFormat};
use crate::search::related::{RelatedSearchEngine, normalize_path};

pub fn run_diff(
    files: &[String],
    limit: usize,
    format: OutputFormat,
    index_path: Option<&std::path::Path>,
) -> Result<(), SearchError> {
    // 1. バリデーション
    if files.len() != 2 {
        return Err(SearchError::InvalidArgument(
            "Exactly two files are required for diff".to_string(),
        ));
    }
    let file_a = &files[0];
    let file_b = &files[1];

    // パスバリデーション: 絶対パス拒否、.. 含有拒否（context.rs パターン踏襲）
    for file in [file_a, file_b] {
        if file.starts_with('/') {
            return Err(SearchError::InvalidArgument(format!(
                "Absolute paths are not supported: {file}"
            )));
        }
        if file.contains("..") {
            return Err(SearchError::InvalidArgument(format!(
                "Path traversal is not allowed: {file}"
            )));
        }
    }

    // normalize_path() で正規化後に同一ファイルチェック
    let norm_a = normalize_path(file_a)?;
    let norm_b = normalize_path(file_b)?;
    if norm_a == norm_b {
        return Err(SearchError::InvalidArgument(format!(
            "Cannot diff a file with itself: {norm_a}"
        )));
    }

    // 2. インデックス存在チェック（resolve_index_path で設定ファイル・CLIオプション対応）
    let config = crate::config::load_config(Path::new(".")).ok();
    let config_index_path = config.as_ref().and_then(|c| c.index.path.as_deref());
    let commandindex_dir =
        crate::indexer::resolve_index_path(index_path, config_index_path, Path::new("."))
            .unwrap_or_else(|_| Path::new(".").join(crate::INDEX_DIR_NAME));
    let tantivy_dir = crate::indexer::index_dir(&commandindex_dir);
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let db_path = crate::indexer::symbol_db_path(&commandindex_dir);
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    // 3. reader/store 共有（context.rs L92 パターン踏襲）
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;
    let engine = RelatedSearchEngine::new(&reader, &store);

    // 4. 各ファイルの関連検索（エラー時は即終了）
    let results_a = engine.find_related(file_a, limit).map_err(|e| {
        SearchError::InvalidArgument(format!("Error analyzing '{}': {}", file_a, e))
    })?;
    let results_b = engine.find_related(file_b, limit).map_err(|e| {
        SearchError::InvalidArgument(format!("Error analyzing '{}': {}", file_b, e))
    })?;

    // 5. 集合演算（標準ライブラリの sort() を使用、itertools 不要）
    let paths_a: HashSet<String> = results_a.iter().map(|r| r.file_path.clone()).collect();
    let paths_b: HashSet<String> = results_b.iter().map(|r| r.file_path.clone()).collect();

    let mut overlap: Vec<String> = paths_a.intersection(&paths_b).cloned().collect();
    overlap.sort();
    let mut only_a: Vec<String> = paths_a.difference(&paths_b).cloned().collect();
    only_a.sort();
    let mut only_b: Vec<String> = paths_b.difference(&paths_a).cloned().collect();
    only_b.sort();

    // 6. 出力
    let diff_result = DiffResult {
        file_a: norm_a,
        file_b: norm_b,
        only_a,
        only_b,
        overlap,
    };

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_diff_results(&diff_result, format, &mut handle)?;
    Ok(())
}

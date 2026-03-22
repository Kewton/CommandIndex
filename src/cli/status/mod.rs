pub mod git_info;

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;
use walkdir::WalkDir;

use crate::embedding::store::EmbeddingStore;
use crate::indexer::manifest::{FileType, Manifest};
use crate::indexer::state::{IndexState, StateError};
use crate::indexer::symbol_store::SymbolStore;
use crate::output::strip_control_chars;
use crate::parser::ignore::IgnoreFilter;

use self::git_info::StalenessInfo;

/// status コマンドの出力フォーマット
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFormat {
    Human,
    Json,
}

/// status コマンドのオプション
#[derive(Debug, Clone)]
pub struct StatusOptions {
    pub detail: bool,
    pub coverage: bool,
    pub format: StatusFormat,
    pub verify: bool,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            detail: false,
            coverage: false,
            format: StatusFormat::Human,
            verify: false,
        }
    }
}

/// status コマンドのエラー型
#[derive(Debug)]
pub enum StatusError {
    State(StateError),
    NotInitialized,
    DirectoryNotFound(PathBuf),
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusError::State(e) => write!(f, "{e}"),
            StatusError::NotInitialized => {
                write!(f, "Index not initialized. Run `commandindex index` first.")
            }
            StatusError::DirectoryNotFound(p) => {
                write!(f, "Directory not found: {}", p.display())
            }
        }
    }
}

impl std::error::Error for StatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StatusError::State(e) => Some(e),
            _ => None,
        }
    }
}

impl From<StateError> for StatusError {
    fn from(e: StateError) -> Self {
        StatusError::State(e)
    }
}

/// ファイルタイプ別カウント
#[derive(Debug, Serialize, Default)]
pub struct FileTypeCounts {
    pub markdown: u64,
    pub typescript: u64,
    pub python: u64,
}

/// カバレッジ情報
#[derive(Debug, Serialize)]
pub struct CoverageInfo {
    pub discoverable_files: u64,
    pub indexed_files: u64,
    pub skipped_files: u64,
    pub embedding_file_count: u64,
    pub embedding_model: Option<String>,
}

/// ストレージ内訳
#[derive(Debug, Serialize)]
pub struct StorageBreakdown {
    pub tantivy_bytes: u64,
    pub symbols_db_bytes: u64,
    pub embeddings_db_bytes: u64,
    pub other_bytes: u64,
    pub total_bytes: u64,
}

/// status コマンドの出力情報
#[derive(Debug, Serialize)]
pub struct StatusInfo {
    #[serde(flatten)]
    pub state: IndexState,
    pub index_size_bytes: u64,
    pub file_type_counts: FileTypeCounts,
    pub symbol_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<CoverageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staleness: Option<StalenessInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageBreakdown>,
}

/// ディレクトリサイズを再帰的に計算する
///
/// エラーが発生したエントリはスキップし、取得可能なファイルサイズの合計を返す。
pub fn compute_dir_size(dir: &Path) -> u64 {
    WalkDir::new(dir)
        .follow_links(false)
        .max_depth(10)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// バイト数を人間が読みやすい形式にフォーマットする
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * 1_024;
    const GB: u64 = 1_024 * 1_024 * 1_024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Manifest からファイルタイプ別のカウントを集計する
fn count_file_types(commandindex_dir: &Path) -> FileTypeCounts {
    let manifest = match Manifest::load_or_default(commandindex_dir) {
        Ok(m) => m,
        Err(_) => return FileTypeCounts::default(),
    };

    let mut counts = FileTypeCounts::default();
    for entry in &manifest.files {
        match entry.file_type {
            FileType::Markdown => counts.markdown += 1,
            FileType::TypeScript => counts.typescript += 1,
            FileType::Python => counts.python += 1,
        }
    }
    counts
}

/// SymbolStore からシンボル数を取得する（DB が存在しない場合は 0）
fn get_symbol_count(base_path: &Path) -> u64 {
    let db_path = crate::indexer::symbol_db_path(base_path);
    if !db_path.exists() {
        return 0;
    }
    match SymbolStore::open(&db_path) {
        Ok(store) => store.count_all().unwrap_or(0),
        Err(crate::indexer::symbol_store::SymbolStoreError::SchemaVersionMismatch { .. }) => {
            eprintln!(
                "Warning: Symbol database schema version mismatch. Run `commandindex clean` then `commandindex index` to rebuild."
            );
            0
        }
        Err(_) => 0,
    }
}

/// 発見可能なファイル数をカウントする（walkdir + デフォルト除外 + .cmindexignore）
fn count_discoverable_files(base_path: &Path) -> u64 {
    let ignore_file = base_path.join(".cmindexignore");
    let ignore_filter = IgnoreFilter::from_file(&ignore_file).unwrap_or_default();

    WalkDir::new(base_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            let path = entry.path();
            // Skip hidden directories and .commandindex
            !path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        })
        .filter(|entry| {
            let rel_path = entry.path().strip_prefix(base_path).unwrap_or(entry.path());
            !ignore_filter.is_ignored(rel_path)
        })
        .filter(|entry| {
            matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("md" | "ts" | "tsx" | "py")
            )
        })
        .count() as u64
}

/// EmbeddingStore からユニークファイル数を取得する（DB が存在しない場合は 0）
fn get_embedding_file_count(base_path: &Path) -> u64 {
    let db_path = crate::indexer::embeddings_db_path(base_path);
    if !db_path.exists() {
        return 0;
    }
    match EmbeddingStore::open(&db_path) {
        Ok(store) => store.count_distinct_files().unwrap_or(0),
        Err(_) => 0,
    }
}

/// 設定から embedding モデル名を取得する
fn get_embedding_model(_commandindex_dir: &Path) -> Option<String> {
    match crate::config::load_config(Path::new(".")) {
        Ok(config) => Some(config.embedding.model),
        Err(_) => None,
    }
}

/// CoverageInfo を収集する
fn collect_coverage_info(
    base_path: &Path,
    commandindex_dir: &Path,
    state: &IndexState,
) -> CoverageInfo {
    let discoverable_files = count_discoverable_files(base_path);
    let indexed_files = state.total_files;
    let skipped_files = discoverable_files.saturating_sub(indexed_files);
    let embedding_file_count = get_embedding_file_count(base_path);
    let embedding_model = get_embedding_model(commandindex_dir);

    CoverageInfo {
        discoverable_files,
        indexed_files,
        skipped_files,
        embedding_file_count,
        embedding_model,
    }
}

/// ファイルサイズを取得する（存在しない場合は 0）
fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// StorageBreakdown を計算する
fn compute_storage_breakdown(base_path: &Path) -> StorageBreakdown {
    let tantivy_bytes = compute_dir_size(&crate::indexer::index_dir(base_path));
    let symbols_db_bytes = file_size(&crate::indexer::symbol_db_path(base_path));
    let embeddings_db_bytes = file_size(&crate::indexer::embeddings_db_path(base_path));
    let total_bytes = compute_dir_size(&crate::indexer::commandindex_dir(base_path));
    let other_bytes =
        total_bytes.saturating_sub(tantivy_bytes + symbols_db_bytes + embeddings_db_bytes);

    StorageBreakdown {
        tantivy_bytes,
        symbols_db_bytes,
        embeddings_db_bytes,
        other_bytes,
        total_bytes,
    }
}

/// status コマンドのメインロジック
pub fn run(
    path: &Path,
    options: &StatusOptions,
    writer: &mut dyn Write,
) -> Result<(), StatusError> {
    if !path.is_dir() {
        return Err(StatusError::DirectoryNotFound(path.to_path_buf()));
    }

    let commandindex_dir = path.join(".commandindex");

    if !IndexState::exists(&commandindex_dir) {
        return Err(StatusError::NotInitialized);
    }

    let state = IndexState::load(&commandindex_dir)?;
    state.check_schema_version()?;

    let index_size_bytes = compute_dir_size(&commandindex_dir);
    let file_type_counts = count_file_types(&commandindex_dir);
    let symbol_count = get_symbol_count(path);

    // Collect extended info based on options
    let coverage = if options.detail || options.coverage {
        Some(collect_coverage_info(path, &commandindex_dir, &state))
    } else {
        None
    };

    let staleness = if options.detail {
        git_info::get_staleness_info(path, state.last_commit_hash.as_deref())
    } else {
        None
    };

    let storage = if options.detail {
        Some(compute_storage_breakdown(path))
    } else {
        None
    };

    let info = StatusInfo {
        state,
        index_size_bytes,
        file_type_counts,
        symbol_count,
        coverage,
        staleness,
        storage,
    };

    // Verify mode
    if options.verify {
        let verify_result = run_verify(path, &commandindex_dir);
        match options.format {
            StatusFormat::Human => {
                writeln!(writer).ok();
                writeln!(writer, "Index Verification").ok();
                writeln!(
                    writer,
                    "  State:     {}",
                    if verify_result.state_valid { "OK" } else { "FAIL" }
                )
                .ok();
                writeln!(
                    writer,
                    "  Tantivy:   {}",
                    if verify_result.tantivy_valid {
                        "OK"
                    } else {
                        "FAIL"
                    }
                )
                .ok();
                writeln!(
                    writer,
                    "  Manifest:  {}",
                    if verify_result.manifest_valid {
                        "OK"
                    } else {
                        "FAIL"
                    }
                )
                .ok();
                writeln!(
                    writer,
                    "  Symbols:   {}",
                    if verify_result.symbols_valid {
                        "OK"
                    } else {
                        "FAIL"
                    }
                )
                .ok();
                for issue in &verify_result.issues {
                    writeln!(
                        writer,
                        "  [{:?}] {}: {}",
                        issue.severity, issue.component, issue.message
                    )
                    .ok();
                }
            }
            StatusFormat::Json => {
                let json = serde_json::to_string_pretty(&verify_result)
                    .map_err(|e| StatusError::State(StateError::Json(e)))?;
                writeln!(writer, "{json}").ok();
            }
        }
        return Ok(());
    }

    match options.format {
        StatusFormat::Human => {
            // Basic info (always displayed)
            let index_root = strip_control_chars(&info.state.index_root.display().to_string());
            writeln!(writer, "CommandIndex Status").ok();
            writeln!(writer, "  Index root:    {index_root}").ok();
            writeln!(writer, "  Version:       {}", info.state.version).ok();
            writeln!(writer, "  Created:       {} UTC", info.state.created_at).ok();
            writeln!(
                writer,
                "  Last updated:  {} UTC",
                info.state.last_updated_at
            )
            .ok();
            writeln!(writer, "  Total files:   {}", info.state.total_files).ok();
            writeln!(writer, "  Total sections: {}", info.state.total_sections).ok();
            writeln!(
                writer,
                "  Files by type: Markdown={}, TypeScript={}, Python={}",
                info.file_type_counts.markdown,
                info.file_type_counts.typescript,
                info.file_type_counts.python
            )
            .ok();
            writeln!(writer, "  Symbols:       {}", info.symbol_count).ok();
            writeln!(
                writer,
                "  Index size:    {}",
                format_size(info.index_size_bytes)
            )
            .ok();

            // Coverage section
            if let Some(ref cov) = info.coverage {
                writeln!(writer).ok();
                writeln!(writer, "Coverage").ok();
                writeln!(writer, "  Discoverable files:  {}", cov.discoverable_files).ok();
                writeln!(writer, "  Indexed files:       {}", cov.indexed_files).ok();
                writeln!(writer, "  Skipped files:       {}", cov.skipped_files).ok();
                writeln!(
                    writer,
                    "  Embedding files:     {}",
                    cov.embedding_file_count
                )
                .ok();
                let model_display = cov
                    .embedding_model
                    .as_deref()
                    .map(strip_control_chars)
                    .unwrap_or_else(|| "(not configured)".to_string());
                writeln!(writer, "  Embedding model:     {model_display}").ok();
            }

            // Staleness section (detail only)
            if let Some(ref stale) = info.staleness {
                writeln!(writer).ok();
                writeln!(writer, "Staleness").ok();
                if let Some(ref hash) = stale.last_commit_hash {
                    writeln!(
                        writer,
                        "  Last indexed commit: {}",
                        strip_control_chars(hash)
                    )
                    .ok();
                }
                if let Some(commits) = stale.commits_since_index {
                    writeln!(writer, "  Commits since index: {commits}").ok();
                }
                if let Some(files) = stale.files_changed_since_index {
                    writeln!(writer, "  Files changed:       {files}").ok();
                }
                if let Some(ref rec) = stale.recommendation {
                    writeln!(
                        writer,
                        "  Recommendation:      {}",
                        strip_control_chars(rec)
                    )
                    .ok();
                }
            }

            // Storage section (detail only)
            if let Some(ref stor) = info.storage {
                writeln!(writer).ok();
                writeln!(writer, "Storage").ok();
                writeln!(
                    writer,
                    "  Tantivy index:   {}",
                    format_size(stor.tantivy_bytes)
                )
                .ok();
                writeln!(
                    writer,
                    "  Symbols DB:      {}",
                    format_size(stor.symbols_db_bytes)
                )
                .ok();
                writeln!(
                    writer,
                    "  Embeddings DB:   {}",
                    format_size(stor.embeddings_db_bytes)
                )
                .ok();
                writeln!(
                    writer,
                    "  Other:           {}",
                    format_size(stor.other_bytes)
                )
                .ok();
                writeln!(
                    writer,
                    "  Total:           {}",
                    format_size(stor.total_bytes)
                )
                .ok();
            }
        }
        StatusFormat::Json => {
            let json = serde_json::to_string_pretty(&info)
                .map_err(|e| StatusError::State(StateError::Json(e)))?;
            writeln!(writer, "{json}").ok();
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Verify
// ---------------------------------------------------------------------------

/// 整合性チェック結果
#[derive(Debug, Serialize)]
pub struct VerifyResult {
    pub state_valid: bool,
    pub tantivy_valid: bool,
    pub manifest_valid: bool,
    pub symbols_valid: bool,
    pub issues: Vec<VerifyIssue>,
}

/// 個別のチェック結果
#[derive(Debug, Serialize)]
pub struct VerifyIssue {
    pub component: String,
    pub severity: VerifySeverity,
    pub message: String,
}

/// 重要度
#[derive(Debug, Serialize)]
pub enum VerifySeverity {
    Error,
    Warning,
}

/// インデックスの整合性をチェックする
fn run_verify(base_path: &Path, commandindex_dir: &Path) -> VerifyResult {
    let mut issues = Vec::new();

    // 1. state.json
    let state_valid = match IndexState::load(commandindex_dir) {
        Ok(state) => match state.check_schema_version() {
            Ok(()) => true,
            Err(e) => {
                issues.push(VerifyIssue {
                    component: "state".to_string(),
                    severity: VerifySeverity::Error,
                    message: format!("Schema version mismatch: {e}"),
                });
                false
            }
        },
        Err(e) => {
            issues.push(VerifyIssue {
                component: "state".to_string(),
                severity: VerifySeverity::Error,
                message: format!("Failed to load: {e}"),
            });
            false
        }
    };

    // 2. tantivy
    let tantivy_dir = crate::indexer::index_dir(base_path);
    let tantivy_valid = if tantivy_dir.exists() {
        match tantivy::Index::open_in_dir(&tantivy_dir) {
            Ok(_) => true,
            Err(e) => {
                issues.push(VerifyIssue {
                    component: "tantivy".to_string(),
                    severity: VerifySeverity::Error,
                    message: format!("Failed to open: {e}"),
                });
                false
            }
        }
    } else {
        issues.push(VerifyIssue {
            component: "tantivy".to_string(),
            severity: VerifySeverity::Error,
            message: "Directory not found".to_string(),
        });
        false
    };

    // 3. manifest.json
    let manifest_valid = match Manifest::load(commandindex_dir) {
        Ok(manifest) => {
            let mut valid = true;
            for entry in &manifest.files {
                let file_path = base_path.join(&entry.path);
                if !file_path.exists() {
                    issues.push(VerifyIssue {
                        component: "manifest".to_string(),
                        severity: VerifySeverity::Warning,
                        message: format!("File not found: {}", entry.path),
                    });
                    valid = false;
                }
            }
            valid
        }
        Err(e) => {
            issues.push(VerifyIssue {
                component: "manifest".to_string(),
                severity: VerifySeverity::Error,
                message: format!("Failed to load: {e}"),
            });
            false
        }
    };

    // 4. symbols.db
    let db_path = crate::indexer::symbol_db_path(base_path);
    let symbols_valid = if db_path.exists() {
        match SymbolStore::open(&db_path) {
            Ok(_) => true,
            Err(e) => {
                issues.push(VerifyIssue {
                    component: "symbols".to_string(),
                    severity: VerifySeverity::Error,
                    message: format!("Failed to open: {e}"),
                });
                false
            }
        }
    } else {
        issues.push(VerifyIssue {
            component: "symbols".to_string(),
            severity: VerifySeverity::Warning,
            message: "Database not found".to_string(),
        });
        true // Not having symbols.db is not critical
    };

    VerifyResult {
        state_valid,
        tantivy_valid,
        manifest_valid,
        symbols_valid,
        issues,
    }
}

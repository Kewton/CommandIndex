use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;
use walkdir::WalkDir;

use crate::indexer::manifest::{FileType, Manifest};
use crate::indexer::state::{IndexState, StateError};
use crate::indexer::symbol_store::SymbolStore;
use crate::output::strip_control_chars;

/// status コマンドの出力フォーマット
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFormat {
    Human,
    Json,
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

/// status コマンドの出力情報
#[derive(Debug, Serialize)]
pub struct StatusInfo {
    #[serde(flatten)]
    pub state: IndexState,
    pub index_size_bytes: u64,
    pub file_type_counts: FileTypeCounts,
    pub symbol_count: u64,
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
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

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

/// status コマンドのメインロジック
pub fn run(path: &Path, format: StatusFormat, writer: &mut dyn Write) -> Result<(), StatusError> {
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

    let info = StatusInfo {
        state,
        index_size_bytes,
        file_type_counts,
        symbol_count,
    };

    match format {
        StatusFormat::Human => {
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
        }
        StatusFormat::Json => {
            let json = serde_json::to_string_pretty(&info)
                .map_err(|e| StatusError::State(StateError::Json(e)))?;
            writeln!(writer, "{json}").ok();
        }
    }

    Ok(())
}

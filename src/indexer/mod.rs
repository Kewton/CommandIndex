pub mod diff;
pub mod manifest;
pub mod reader;
pub mod schema;
pub mod state;
pub mod symbol_store;
pub mod writer;

use std::path::{Path, PathBuf};

const COMMANDINDEX_DIR: &str = ".commandindex";
const TANTIVY_DIR: &str = "tantivy";
const SYMBOLS_DB_FILE: &str = "symbols.db";

/// 対象ファイル拡張子（Phase 1: Markdown のみ）
// TODO: Phase 2 で cli/index.rs のハードコードをこの定数に統合すること
pub const SUPPORTED_EXTENSIONS: &[&str] = &["md"];

/// `.commandindex/tantivy` ディレクトリのパスを返す
pub fn index_dir(base_path: &Path) -> PathBuf {
    base_path.join(COMMANDINDEX_DIR).join(TANTIVY_DIR)
}

/// `.commandindex` ディレクトリのパスを返す
pub fn commandindex_dir(base_path: &Path) -> PathBuf {
    base_path.join(COMMANDINDEX_DIR)
}

/// `.commandindex/symbols.db` のパスを返す
pub fn symbol_db_path(base_path: &Path) -> PathBuf {
    commandindex_dir(base_path).join(SYMBOLS_DB_FILE)
}

pub mod manifest;
pub mod reader;
pub mod schema;
pub mod state;
pub mod writer;

use std::path::{Path, PathBuf};

const COMMANDINDEX_DIR: &str = ".commandindex";
const TANTIVY_DIR: &str = "tantivy";

/// `.commandindex/tantivy` ディレクトリのパスを返す
pub fn index_dir(base_path: &Path) -> PathBuf {
    base_path.join(COMMANDINDEX_DIR).join(TANTIVY_DIR)
}

/// `.commandindex` ディレクトリのパスを返す
pub fn commandindex_dir(base_path: &Path) -> PathBuf {
    base_path.join(COMMANDINDEX_DIR)
}

pub mod diff;
pub mod manifest;
pub mod reader;
pub mod schema;
pub mod snapshot;
pub mod state;
pub mod symbol_store;
pub mod writer;

use std::fmt;
use std::path::{Path, PathBuf};

const TANTIVY_DIR: &str = "tantivy";
const SYMBOLS_DB_FILE: &str = "symbols.db";
const EMBEDDINGS_DB_FILE: &str = "embeddings.db";

// ---------------------------------------------------------------------------
// ResolveIndexPathError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ResolveIndexPathError {
    CurrentDirUnavailable(std::io::Error),
    CanonicalizeFailed(PathBuf, std::io::Error),
    PathTraversal(PathBuf),
    SymlinkDetected(PathBuf),
}

impl fmt::Display for ResolveIndexPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirUnavailable(e) => {
                write!(f, "Cannot determine current directory: {e}")
            }
            Self::CanonicalizeFailed(p, e) => {
                write!(f, "Cannot canonicalize path {}: {e}", p.display())
            }
            Self::PathTraversal(p) => {
                write!(f, "Path traversal detected in index path: {}", p.display())
            }
            Self::SymlinkDetected(p) => {
                write!(f, "Symlink detected at index path: {}", p.display())
            }
        }
    }
}

impl std::error::Error for ResolveIndexPathError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirUnavailable(e) | Self::CanonicalizeFailed(_, e) => Some(e),
            Self::PathTraversal(_) | Self::SymlinkDetected(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_index_path
// ---------------------------------------------------------------------------

/// インデックスパスを解決する
/// 優先順位: CLI --index-path > config [index].path > デフォルト {base_path}/.commandindex/
pub fn resolve_index_path(
    cli_index_path: Option<&Path>,
    config_index_path: Option<&str>,
    base_path: &Path,
) -> Result<PathBuf, ResolveIndexPathError> {
    let raw_path = if let Some(cli_path) = cli_index_path {
        // CLI 指定: cwd 基準で解決
        if cli_path.is_relative() {
            std::env::current_dir()
                .map_err(ResolveIndexPathError::CurrentDirUnavailable)?
                .join(cli_path)
        } else {
            cli_path.to_path_buf()
        }
    } else if let Some(config_path) = config_index_path {
        // config 指定: リポジトリルート基準で解決
        let p = Path::new(config_path);
        if p.is_relative() {
            base_path.join(p)
        } else {
            p.to_path_buf()
        }
    } else {
        // デフォルト
        base_path.join(crate::INDEX_DIR_NAME)
    };

    // パストラバーサル検出: ".." を含む未作成パスは明示的エラー
    if !raw_path.exists()
        && raw_path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(ResolveIndexPathError::PathTraversal(raw_path));
    }

    // パス正規化: 存在するパスは canonicalize
    if raw_path.exists() {
        std::fs::canonicalize(&raw_path)
            .map_err(|e| ResolveIndexPathError::CanonicalizeFailed(raw_path, e))
    } else {
        Ok(raw_path)
    }
}

/// symlink チェック（destructive/write コマンドの呼び出し側で使用）
pub fn reject_symlink(path: &Path) -> Result<(), ResolveIndexPathError> {
    if path.exists()
        && path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(ResolveIndexPathError::SymlinkDetected(path.to_path_buf()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper functions (now take commandindex_dir directly)
// ---------------------------------------------------------------------------

/// `{commandindex_dir}/tantivy` ディレクトリのパスを返す
pub fn index_dir(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(TANTIVY_DIR)
}

/// `{commandindex_dir}/symbols.db` のパスを返す
pub fn symbol_db_path(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(SYMBOLS_DB_FILE)
}

/// `{commandindex_dir}/embeddings.db` のパスを返す
pub fn embeddings_db_path(commandindex_dir: &Path) -> PathBuf {
    commandindex_dir.join(EMBEDDINGS_DB_FILE)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_index_path_default_fallback() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_index_path(None, None, tmp.path()).unwrap();
        assert_eq!(result, tmp.path().join(".commandindex"));
    }

    #[test]
    fn test_resolve_index_path_cli_overrides_config() {
        let tmp = TempDir::new().unwrap();
        let cli_path = tmp.path().join("custom-index");
        let result = resolve_index_path(Some(&cli_path), Some("config-index"), tmp.path()).unwrap();
        assert_eq!(result, cli_path);
    }

    #[test]
    fn test_resolve_index_path_config_overrides_default() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_index_path(None, Some("my-index"), tmp.path()).unwrap();
        assert_eq!(result, tmp.path().join("my-index"));
    }

    #[test]
    fn test_resolve_index_path_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let abs_path = tmp.path().join("abs-index");
        let result = resolve_index_path(Some(&abs_path), None, tmp.path()).unwrap();
        assert_eq!(result, abs_path);
    }

    #[test]
    fn test_resolve_index_path_existing_path_canonicalized() {
        let tmp = TempDir::new().unwrap();
        let idx_dir = tmp.path().join("my-index");
        std::fs::create_dir_all(&idx_dir).unwrap();
        let result = resolve_index_path(Some(&idx_dir), None, tmp.path()).unwrap();
        // canonicalize resolves to real path
        assert_eq!(result, std::fs::canonicalize(&idx_dir).unwrap());
    }

    #[test]
    fn test_resolve_index_path_path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let traversal_path = tmp.path().join("a/../../etc/passwd");
        let result = resolve_index_path(Some(&traversal_path), None, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{err}").contains("Path traversal"),
            "Expected PathTraversal error, got: {err}"
        );
    }

    #[test]
    fn test_resolve_index_path_config_absolute() {
        let tmp = TempDir::new().unwrap();
        let abs_config = tmp.path().join("shared-index");
        let result =
            resolve_index_path(None, Some(abs_config.to_str().unwrap()), tmp.path()).unwrap();
        assert_eq!(result, abs_config);
    }

    #[test]
    fn test_reject_symlink_non_symlink() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("normal-dir");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(reject_symlink(&dir).is_ok());
    }

    #[test]
    fn test_reject_symlink_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("nonexistent");
        assert!(reject_symlink(&p).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_reject_symlink_detects_symlink() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("real-dir");
        std::fs::create_dir_all(&target).unwrap();
        let link = tmp.path().join("link-dir");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let result = reject_symlink(&link);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{err}").contains("Symlink detected"),
            "Expected SymlinkDetected, got: {err}"
        );
    }

    #[test]
    fn test_index_dir_helper() {
        let p = Path::new("/tmp/my-index");
        assert_eq!(index_dir(p), PathBuf::from("/tmp/my-index/tantivy"));
    }

    #[test]
    fn test_symbol_db_path_helper() {
        let p = Path::new("/tmp/my-index");
        assert_eq!(symbol_db_path(p), PathBuf::from("/tmp/my-index/symbols.db"));
    }

    #[test]
    fn test_embeddings_db_path_helper() {
        let p = Path::new("/tmp/my-index");
        assert_eq!(
            embeddings_db_path(p),
            PathBuf::from("/tmp/my-index/embeddings.db")
        );
    }
}

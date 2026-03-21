use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum CleanError {
    Io(std::io::Error),
    SymlinkDetected,
}

impl fmt::Display for CleanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CleanError::Io(e) => write!(f, "failed to remove .commandindex/: {e}"),
            CleanError::SymlinkDetected => {
                write!(f, ".commandindex/ is a symbolic link — refusing to delete")
            }
        }
    }
}

impl std::error::Error for CleanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CleanError::Io(e) => Some(e),
            CleanError::SymlinkDetected => None,
        }
    }
}

impl From<std::io::Error> for CleanError {
    fn from(e: std::io::Error) -> Self {
        CleanError::Io(e)
    }
}

/// クリーンオプション（Default実装で後方互換性を維持）
#[derive(Debug, Default)]
pub struct CleanOptions {
    pub keep_embeddings: bool,
}

pub enum CleanResult {
    Removed,
    NotFound,
}

pub fn run(path: &Path, options: &CleanOptions) -> Result<CleanResult, CleanError> {
    let commandindex_dir = path.join(crate::INDEX_DIR_NAME);

    // Symlink safety check
    match std::fs::symlink_metadata(&commandindex_dir) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CleanError::SymlinkDetected);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CleanResult::NotFound);
        }
        Err(e) => {
            return Err(CleanError::Io(e));
        }
    }

    if options.keep_embeddings {
        // Selective deletion: remove tantivy/, manifest.json, state.json, symbols.db
        // but keep embeddings.db and config.toml
        let items_to_delete = [
            "tantivy",
            "manifest.json",
            "state.json",
            "symbols.db",
            "symbols.db-wal",
            "symbols.db-shm",
        ];

        let mut any_deleted = false;
        for item in &items_to_delete {
            let item_path = commandindex_dir.join(item);
            if item_path.exists() {
                if item_path.is_dir() {
                    std::fs::remove_dir_all(&item_path)?;
                } else {
                    std::fs::remove_file(&item_path)?;
                }
                any_deleted = true;
            }
        }

        // If nothing was deleted and no embeddings.db/config.toml exist either,
        // it's effectively not found
        if !any_deleted
            && !commandindex_dir.join("embeddings.db").exists()
            && !commandindex_dir.join("config.toml").exists()
        {
            return Ok(CleanResult::NotFound);
        }

        Ok(CleanResult::Removed)
    } else {
        // Full deletion: remove entire .commandindex/
        match std::fs::remove_dir_all(&commandindex_dir) {
            Ok(()) => Ok(CleanResult::Removed),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CleanResult::NotFound),
            Err(e) => Err(CleanError::Io(e)),
        }
    }
}

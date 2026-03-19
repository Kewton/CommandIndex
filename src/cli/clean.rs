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

pub enum CleanResult {
    Removed,
    NotFound,
}

pub fn run(path: &Path) -> Result<CleanResult, CleanError> {
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

    // TOCTOU-safe: attempt deletion directly
    match std::fs::remove_dir_all(&commandindex_dir) {
        Ok(()) => Ok(CleanResult::Removed),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(CleanResult::NotFound),
        Err(e) => Err(CleanError::Io(e)),
    }
}

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use tar::Archive;

use crate::indexer::snapshot::{
    EXPORT_FORMAT_VERSION, EXPORT_META_FILE, ExportMeta, current_git_hash,
};
use crate::indexer::state::{IndexState, StateError};

/// Maximum cumulative decompressed size (1 GB)
const MAX_DECOMPRESS_SIZE: u64 = 1_073_741_824;

/// Maximum number of entries in the archive
const MAX_ENTRY_COUNT: u64 = 10_000;

/// Import options
pub struct ImportOptions {
    pub force: bool,
}

/// Import result
#[derive(Debug)]
pub struct ImportResult {
    pub imported_files: u64,
    pub git_hash_match: bool,
    pub warnings: Vec<String>,
}

/// Import error
#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    ExistingIndex(PathBuf),
    PathTraversal(String),
    SymlinkDetected(PathBuf),
    InvalidArchive(String),
    IncompatibleVersion { expected: u32, found: u32 },
    DecompressionBomb { limit: u64 },
    State(StateError),
    Deserialize(serde_json::Error),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "IO error: {e}"),
            ImportError::ExistingIndex(p) => {
                write!(
                    f,
                    "Index already exists at {}. Use --force to overwrite.",
                    p.display()
                )
            }
            ImportError::PathTraversal(msg) => write!(f, "Path traversal detected: {msg}"),
            ImportError::SymlinkDetected(p) => {
                write!(f, "Symlink/hardlink entry detected: {}", p.display())
            }
            ImportError::InvalidArchive(msg) => write!(f, "Invalid archive: {msg}"),
            ImportError::IncompatibleVersion { expected, found } => {
                write!(
                    f,
                    "Incompatible export format version: expected <= {expected}, found {found}"
                )
            }
            ImportError::DecompressionBomb { limit } => {
                write!(f, "Decompression bomb: exceeded {limit} bytes limit")
            }
            ImportError::State(e) => write!(f, "State error: {e}"),
            ImportError::Deserialize(e) => write!(f, "Deserialization error: {e}"),
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImportError::Io(e) => Some(e),
            ImportError::State(e) => Some(e),
            ImportError::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<StateError> for ImportError {
    fn from(e: StateError) -> Self {
        ImportError::State(e)
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        ImportError::Deserialize(e)
    }
}

/// Validate an entry path for path traversal attacks
fn validate_entry_path(entry_path: &Path, target_dir: &Path) -> Result<PathBuf, ImportError> {
    // 1. Reject absolute paths
    if entry_path.is_absolute() {
        return Err(ImportError::PathTraversal(format!(
            "absolute path: {}",
            entry_path.display()
        )));
    }

    // 2. Reject ".." and prefix components
    for component in entry_path.components() {
        match component {
            Component::ParentDir => {
                return Err(ImportError::PathTraversal(format!(
                    "parent dir: {}",
                    entry_path.display()
                )));
            }
            Component::Prefix(_) => {
                return Err(ImportError::PathTraversal(format!(
                    "path prefix: {}",
                    entry_path.display()
                )));
            }
            _ => {}
        }
    }

    // 3. Build full path
    let full_path = target_dir.join(entry_path);

    Ok(full_path)
}

/// Validate entry type - reject symlinks and hardlinks
fn validate_entry_type(entry_type: tar::EntryType, entry_path: &Path) -> Result<(), ImportError> {
    match entry_type {
        tar::EntryType::Symlink | tar::EntryType::Link => {
            Err(ImportError::SymlinkDetected(entry_path.to_path_buf()))
        }
        _ => Ok(()),
    }
}

/// Import index from tar.gz archive
pub fn run(
    path: &Path,
    archive: &Path,
    options: &ImportOptions,
) -> Result<ImportResult, ImportError> {
    let ci_dir = crate::indexer::commandindex_dir(path);

    // 1. Check archive exists
    if !archive.exists() {
        return Err(ImportError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Archive not found: {}", archive.display()),
        )));
    }

    // 2. Check existing .commandindex/
    if ci_dir.exists() {
        if !options.force {
            return Err(ImportError::ExistingIndex(ci_dir.clone()));
        }
        // --force: verify not a symlink before removing
        let metadata = std::fs::symlink_metadata(&ci_dir)?;
        if metadata.file_type().is_symlink() {
            return Err(ImportError::SymlinkDetected(ci_dir.clone()));
        }
    }

    // 3. Extract to a temporary directory first, then swap on success
    let temp_dir = ci_dir.with_file_name(".commandindex_import_tmp");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    // 4. Extract archive into temp_dir with security checks
    let result = extract_and_validate(path, archive, &temp_dir);

    match result {
        Ok(import_result) => {
            // All validation passed — atomically swap directories
            if ci_dir.exists() {
                std::fs::remove_dir_all(&ci_dir)?;
            }
            std::fs::rename(&temp_dir, &ci_dir)?;
            Ok(import_result)
        }
        Err(e) => {
            // Cleanup temp dir on any error
            let _ = std::fs::remove_dir_all(&temp_dir);
            Err(e)
        }
    }
}

/// Extract archive contents into target directory and validate
fn extract_and_validate(
    base_path: &Path,
    archive: &Path,
    target_dir: &Path,
) -> Result<ImportResult, ImportError> {
    let file = File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut tar_archive = Archive::new(decoder);

    let mut total_size: u64 = 0;
    let mut entry_count: u64 = 0;
    let mut imported_files: u64 = 0;
    let mut warnings = Vec::new();

    let entries = tar_archive
        .entries()
        .map_err(|e| ImportError::InvalidArchive(format!("Failed to read entries: {e}")))?;

    for entry_result in entries {
        let mut entry = entry_result
            .map_err(|e| ImportError::InvalidArchive(format!("Failed to read entry: {e}")))?;

        // Entry count check
        entry_count += 1;
        if entry_count > MAX_ENTRY_COUNT {
            return Err(ImportError::DecompressionBomb {
                limit: MAX_ENTRY_COUNT,
            });
        }

        let entry_path = entry
            .path()
            .map_err(|e| ImportError::InvalidArchive(format!("Invalid path: {e}")))?
            .to_path_buf();

        let entry_type = entry.header().entry_type();

        // Skip directory entries - we'll create dirs as needed
        if entry_type == tar::EntryType::Directory {
            continue;
        }

        // Security: validate entry type (reject symlinks/hardlinks)
        validate_entry_type(entry_type, &entry_path)?;

        // Security: validate path (reject traversal)
        let full_path = validate_entry_path(&entry_path, target_dir)?;

        // Header size pre-check (early rejection, may be forged)
        let header_size = entry.header().size().unwrap_or(0);
        total_size = total_size.saturating_add(header_size);
        if total_size > MAX_DECOMPRESS_SIZE {
            return Err(ImportError::DecompressionBomb {
                limit: MAX_DECOMPRESS_SIZE,
            });
        }

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Extract file with fixed permissions
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        // Verify actual data size (header size may be forged)
        let actual_size = content.len() as u64;
        if actual_size > header_size {
            // Re-adjust total_size with actual data
            total_size = total_size
                .saturating_sub(header_size)
                .saturating_add(actual_size);
            if total_size > MAX_DECOMPRESS_SIZE {
                return Err(ImportError::DecompressionBomb {
                    limit: MAX_DECOMPRESS_SIZE,
                });
            }
        }

        std::fs::write(&full_path, &content)?;

        // Fix permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if full_path
                .extension()
                .is_some_and(|e| e == "sh" || e == "exe")
            {
                0o755
            } else {
                0o644
            };
            std::fs::set_permissions(&full_path, std::fs::Permissions::from_mode(mode))?;
        }

        imported_files += 1;
    }

    // 5. Load and validate export_meta.json
    let meta_path = target_dir.join(EXPORT_META_FILE);
    if !meta_path.exists() {
        return Err(ImportError::InvalidArchive(
            "Missing export_meta.json".to_string(),
        ));
    }

    let meta = ExportMeta::load(&meta_path).map_err(|e| {
        ImportError::InvalidArchive(format!("Failed to load export_meta.json: {e}"))
    })?;

    // Version compatibility check (forward-compatible policy)
    if meta.export_format_version > EXPORT_FORMAT_VERSION {
        return Err(ImportError::IncompatibleVersion {
            expected: EXPORT_FORMAT_VERSION,
            found: meta.export_format_version,
        });
    }

    // Validate commandindex_version length
    if meta.commandindex_version.len() > 64 {
        return Err(ImportError::InvalidArchive(
            "commandindex_version too long".to_string(),
        ));
    }

    // 6. Rewrite state.json index_root to import target path
    let state_path = target_dir.join("state.json");
    if state_path.exists() {
        let state_content = std::fs::read_to_string(&state_path)?;
        let mut state: IndexState = serde_json::from_str(&state_content)?;

        // Always set index_root to import target path
        state.index_root = base_path.to_path_buf();
        state.save(target_dir)?;
    }

    // 7. Check git hash match
    let current_hash = current_git_hash(base_path);
    let git_hash_match = match (&meta.git_commit_hash, &current_hash) {
        (Some(export_hash), Some(current)) => {
            if export_hash != current {
                warnings.push(format!(
                    "Git commit hash mismatch: exported={}, current={}",
                    export_hash, current
                ));
                false
            } else {
                true
            }
        }
        (Some(_), None) => {
            warnings.push("Could not determine current git commit hash".to_string());
            false
        }
        (None, _) => {
            // No hash recorded in export
            true
        }
    };

    Ok(ImportResult {
        imported_files,
        git_hash_match,
        warnings,
    })
}

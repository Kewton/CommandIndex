use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const MAX_REPOSITORIES: usize = 50;
pub const MAX_ALIAS_LENGTH: usize = 64;
pub const MAX_CONFIG_FILE_SIZE: u64 = 1_048_576; // 1MB

// ---------------------------------------------------------------------------
// Config types (Deserialize)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceDefinition,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceDefinition {
    pub name: String,
    pub repositories: Vec<RepositoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RepositoryEntry {
    pub path: String,
    pub alias: Option<String>,
}

// ---------------------------------------------------------------------------
// Resolved types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolvedRepository {
    pub path: PathBuf,
    pub alias: String,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WorkspaceConfigError {
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
    DuplicateAlias(String),
    DuplicatePath(String),
    TooManyRepositories,
    HomeDirNotFound,
    FileTooLarge {
        path: PathBuf,
        size: u64,
    },
    InvalidName(String),
    UnsafePath(String),
}

impl fmt::Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadError { path, source } => {
                write!(
                    f,
                    "Failed to read workspace config '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ParseError { path, source } => {
                write!(
                    f,
                    "Failed to parse workspace config '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::DuplicateAlias(alias) => {
                write!(f, "Duplicate repository alias: '{}'", alias)
            }
            Self::DuplicatePath(path) => {
                write!(f, "Duplicate repository path: '{}'", path)
            }
            Self::TooManyRepositories => {
                write!(f, "Too many repositories (maximum: {})", MAX_REPOSITORIES)
            }
            Self::HomeDirNotFound => {
                write!(f, "Could not determine home directory")
            }
            Self::FileTooLarge { path, size } => {
                write!(
                    f,
                    "Workspace config file '{}' is too large ({} bytes, max: {} bytes)",
                    path.display(),
                    size,
                    MAX_CONFIG_FILE_SIZE
                )
            }
            Self::InvalidName(name) => {
                write!(f, "Invalid name: '{}'", name)
            }
            Self::UnsafePath(path) => {
                write!(
                    f,
                    "Unsafe path '{}': shell expansion characters are not allowed",
                    path
                )
            }
        }
    }
}

impl std::error::Error for WorkspaceConfigError {}

// ---------------------------------------------------------------------------
// Warning type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WorkspaceWarning {
    RepositoryNotFound { path: PathBuf, alias: String },
    IndexNotFound { path: PathBuf, alias: String },
    PathResolved { original: String, resolved: PathBuf },
    SymlinkDetected { path: PathBuf, alias: String },
}

impl fmt::Display for WorkspaceWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryNotFound { path, alias } => {
                write!(
                    f,
                    "Repository '{}' not found at '{}'",
                    alias,
                    path.display()
                )
            }
            Self::IndexNotFound { path, alias } => {
                write!(
                    f,
                    "Repository '{}' has no index at '{}'",
                    alias,
                    path.display()
                )
            }
            Self::PathResolved { original, resolved } => {
                write!(
                    f,
                    "Path '{}' resolved to '{}'",
                    original,
                    resolved.display()
                )
            }
            Self::SymlinkDetected { path, alias } => {
                write!(
                    f,
                    "Repository '{}' path '{}' is a symbolic link",
                    alias,
                    path.display()
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Load and parse a workspace TOML config file.
/// Validates file size before reading.
pub fn load_workspace_config(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    // Check file size
    let metadata = std::fs::metadata(path).map_err(|e| WorkspaceConfigError::ReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let file_size = metadata.len();
    if file_size > MAX_CONFIG_FILE_SIZE {
        return Err(WorkspaceConfigError::FileTooLarge {
            path: path.to_path_buf(),
            size: file_size,
        });
    }

    // Read file
    let content = std::fs::read_to_string(path).map_err(|e| WorkspaceConfigError::ReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Parse TOML
    let config: WorkspaceConfig =
        toml::from_str(&content).map_err(|e| WorkspaceConfigError::ParseError {
            path: path.to_path_buf(),
            source: e,
        })?;

    // Validate workspace name
    validate_alias(&config.workspace.name)?;

    Ok(config)
}

/// Resolve repository paths and validate uniqueness.
/// Returns resolved repositories and any warnings.
pub fn resolve_repositories(
    config: &WorkspaceConfig,
    base_dir: &Path,
) -> Result<(Vec<ResolvedRepository>, Vec<WorkspaceWarning>), WorkspaceConfigError> {
    // Check repository count
    if config.workspace.repositories.len() > MAX_REPOSITORIES {
        return Err(WorkspaceConfigError::TooManyRepositories);
    }

    let mut resolved = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_aliases = HashSet::new();
    let mut seen_paths = HashSet::new();

    for entry in &config.workspace.repositories {
        // Expand path (tilde, etc.)
        let expanded = expand_path(&entry.path)?;

        // Resolve relative paths against base_dir
        let full_path = if expanded.is_absolute() {
            expanded
        } else {
            base_dir.join(&expanded)
        };

        // Determine alias
        let alias = entry.alias.clone().unwrap_or_else(|| {
            full_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

        // Validate alias
        validate_alias(&alias)?;

        // Check alias uniqueness
        if !seen_aliases.insert(alias.clone()) {
            return Err(WorkspaceConfigError::DuplicateAlias(alias));
        }

        // Check if path is a symlink (before canonicalize)
        let is_symlink = full_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        // Check if path exists
        if !full_path.exists() {
            warnings.push(WorkspaceWarning::RepositoryNotFound {
                path: full_path,
                alias,
            });
            continue;
        }

        // Emit symlink warning
        if is_symlink {
            warnings.push(WorkspaceWarning::SymlinkDetected {
                path: full_path.clone(),
                alias: alias.clone(),
            });
        }

        // Canonicalize for dedup
        let canonical = full_path
            .canonicalize()
            .unwrap_or_else(|_| full_path.clone());

        // Check path uniqueness (after canonicalize)
        let canonical_str = canonical.to_string_lossy().to_string();
        if !seen_paths.insert(canonical_str.clone()) {
            return Err(WorkspaceConfigError::DuplicatePath(canonical_str));
        }

        // Check for .commandindex directory
        let index_dir = canonical.join(crate::INDEX_DIR_NAME);
        if !index_dir.exists() {
            warnings.push(WorkspaceWarning::IndexNotFound {
                path: canonical.clone(),
                alias: alias.clone(),
            });
        }

        // Track path resolution if different from original
        if entry.path != canonical.to_string_lossy() {
            warnings.push(WorkspaceWarning::PathResolved {
                original: entry.path.clone(),
                resolved: canonical.clone(),
            });
        }

        resolved.push(ResolvedRepository {
            path: canonical,
            alias,
        });
    }

    Ok((resolved, warnings))
}

/// Expand tilde in paths. Only `~` and `~/...` are supported.
/// `~user` syntax is rejected. Paths containing `$` or backtick are rejected.
pub fn expand_path(path: &str) -> Result<PathBuf, WorkspaceConfigError> {
    // Reject unsafe shell expansion characters
    if path.contains('$') {
        return Err(WorkspaceConfigError::UnsafePath(path.to_string()));
    }
    if path.contains('`') {
        return Err(WorkspaceConfigError::UnsafePath(path.to_string()));
    }

    if path == "~" {
        // ~ alone → home directory
        dirs::home_dir().ok_or(WorkspaceConfigError::HomeDirNotFound)
    } else if let Some(rest) = path.strip_prefix("~/") {
        // ~/... → home + rest
        let home = dirs::home_dir().ok_or(WorkspaceConfigError::HomeDirNotFound)?;
        Ok(home.join(rest))
    } else if path.starts_with('~') {
        // ~user → rejected
        Err(WorkspaceConfigError::UnsafePath(path.to_string()))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Validate that a name/alias contains only ASCII alphanumeric, hyphen, underscore.
/// Must be non-empty and at most MAX_ALIAS_LENGTH characters.
pub fn validate_alias(name: &str) -> Result<(), WorkspaceConfigError> {
    if name.is_empty() {
        return Err(WorkspaceConfigError::InvalidName(
            "name must not be empty".to_string(),
        ));
    }

    if name.len() > MAX_ALIAS_LENGTH {
        return Err(WorkspaceConfigError::InvalidName(format!(
            "'{}' exceeds maximum length of {} characters",
            name, MAX_ALIAS_LENGTH
        )));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(WorkspaceConfigError::InvalidName(format!(
            "'{}' contains invalid characters (only ASCII alphanumeric, hyphen, underscore allowed)",
            name
        )));
    }

    Ok(())
}

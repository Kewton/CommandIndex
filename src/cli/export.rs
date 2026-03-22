use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;

use crate::indexer::manifest::ManifestError;
use crate::indexer::snapshot::{
    EXPORT_FORMAT_VERSION, EXPORT_META_FILE, ExportMeta, current_git_hash,
};
use crate::indexer::state::{IndexState, StateError};

/// Export options
pub struct ExportOptions {
    pub with_embeddings: bool,
}

/// Export result
#[derive(Debug)]
pub struct ExportResult {
    pub output_path: PathBuf,
    pub archive_size: u64,
    pub git_commit_hash: Option<String>,
}

/// Export error
#[derive(Debug)]
pub enum ExportError {
    NotInitialized,
    Io(std::io::Error),
    State(StateError),
    Manifest(ManifestError),
    Serialize(serde_json::Error),
    GitError(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::NotInitialized => {
                write!(f, "Index not initialized. Run `commandindex index` first.")
            }
            ExportError::Io(e) => write!(f, "IO error: {e}"),
            ExportError::State(e) => write!(f, "State error: {e}"),
            ExportError::Manifest(e) => write!(f, "Manifest error: {e}"),
            ExportError::Serialize(e) => write!(f, "Serialization error: {e}"),
            ExportError::GitError(msg) => write!(f, "Git error: {msg}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(e) => Some(e),
            ExportError::State(e) => Some(e),
            ExportError::Serialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e)
    }
}

impl From<StateError> for ExportError {
    fn from(e: StateError) -> Self {
        ExportError::State(e)
    }
}

impl From<ManifestError> for ExportError {
    fn from(e: ManifestError) -> Self {
        ExportError::Manifest(e)
    }
}

impl From<serde_json::Error> for ExportError {
    fn from(e: serde_json::Error) -> Self {
        ExportError::Serialize(e)
    }
}

/// Placeholder for sanitized index_root in exported state.json
const INDEX_ROOT_PLACEHOLDER: &str = "__COMMANDINDEX_EXPORT_PLACEHOLDER__";

/// Export index as portable tar.gz archive
pub fn run(
    path: &Path,
    output: &Path,
    options: &ExportOptions,
) -> Result<ExportResult, ExportError> {
    let ci_dir = crate::indexer::commandindex_dir(path);

    // 1. Check .commandindex/ exists
    if !IndexState::exists(&ci_dir) {
        return Err(ExportError::NotInitialized);
    }

    // 2. Load index state
    let state = IndexState::load(&ci_dir)?;
    state.check_schema_version()?;

    // 3. Get git commit hash
    let git_hash = current_git_hash(path);

    // 4. Build ExportMeta
    let meta = ExportMeta {
        export_format_version: EXPORT_FORMAT_VERSION,
        commandindex_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_hash: git_hash.clone(),
        exported_at: Utc::now(),
    };

    // 5. Create tar.gz archive
    let file = File::create(output)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    // 5a. Add export_meta.json first
    let meta_json = serde_json::to_string_pretty(&meta)?;
    add_bytes_to_tar(&mut builder, EXPORT_META_FILE, meta_json.as_bytes())?;

    // 5b. Add state.json with sanitized index_root
    let mut sanitized_state = state.clone();
    sanitized_state.index_root = PathBuf::from(INDEX_ROOT_PLACEHOLDER);
    let state_json = serde_json::to_string_pretty(&sanitized_state)?;
    add_bytes_to_tar(&mut builder, "state.json", state_json.as_bytes())?;

    // 5c. Add manifest.json if it exists
    let manifest_path = ci_dir.join("manifest.json");
    if manifest_path.exists() {
        builder.append_path_with_name(&manifest_path, "manifest.json")?;
    }

    // 5d. Add symbols.db if it exists
    let symbols_path = ci_dir.join("symbols.db");
    if symbols_path.exists() {
        builder.append_path_with_name(&symbols_path, "symbols.db")?;
    }

    // 5e. Add tantivy/ directory recursively
    let tantivy_dir = ci_dir.join("tantivy");
    if tantivy_dir.is_dir() {
        builder.append_dir_all("tantivy", &tantivy_dir)?;
    }

    // 5f. Add embeddings.db if --with-embeddings
    if options.with_embeddings {
        let embeddings_path = ci_dir.join("embeddings.db");
        if embeddings_path.exists() {
            builder.append_path_with_name(&embeddings_path, "embeddings.db")?;
        }
    }

    // Note: config.local.toml is always excluded (not added)

    // 6. Finalize archive
    let encoder = builder.into_inner()?;
    encoder.finish()?;

    // 7. Get archive size
    let archive_size = std::fs::metadata(output)?.len();

    Ok(ExportResult {
        output_path: output.to_path_buf(),
        archive_size,
        git_commit_hash: git_hash,
    })
}

/// Helper to add bytes as a file entry to a tar archive
fn add_bytes_to_tar<W: Write>(
    builder: &mut Builder<W>,
    name: &str,
    data: &[u8],
) -> Result<(), std::io::Error> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    header.set_cksum();
    builder.append_data(&mut header, name, data)
}

use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::indexer::SUPPORTED_EXTENSIONS;
use crate::indexer::diff::{DiffError, detect_changes, scan_files};
use crate::indexer::manifest::{self, FileEntry, Manifest, ManifestError, to_relative_path_string};
use crate::indexer::state::{IndexState, StateError};
use crate::indexer::writer::{IndexWriterWrapper, SectionDoc, WriterError};
use crate::parser::ignore::{IgnoreError, IgnoreFilter};
use crate::parser::markdown::{self, ParseError};

#[derive(Debug)]
pub enum IndexError {
    Io(std::io::Error),
    Parse(ParseError),
    Writer(WriterError),
    State(StateError),
    Manifest(ManifestError),
    Ignore(IgnoreError),
    Diff(DiffError),
    IndexNotFound,
    SchemaVersionMismatch,
    IndexCorrupted(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Io(e) => write!(f, "IO error: {e}"),
            IndexError::Parse(e) => write!(f, "Parse error: {e}"),
            IndexError::Writer(e) => write!(f, "Index writer error: {e}"),
            IndexError::State(e) => write!(f, "State error: {e}"),
            IndexError::Manifest(e) => write!(f, "Manifest error: {e}"),
            IndexError::Ignore(e) => write!(f, "Ignore filter error: {e}"),
            IndexError::Diff(e) => write!(f, "Diff error: {e}"),
            IndexError::IndexNotFound => write!(
                f,
                "No index found. Run `commandindex index` to build the index first."
            ),
            IndexError::SchemaVersionMismatch => write!(
                f,
                "Index schema version mismatch. Run `commandindex clean` then `commandindex index` to rebuild."
            ),
            IndexError::IndexCorrupted(detail) => write!(
                f,
                "Failed to read index state: {detail}. Run `commandindex clean` then `commandindex index` to rebuild."
            ),
        }
    }
}

impl std::error::Error for IndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IndexError::Io(e) => Some(e),
            IndexError::Parse(e) => Some(e),
            IndexError::Writer(e) => Some(e),
            IndexError::State(e) => Some(e),
            IndexError::Manifest(e) => Some(e),
            IndexError::Ignore(e) => Some(e),
            IndexError::Diff(e) => Some(e),
            IndexError::IndexNotFound
            | IndexError::SchemaVersionMismatch
            | IndexError::IndexCorrupted(_) => None,
        }
    }
}

impl From<std::io::Error> for IndexError {
    fn from(e: std::io::Error) -> Self {
        IndexError::Io(e)
    }
}

impl From<ParseError> for IndexError {
    fn from(e: ParseError) -> Self {
        IndexError::Parse(e)
    }
}

impl From<WriterError> for IndexError {
    fn from(e: WriterError) -> Self {
        IndexError::Writer(e)
    }
}

impl From<StateError> for IndexError {
    fn from(e: StateError) -> Self {
        IndexError::State(e)
    }
}

impl From<ManifestError> for IndexError {
    fn from(e: ManifestError) -> Self {
        IndexError::Manifest(e)
    }
}

impl From<IgnoreError> for IndexError {
    fn from(e: IgnoreError) -> Self {
        IndexError::Ignore(e)
    }
}

impl From<DiffError> for IndexError {
    fn from(e: DiffError) -> Self {
        IndexError::Diff(e)
    }
}

pub struct IndexSummary {
    pub scanned: u64,
    pub indexed_sections: u64,
    pub skipped: u64,
    pub ignored: u64,
    pub duration: Duration,
}

fn section_to_doc(
    section: &crate::parser::markdown::Section,
    file_path: &str,
    frontmatter: Option<&crate::parser::Frontmatter>,
) -> SectionDoc {
    let tags = frontmatter.map(|fm| fm.tags.join(" ")).unwrap_or_default();

    SectionDoc {
        path: file_path.to_string(),
        heading: section.heading.clone(),
        body: section.body.clone(),
        tags,
        heading_level: section.level as u64,
        line_start: section.line_start as u64,
    }
}

pub fn run(path: &Path) -> Result<IndexSummary, IndexError> {
    let start = Instant::now();

    // 1. Validate target directory
    if !path.is_dir() {
        return Err(IndexError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", path.display()),
        )));
    }

    // 2. Load .cmindexignore
    let ignore_filter = IgnoreFilter::from_file(&path.join(".cmindexignore"))?;

    // 3-4. Walk directory, filter .md files, apply ignore rules
    let mut md_files = Vec::new();
    let mut ignored: u64 = 0;

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let rel_path = entry.path().strip_prefix(path).unwrap_or(entry.path());

        if ignore_filter.is_ignored(rel_path) {
            ignored += 1;
        } else {
            md_files.push(entry.into_path());
        }
    }

    // 5. Remove existing tantivy directory if present
    let commandindex_dir = crate::indexer::commandindex_dir(path);
    let tantivy_dir = crate::indexer::index_dir(path);
    if tantivy_dir.exists() {
        std::fs::remove_dir_all(&tantivy_dir)?;
    }

    // 6. Create new tantivy index
    let mut writer = IndexWriterWrapper::open(&tantivy_dir)?;

    let mut scanned: u64 = 0;
    let mut indexed_sections: u64 = 0;
    let mut skipped: u64 = 0;
    let mut manifest = Manifest::new();

    // 7-11. Parse each file and add to index
    for file_path in &md_files {
        scanned += 1;

        let doc = match markdown::parse_file(file_path) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Warning: skipping {}: {e}", file_path.display());
                skipped += 1;
                continue;
            }
        };

        let rel_path = file_path
            .strip_prefix(path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        let section_count = doc.sections.len() as u64;

        // Add sections to index
        for section in &doc.sections {
            let section_doc = section_to_doc(section, &rel_path, doc.frontmatter.as_ref());
            writer.add_section(&section_doc)?;
        }
        indexed_sections += section_count;

        // Build manifest entry
        let hash =
            manifest::compute_file_hash(file_path).unwrap_or_else(|_| "sha256:unknown".to_string());

        let last_modified = std::fs::metadata(file_path)
            .and_then(|m| m.modified())
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());

        manifest.add_entry(FileEntry {
            path: rel_path,
            hash,
            last_modified,
            sections: section_count,
        });
    }

    // 12. Commit index
    writer.commit()?;

    // 13. Save manifest
    manifest.save(&commandindex_dir)?;

    // 14. Save state
    let mut state = IndexState::new(path.to_path_buf());
    state.total_files = scanned;
    state.total_sections = indexed_sections;
    state.save(&commandindex_dir)?;

    // 15. Return summary
    Ok(IndexSummary {
        scanned,
        indexed_sections,
        skipped,
        ignored,
        duration: start.elapsed(),
    })
}

pub struct IncrementalSummary {
    pub added_files: u64,
    pub added_sections: u64,
    pub modified_files: u64,
    pub modified_sections: u64,
    pub deleted_files: u64,
    pub unchanged: u64,
    pub skipped: u64,
    pub duration: Duration,
}

/// Result type for parse failures that should be skipped vs fatal errors.
enum IndexFileResult {
    Skipped,
    Error(IndexError),
}

/// Parse a file, add its sections to the index, and upsert the manifest entry.
/// Returns the number of sections indexed, or `IndexFileResult::Skipped` on parse failure.
fn index_file_and_upsert(
    file_path: &Path,
    rel_path: &str,
    writer: &mut IndexWriterWrapper,
    manifest: &mut Manifest,
) -> Result<u64, IndexFileResult> {
    let doc = match markdown::parse_file(file_path) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Warning: skipping {}: {e}", file_path.display());
            return Err(IndexFileResult::Skipped);
        }
    };

    let section_count = doc.sections.len() as u64;
    for section in &doc.sections {
        let section_doc = section_to_doc(section, rel_path, doc.frontmatter.as_ref());
        writer
            .add_section(&section_doc)
            .map_err(|e| IndexFileResult::Error(e.into()))?;
    }

    let hash =
        manifest::compute_file_hash(file_path).unwrap_or_else(|_| "sha256:unknown".to_string());
    let last_modified = std::fs::metadata(file_path)
        .and_then(|m| m.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    manifest.upsert_entry(FileEntry {
        path: rel_path.to_string(),
        hash,
        last_modified,
        sections: section_count,
    });

    Ok(section_count)
}

pub fn run_incremental(path: &Path) -> Result<IncrementalSummary, IndexError> {
    let start = Instant::now();

    // 1. Validate target directory
    if !path.is_dir() {
        return Err(IndexError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", path.display()),
        )));
    }

    // 2. Check preconditions: existing index with valid schema
    let commandindex_dir = crate::indexer::commandindex_dir(path);
    if !IndexState::exists(&commandindex_dir) {
        return Err(IndexError::IndexNotFound);
    }
    let mut state = match IndexState::load(&commandindex_dir) {
        Ok(s) => s,
        Err(e) => return Err(IndexError::IndexCorrupted(e.to_string())),
    };
    if state.check_schema_version().is_err() {
        return Err(IndexError::SchemaVersionMismatch);
    }

    // 3. Load ignore filter (from_file returns default if file not found)
    let ignore_filter = IgnoreFilter::from_file(&path.join(".cmindexignore"))?;

    // 4. Scan files
    let scan_result = scan_files(path, &ignore_filter, SUPPORTED_EXTENSIONS)?;

    // 5. Load old manifest
    let mut old_manifest = Manifest::load_or_default(&commandindex_dir)?;

    // 6. Detect changes
    let diff_result = detect_changes(path, &old_manifest, &scan_result.files)?;

    // 7. Early return if no changes
    if diff_result.is_empty() {
        return Ok(IncrementalSummary {
            added_files: 0,
            added_sections: 0,
            modified_files: 0,
            modified_sections: 0,
            deleted_files: 0,
            unchanged: diff_result.unchanged as u64,
            skipped: 0,
            duration: start.elapsed(),
        });
    }

    // 8. Open existing tantivy index
    let tantivy_dir = crate::indexer::index_dir(path);
    let mut writer = match IndexWriterWrapper::open_existing(&tantivy_dir) {
        Ok(w) => w,
        Err(e) => {
            return Err(IndexError::IndexCorrupted(format!(
                "Failed to open tantivy index: {e}"
            )));
        }
    };

    let mut added_files: u64 = 0;
    let mut added_sections: u64 = 0;
    let mut modified_files: u64 = 0;
    let mut modified_sections: u64 = 0;
    let mut deleted_files: u64 = 0;
    let mut skipped: u64 = 0;

    // Track old sections for state update
    let mut old_deleted_sections: u64 = 0;
    let mut old_modified_sections: u64 = 0;

    // 9. Process deleted files
    for del_path in &diff_result.deleted {
        let path_str = del_path.to_string_lossy().to_string();
        writer.delete_by_path(&path_str)?;

        // Track old sections count
        if let Some(entry) = old_manifest.find_by_path(&path_str) {
            old_deleted_sections += entry.sections;
        }

        old_manifest.remove_by_path(&path_str);
        deleted_files += 1;
    }

    // 10. Process modified files
    for mod_path in &diff_result.modified {
        let rel_path = to_relative_path_string(mod_path, path);

        // Track old sections count
        if let Some(entry) = old_manifest.find_by_path(&rel_path) {
            old_modified_sections += entry.sections;
        }

        writer.delete_by_path(&rel_path)?;

        match index_file_and_upsert(mod_path, &rel_path, &mut writer, &mut old_manifest) {
            Ok(section_count) => {
                modified_files += 1;
                modified_sections += section_count;
            }
            Err(IndexFileResult::Skipped) => {
                skipped += 1;
            }
            Err(IndexFileResult::Error(e)) => return Err(e),
        }
    }

    // 11. Process added files
    for add_path in &diff_result.added {
        let rel_path = to_relative_path_string(add_path, path);

        match index_file_and_upsert(add_path, &rel_path, &mut writer, &mut old_manifest) {
            Ok(section_count) => {
                added_files += 1;
                added_sections += section_count;
            }
            Err(IndexFileResult::Skipped) => {
                skipped += 1;
            }
            Err(IndexFileResult::Error(e)) => return Err(e),
        }
    }

    // 12. Commit index
    writer.commit()?;

    // 13. Save updated manifest
    old_manifest.save(&commandindex_dir)?;

    // 14. Update state (use saturating arithmetic to prevent underflow on corrupted state)
    state.total_files = state.total_files.saturating_add(added_files);
    if deleted_files > state.total_files {
        eprintln!(
            "Warning: state inconsistency detected (total_files underflow). Consider running `commandindex clean` and `commandindex index`."
        );
    }
    state.total_files = state.total_files.saturating_sub(deleted_files);
    state.total_sections = state
        .total_sections
        .saturating_add(added_sections + modified_sections);
    let sections_to_remove = old_deleted_sections + old_modified_sections;
    if sections_to_remove > state.total_sections {
        eprintln!(
            "Warning: state inconsistency detected (total_sections underflow). Consider running `commandindex clean` and `commandindex index`."
        );
    }
    state.total_sections = state.total_sections.saturating_sub(sections_to_remove);
    state.touch();
    state.save(&commandindex_dir)?;

    // 15. Return summary
    Ok(IncrementalSummary {
        added_files,
        added_sections,
        modified_files,
        modified_sections,
        deleted_files,
        unchanged: diff_result.unchanged as u64,
        skipped,
        duration: start.elapsed(),
    })
}

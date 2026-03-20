use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::indexer::manifest::{self, FileEntry, Manifest, ManifestError};
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

use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::embedding::store::{EmbeddingStore, EmbeddingStoreError};
use crate::embedding::{Config, EmbeddingError, create_provider};
use crate::indexer::manifest::{Manifest, ManifestError};
use crate::indexer::reader::{IndexReaderWrapper, ReaderError};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum EmbedError {
    IndexNotFound,
    Embedding(EmbeddingError),
    Store(EmbeddingStoreError),
    Manifest(ManifestError),
    Reader(ReaderError),
    Io(std::io::Error),
}

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexNotFound => write!(
                f,
                "No index found. Run `commandindex index` to build the index first."
            ),
            Self::Embedding(e) => write!(f, "Embedding error: {e}"),
            Self::Store(e) => write!(f, "Store error: {e}"),
            Self::Manifest(e) => write!(f, "Manifest error: {e}"),
            Self::Reader(e) => write!(f, "Reader error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for EmbedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexNotFound => None,
            Self::Embedding(e) => Some(e),
            Self::Store(e) => Some(e),
            Self::Manifest(e) => Some(e),
            Self::Reader(e) => Some(e),
            Self::Io(e) => Some(e),
        }
    }
}

impl From<EmbeddingError> for EmbedError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<EmbeddingStoreError> for EmbedError {
    fn from(e: EmbeddingStoreError) -> Self {
        Self::Store(e)
    }
}

impl From<ManifestError> for EmbedError {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}

impl From<ReaderError> for EmbedError {
    fn from(e: ReaderError) -> Self {
        Self::Reader(e)
    }
}

impl From<std::io::Error> for EmbedError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct EmbedSummary {
    pub total_sections: u64,
    pub generated: u64,
    pub cached: u64,
    pub failed: u64,
    pub duration: Duration,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn run(path: &Path) -> Result<EmbedSummary, EmbedError> {
    let start = Instant::now();

    // 1. .commandindex/ existence check
    let commandindex_dir = crate::indexer::commandindex_dir(path);
    if !commandindex_dir.exists() {
        return Err(EmbedError::IndexNotFound);
    }

    // 2. Load config (default if no config.toml)
    let config = Config::load(&commandindex_dir)?;
    let embedding_config = config.and_then(|c| c.embedding).unwrap_or_default();

    // 3. Create provider
    let provider = create_provider(&embedding_config)?;

    // 4. Load manifest
    let manifest = Manifest::load(&commandindex_dir)?;

    // 5. Open EmbeddingStore
    let db_path = crate::indexer::embeddings_db_path(path);
    let store = EmbeddingStore::open(&db_path)?;
    store.create_tables()?;

    // 6. Open tantivy reader for section text retrieval
    let tantivy_dir = crate::indexer::index_dir(path);
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;

    let mut total_sections: u64 = 0;
    let mut generated: u64 = 0;
    let mut cached: u64 = 0;
    let mut failed: u64 = 0;

    // 7. Process each file entry
    for entry in &manifest.files {
        // Check cache: if embedding already exists for this hash, skip
        if store.has_current_embedding(&entry.path, &entry.hash)? {
            cached += entry.sections;
            total_sections += entry.sections;
            continue;
        }

        // Get sections from tantivy
        let sections = reader.search_by_exact_path(&entry.path)?;
        if sections.is_empty() {
            continue;
        }

        total_sections += sections.len() as u64;

        // Build texts for embedding
        let texts: Vec<String> = sections
            .iter()
            .map(|s| {
                if s.heading.is_empty() {
                    s.body.clone()
                } else {
                    format!("{}\n{}", s.heading, s.body)
                }
            })
            .collect();

        // Generate embeddings
        match provider.embed(&texts) {
            Ok(embeddings) => {
                let dimension = provider.dimension();
                let model = provider.model_name();
                for (section, embedding) in sections.iter().zip(embeddings.iter()) {
                    if let Err(e) = store.upsert_embedding(
                        &entry.path,
                        &section.heading,
                        embedding,
                        dimension,
                        model,
                        &entry.hash,
                    ) {
                        eprintln!(
                            "Warning: failed to store embedding for {}#{}: {e}",
                            entry.path, section.heading
                        );
                        failed += 1;
                        continue;
                    }
                    generated += 1;
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: embedding generation failed for {}: {e}",
                    entry.path
                );
                failed += sections.len() as u64;
            }
        }
    }

    Ok(EmbedSummary {
        total_sections,
        generated,
        cached,
        failed,
        duration: start.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // --- EmbedError Display ---

    #[test]
    fn test_embed_error_display_index_not_found() {
        let err = EmbedError::IndexNotFound;
        let msg = format!("{err}");
        assert!(msg.contains("No index found"));
    }

    #[test]
    fn test_embed_error_display_embedding() {
        let err = EmbedError::Embedding(EmbeddingError::Timeout);
        let msg = format!("{err}");
        assert!(msg.contains("Embedding error"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn test_embed_error_display_store() {
        let err = EmbedError::Store(EmbeddingStoreError::SchemaVersionMismatch {
            expected: 1,
            found: 2,
        });
        let msg = format!("{err}");
        assert!(msg.contains("Store error"));
    }

    #[test]
    fn test_embed_error_display_io() {
        let err = EmbedError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let msg = format!("{err}");
        assert!(msg.contains("IO error"));
    }

    // --- From conversions ---

    #[test]
    fn test_embed_error_from_embedding_error() {
        let e: EmbedError = EmbeddingError::RateLimited.into();
        assert!(matches!(e, EmbedError::Embedding(_)));
    }

    #[test]
    fn test_embed_error_from_store_error() {
        let e: EmbedError = EmbeddingStoreError::SchemaVersionMismatch {
            expected: 1,
            found: 2,
        }
        .into();
        assert!(matches!(e, EmbedError::Store(_)));
    }

    #[test]
    fn test_embed_error_from_io_error() {
        let e: EmbedError =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied").into();
        assert!(matches!(e, EmbedError::Io(_)));
    }

    // --- EmbedError source ---

    #[test]
    fn test_embed_error_source_index_not_found() {
        let err = EmbedError::IndexNotFound;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_embed_error_source_io() {
        let err = EmbedError::Io(std::io::Error::other("test"));
        assert!(err.source().is_some());
    }

    // --- run() with non-existent path ---

    #[test]
    fn test_run_index_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = run(tmp.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbedError::IndexNotFound => {}
            other => panic!("Expected IndexNotFound, got: {other}"),
        }
    }
}

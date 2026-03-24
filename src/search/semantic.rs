use std::collections::HashMap;
use std::path::Path;

use crate::config::AppConfig;
use crate::embedding::store::{EmbeddingSimilarityResult, EmbeddingStore, EmbeddingStoreError};
use crate::embedding::{EmbeddingError, create_provider};
use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchResult};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// SemanticError: セマンティック検索固有のエラー型
#[derive(Debug)]
pub enum SemanticError {
    StoreError(EmbeddingStoreError),
    ProviderError(EmbeddingError),
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StoreError(e) => write!(f, "embedding store error: {e}"),
            Self::ProviderError(e) => write!(f, "embedding provider error: {e}"),
        }
    }
}

impl std::error::Error for SemanticError {}

// ---------------------------------------------------------------------------
// Semantic query
// ---------------------------------------------------------------------------

/// セマンティッククエリを実行し、類似結果を返す。
///
/// embeddings.db不在時は `Ok(None)` を返す（エラーではない）。
/// エラー時は `Err(SemanticError)` を返す。
pub fn query_semantic(
    embeddings_db_path: &Path,
    config: &AppConfig,
    query: &str,
    limit: usize,
) -> Result<Option<Vec<EmbeddingSimilarityResult>>, SemanticError> {
    // 1. embeddings.db存在確認 -> 不在ならOk(None)
    if !embeddings_db_path.exists() {
        return Ok(None);
    }

    // 2. EmbeddingStore::open()
    let store = EmbeddingStore::open(embeddings_db_path).map_err(SemanticError::StoreError)?;

    // 3. count() > 0 確認
    let count = store.count().map_err(SemanticError::StoreError)?;
    if count == 0 {
        return Ok(None);
    }

    // 4. create_provider()
    let provider = create_provider(&config.embedding).map_err(SemanticError::ProviderError)?;

    // 5. provider.embed(&[query])
    let embeddings = provider
        .embed(&[query.to_string()])
        .map_err(SemanticError::ProviderError)?;
    let query_embedding = &embeddings[0];

    // 6. store.search_similar()
    let output = store
        .search_similar(query_embedding, limit)
        .map_err(SemanticError::StoreError)?;

    if output.should_warn_dimension_mismatch() {
        eprintln!(
            "Warning: {}/{} embeddings were skipped due to dimension mismatch. \
             Consider re-running 'commandindex embed' after model change.",
            output.skipped_dimension_mismatch, output.total_records
        );
    }

    if output.results.is_empty() {
        return Ok(None);
    }

    Ok(Some(output.results))
}

// ---------------------------------------------------------------------------
// Enrichment
// ---------------------------------------------------------------------------

/// EmbeddingSimilarityResult を SearchResult 型に変換する。
///
/// tantivy からメタデータを取得し、section_heading でマッチングする。
/// マッチしない場合は最低限の情報で fallback する。
pub fn enrich_semantic_to_search_results(
    semantic_results: &[EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<SearchResult>, ReaderError> {
    // Group by file_path
    let mut groups: HashMap<&str, Vec<&EmbeddingSimilarityResult>> = HashMap::new();
    for result in semantic_results {
        groups.entry(&result.file_path).or_default().push(result);
    }

    let mut enriched = Vec::new();

    for (file_path, items) in &groups {
        let sections = reader.search_by_exact_path(file_path)?;

        for item in items {
            let matched = sections.iter().find(|s| s.heading == item.section_heading);

            if let Some(section) = matched {
                enriched.push(SearchResult {
                    path: section.path.clone(),
                    heading: section.heading.clone(),
                    body: section.body.clone(),
                    tags: section.tags.clone(),
                    heading_level: section.heading_level,
                    line_start: section.line_start,
                    score: 0.0, // RRFマージで上書きされる
                });
            } else {
                // Fallback: minimal result
                enriched.push(SearchResult {
                    path: item.file_path.clone(),
                    heading: item.section_heading.clone(),
                    body: String::new(),
                    tags: String::new(),
                    heading_level: 0,
                    line_start: 0,
                    score: 0.0,
                });
            }
        }
    }

    Ok(enriched)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_error_display_store() {
        let err = SemanticError::StoreError(EmbeddingStoreError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        )));
        let msg = format!("{err}");
        assert!(
            msg.contains("embedding store error"),
            "Expected store error message: {msg}"
        );
    }

    #[test]
    fn semantic_error_display_provider() {
        let err = SemanticError::ProviderError(EmbeddingError::NetworkError("timeout".to_string()));
        let msg = format!("{err}");
        assert!(
            msg.contains("embedding provider error"),
            "Expected provider error message: {msg}"
        );
    }

    #[test]
    fn query_semantic_returns_none_when_db_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake_path = tmp.path().join("nonexistent_embeddings.db");
        let config = crate::config::load_config(tmp.path()).unwrap();
        let result = query_semantic(&fake_path, &config, "test query", 10);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}

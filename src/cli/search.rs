use std::fmt;
use std::path::Path;

use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchFilters, SearchOptions};
use crate::indexer::symbol_store::{SymbolInfo, SymbolStore, SymbolStoreError};
use crate::output::{self, OutputError, OutputFormat, SemanticSearchResult, SymbolSearchResult};

#[derive(Debug)]
pub enum SearchError {
    IndexNotFound,
    Reader(ReaderError),
    Output(OutputError),
    SymbolStore(SymbolStoreError),
    SymbolDbNotFound,
    InvalidArgument(String),
    SchemaVersionMismatch,
    RelatedSearch(crate::search::related::RelatedSearchError),
    Embedding(crate::embedding::EmbeddingError),
    NoEmbeddings,
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::IndexNotFound => {
                write!(f, "Index not found. Run `commandindex index` first.")
            }
            SearchError::Reader(e) => write!(f, "{e}"),
            SearchError::Output(e) => write!(f, "{e}"),
            SearchError::SymbolStore(e) => write!(f, "{e}"),
            SearchError::SymbolDbNotFound => {
                write!(
                    f,
                    "Symbol database not found. Run `commandindex index` first."
                )
            }
            SearchError::InvalidArgument(msg) => write!(f, "{msg}"),
            SearchError::RelatedSearch(e) => write!(f, "{e}"),
            SearchError::SchemaVersionMismatch => write!(
                f,
                "Index schema version mismatch. Run `commandindex clean` then `commandindex index` to rebuild."
            ),
            SearchError::Embedding(e) => match e {
                crate::embedding::EmbeddingError::NetworkError(_) => {
                    write!(
                        f,
                        "Embedding error: {e}\nHint: Is Ollama running? Try `ollama serve`"
                    )
                }
                _ => write!(f, "Embedding error: {e}"),
            },
            SearchError::NoEmbeddings => {
                write!(f, "No embeddings found. Run `commandindex embed` first.")
            }
        }
    }
}

impl std::error::Error for SearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SearchError::IndexNotFound => None,
            SearchError::Reader(e) => Some(e),
            SearchError::Output(e) => Some(e),
            SearchError::SymbolStore(e) => Some(e),
            SearchError::SymbolDbNotFound => None,
            SearchError::InvalidArgument(_) => None,
            SearchError::SchemaVersionMismatch => None,
            SearchError::RelatedSearch(e) => Some(e),
            SearchError::Embedding(e) => Some(e),
            SearchError::NoEmbeddings => None,
        }
    }
}

impl From<ReaderError> for SearchError {
    fn from(e: ReaderError) -> Self {
        SearchError::Reader(e)
    }
}

impl From<OutputError> for SearchError {
    fn from(e: OutputError) -> Self {
        SearchError::Output(e)
    }
}

impl From<crate::search::related::RelatedSearchError> for SearchError {
    fn from(e: crate::search::related::RelatedSearchError) -> Self {
        SearchError::RelatedSearch(e)
    }
}

impl From<SymbolStoreError> for SearchError {
    fn from(e: SymbolStoreError) -> Self {
        match e {
            SymbolStoreError::SchemaVersionMismatch { .. } => SearchError::SchemaVersionMismatch,
            other => SearchError::SymbolStore(other),
        }
    }
}

impl From<crate::embedding::EmbeddingError> for SearchError {
    fn from(e: crate::embedding::EmbeddingError) -> Self {
        SearchError::Embedding(e)
    }
}

pub fn run(
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
) -> Result<(), SearchError> {
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let results = reader.search_with_options(options, filters)?;
    if results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_results(&results, format, &mut handle)?;
    Ok(())
}

pub fn run_symbol_search(
    symbol_name: &str,
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError> {
    if symbol_name.is_empty() {
        return Err(SearchError::InvalidArgument(
            "Symbol name cannot be empty".to_string(),
        ));
    }
    if symbol_name.len() > 256 {
        return Err(SearchError::InvalidArgument(
            "Symbol name too long (max 256 characters)".to_string(),
        ));
    }

    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    let store = SymbolStore::open(&db_path)?;
    let symbols = store.find_by_name_like(symbol_name, limit)?;
    let results = build_symbol_tree(&store, &symbols)?;

    if results.is_empty() {
        eprintln!("No symbols found matching '{symbol_name}'");
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_symbol_results(&results, format, &mut handle)?;
    Ok(())
}

pub fn run_related_search(
    file_path: &str,
    limit: usize,
    format: OutputFormat,
) -> Result<(), SearchError> {
    if file_path.is_empty() {
        return Err(SearchError::InvalidArgument(
            "File path cannot be empty".to_string(),
        ));
    }
    if file_path.len() > 1024 {
        return Err(SearchError::InvalidArgument(
            "File path too long (max 1024 characters)".to_string(),
        ));
    }

    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    let engine = crate::search::related::RelatedSearchEngine::new(&reader, &store);
    let results = engine.find_related(file_path, limit)?;

    if results.is_empty() {
        eprintln!("No related files found for '{file_path}'");
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}

pub fn run_semantic_search(
    query: &str,
    limit: usize,
    format: OutputFormat,
    tag: Option<&str>,
    filters: &SearchFilters,
) -> Result<(), SearchError> {
    if query.is_empty() {
        return Err(SearchError::InvalidArgument(
            "Semantic search query cannot be empty".to_string(),
        ));
    }

    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    // Load embedding config
    let commandindex_dir = crate::indexer::commandindex_dir(Path::new("."));
    let config = crate::embedding::Config::load(&commandindex_dir)?;
    let embedding_config = config.and_then(|c| c.embedding).unwrap_or_default();
    let provider = crate::embedding::create_provider(&embedding_config)?;

    // Check embeddings exist
    let store = SymbolStore::open(&db_path)?;
    if store.count_embeddings()? == 0 {
        return Err(SearchError::NoEmbeddings);
    }

    // Generate query embedding
    let query_texts = [query.to_string()];
    let query_embeddings = provider.embed(&query_texts)?;
    let query_embedding = query_embeddings.first().ok_or_else(|| {
        SearchError::InvalidArgument("Failed to generate query embedding".to_string())
    })?;

    // Search similar with oversampling
    let similar_results = store.search_similar(query_embedding, limit.saturating_mul(5))?;

    // Enrich with metadata from tantivy
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let enriched = enrich_with_metadata(&similar_results, &reader)?;

    // Apply filters and truncate to limit
    let final_results: Vec<SemanticSearchResult> = apply_semantic_filters(enriched, tag, filters)
        .into_iter()
        .take(limit)
        .collect();

    if final_results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_semantic_results(&final_results, format, &mut handle)?;
    Ok(())
}

fn enrich_with_metadata(
    similar_results: &[crate::indexer::symbol_store::EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<SemanticSearchResult>, SearchError> {
    use std::collections::HashMap;

    // Group by file_path
    let mut groups: HashMap<&str, Vec<&crate::indexer::symbol_store::EmbeddingSimilarityResult>> =
        HashMap::new();
    for result in similar_results {
        groups.entry(&result.file_path).or_default().push(result);
    }

    let mut enriched = Vec::new();

    for (file_path, items) in &groups {
        let sections = reader.search_by_exact_path(file_path)?;

        for item in items {
            // Find matching section by heading
            let matched = sections.iter().find(|s| s.heading == item.section_heading);

            if let Some(section) = matched {
                enriched.push(SemanticSearchResult {
                    path: section.path.clone(),
                    heading: section.heading.clone(),
                    similarity: item.similarity,
                    body: section.body.clone(),
                    tags: section.tags.clone(),
                    heading_level: section.heading_level,
                });
            } else {
                // Fallback: use the first section or create a minimal result
                enriched.push(SemanticSearchResult {
                    path: item.file_path.clone(),
                    heading: item.section_heading.clone(),
                    similarity: item.similarity,
                    body: String::new(),
                    tags: String::new(),
                    heading_level: 0,
                });
            }
        }
    }

    // Sort by similarity descending
    enriched.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(enriched)
}

fn apply_semantic_filters(
    results: Vec<SemanticSearchResult>,
    tag: Option<&str>,
    filters: &SearchFilters,
) -> Vec<SemanticSearchResult> {
    results
        .into_iter()
        .filter(|r| {
            // path_prefix filter
            if let Some(ref prefix) = filters.path_prefix
                && !r.path.starts_with(prefix.as_str())
            {
                return false;
            }

            // file_type filter
            if let Some(ref file_type) = filters.file_type
                && !crate::indexer::reader::matches_file_type(&r.path, file_type)
            {
                return false;
            }

            // tag filter
            if let Some(tag_value) = tag
                && !r
                    .tags
                    .split_whitespace()
                    .any(|t| t.eq_ignore_ascii_case(tag_value))
            {
                return false;
            }

            true
        })
        .collect()
}

fn build_symbol_tree(
    store: &SymbolStore,
    symbols: &[SymbolInfo],
) -> Result<Vec<SymbolSearchResult>, SearchError> {
    let mut results = Vec::new();
    for sym in symbols {
        let children = if let Some(id) = sym.id {
            let child_symbols = store.find_children_by_parent_id(id)?;
            child_symbols
                .iter()
                .map(|c| SymbolSearchResult {
                    name: c.name.clone(),
                    kind: c.kind.clone(),
                    file_path: c.file_path.clone(),
                    line_start: c.line_start,
                    line_end: c.line_end,
                    parent_name: Some(sym.name.clone()),
                    children: Vec::new(),
                })
                .collect()
        } else {
            Vec::new()
        };

        results.push(SymbolSearchResult {
            name: sym.name.clone(),
            kind: sym.kind.clone(),
            file_path: sym.file_path.clone(),
            line_start: sym.line_start,
            line_end: sym.line_end,
            parent_name: None,
            children,
        });
    }
    Ok(results)
}

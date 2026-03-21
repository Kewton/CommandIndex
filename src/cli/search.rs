use std::fmt;
use std::path::Path;

use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchFilters, SearchOptions};
use crate::indexer::symbol_store::{SymbolInfo, SymbolStore, SymbolStoreError};
use crate::output::{
    self, OutputError, OutputFormat, SemanticSearchResult, SnippetConfig, SymbolSearchResult,
};

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
    snippet_config: SnippetConfig,
) -> Result<(), SearchError> {
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;

    // BM25検索実行
    let results = reader.search_with_options(options, filters)?;

    // ハイブリッド判定: no_semanticでなく、heading指定がない場合にハイブリッド統合
    let use_hybrid = !options.no_semantic && options.heading.is_none();

    let final_results = if use_hybrid {
        try_hybrid_search(results, options, filters)?
    } else {
        results
    };

    if final_results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match format {
        OutputFormat::Human => {
            output::human::format_human(&final_results, &mut handle, snippet_config)?;
        }
        _ => {
            output::format_results(&final_results, format, &mut handle)?;
        }
    }
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

/// ハイブリッド検索を試行し、BM25結果とセマンティック結果をRRFで統合する。
/// 外部依存の一時的障害時はBM25結果をそのまま返す（graceful degradation）。
fn try_hybrid_search(
    bm25_results: Vec<crate::indexer::reader::SearchResult>,
    options: &SearchOptions,
    filters: &SearchFilters,
) -> Result<Vec<crate::indexer::reader::SearchResult>, SearchError> {
    use crate::search::hybrid::{rrf_merge, HYBRID_OVERSAMPLING_FACTOR};

    // 1. SymbolStore を開く
    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    let store = match crate::indexer::symbol_store::SymbolStore::open(&db_path) {
        Ok(s) => s,
        Err(crate::indexer::symbol_store::SymbolStoreError::SchemaVersionMismatch { .. }) => {
            return Err(SearchError::SchemaVersionMismatch);
        }
        Err(_) => {
            eprintln!("[hybrid] Embedding database not available, using BM25 only.");
            return Ok(bm25_results);
        }
    };

    // 2. Embeddingが存在するか確認
    match store.count_embeddings() {
        Ok(0) => {
            eprintln!("[hybrid] No embeddings found, using BM25 only.");
            return Ok(bm25_results);
        }
        Err(_) => {
            eprintln!("[hybrid] Failed to check embeddings, using BM25 only.");
            return Ok(bm25_results);
        }
        Ok(_) => {}
    }

    // 3. EmbeddingConfig読み込み → provider生成
    let commandindex_dir = crate::indexer::commandindex_dir(Path::new("."));
    let config = match crate::embedding::Config::load(&commandindex_dir) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("[hybrid] Failed to load embedding config, using BM25 only.");
            return Ok(bm25_results);
        }
    };
    let embedding_config = config.and_then(|c| c.embedding).unwrap_or_default();
    let provider = match crate::embedding::create_provider(&embedding_config) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[hybrid] Failed to create embedding provider, using BM25 only.");
            return Ok(bm25_results);
        }
    };

    // 4. クエリ埋め込み生成
    let query_texts = [options.query.clone()];
    let query_embeddings = match provider.embed(&query_texts) {
        Ok(e) => e,
        Err(_) => {
            eprintln!("[hybrid] Failed to generate query embedding, using BM25 only.");
            return Ok(bm25_results);
        }
    };
    let query_embedding = match query_embeddings.first() {
        Some(e) => e,
        None => {
            eprintln!("[hybrid] Empty query embedding result, using BM25 only.");
            return Ok(bm25_results);
        }
    };

    // 5. 類似検索（オーバーサンプリング付き）
    let similar_results = match store.search_similar(
        query_embedding,
        options.limit.saturating_mul(HYBRID_OVERSAMPLING_FACTOR),
    ) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[hybrid] Similarity search failed, using BM25 only.");
            return Ok(bm25_results);
        }
    };

    // 6. セマンティック結果をSearchResult型に変換
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    let reader = match IndexReaderWrapper::open(&tantivy_dir) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[hybrid] Failed to open index reader, using BM25 only.");
            return Ok(bm25_results);
        }
    };
    let semantic_search_results =
        match enrich_semantic_to_search_results(&similar_results, &reader) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("[hybrid] Failed to enrich semantic results, using BM25 only.");
                return Ok(bm25_results);
            }
        };

    // 7. フィルタ適用（tag/path/file_type）
    let filtered_semantic: Vec<crate::indexer::reader::SearchResult> = semantic_search_results
        .into_iter()
        .filter(|r| {
            if let Some(ref prefix) = filters.path_prefix
                && !r.path.starts_with(prefix.as_str())
            {
                return false;
            }
            if let Some(ref file_type) = filters.file_type
                && !crate::indexer::reader::matches_file_type(&r.path, file_type)
            {
                return false;
            }
            if let Some(ref tag) = options.tag
                && !r
                    .tags
                    .split_whitespace()
                    .any(|t| t.eq_ignore_ascii_case(tag))
            {
                return false;
            }
            true
        })
        .collect();

    // 8. RRFマージ
    Ok(rrf_merge(&bm25_results, &filtered_semantic, options.limit))
}

/// セマンティック検索結果をSearchResult型に変換する（ハイブリッド検索用）
/// tantivyからメタデータを取得し、section_headingでマッチングする。
fn enrich_semantic_to_search_results(
    semantic_results: &[crate::indexer::symbol_store::EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<crate::indexer::reader::SearchResult>, SearchError> {
    use std::collections::HashMap;

    // Group by file_path
    let mut groups: HashMap<&str, Vec<&crate::indexer::symbol_store::EmbeddingSimilarityResult>> =
        HashMap::new();
    for result in semantic_results {
        groups.entry(&result.file_path).or_default().push(result);
    }

    let mut enriched = Vec::new();

    for (file_path, items) in &groups {
        let sections = reader.search_by_exact_path(file_path)?;

        for item in items {
            let matched = sections.iter().find(|s| s.heading == item.section_heading);

            if let Some(section) = matched {
                enriched.push(crate::indexer::reader::SearchResult {
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
                enriched.push(crate::indexer::reader::SearchResult {
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

pub const SEARCH_AFTER_HELP: &str = "\
When to use:
  Find relevant documents, code, or symbols across your repository.
  Use --related for impact analysis, --semantic for meaning-based search.

Search modes (mutually exclusive):
  [QUERY]                Full-text keyword search (default)
  --symbol <NAME>        Search for code symbols (functions, structs, etc.)
  --related <FILE>       Find files related to specified file(s)
  --related-stdin        Find related files from stdin paths
  --semantic <QUERY>     Meaning-based search (requires embeddings)
  --changed-since <EXPR> Find content changed since time expression

Examples:
  commandindexdev search \"query\" --format json          # Full-text search
  commandindexdev search --related src/auth.rs          # Related files
  commandindexdev search --semantic \"login flow\"        # Semantic search
  commandindexdev search --changed-since \"yesterday\"    # Recent changes
  commandindexdev search --symbol parse_config           # Symbol search
  # Related files with snippets
  commandindex search --related src/auth.rs --with-snippet --format json";

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::{AppConfig, ConfigError, load_config};
use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchFilters, SearchOptions};
use crate::indexer::symbol_store::{SymbolInfo, SymbolStore, SymbolStoreError};
use crate::output::{
    self, LlmFormatOptions, OutputError, OutputFormat, SemanticSearchResult, SnippetConfig,
    SymbolSearchResult,
};
use crate::rerank::RerankError;

// ---------------------------------------------------------------------------
// RerankStatus (private to CLI layer)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum RerankStatus {
    Applied,
    AppliedPartially { warning: String },
    Skipped { reason: String },
}

// ---------------------------------------------------------------------------
// Rerank output helpers
// ---------------------------------------------------------------------------

/// RerankError に対応するユーザー向けヒント文字列を返す（テスト用）
#[cfg(test)]
fn rerank_error_hint(err: &RerankError) -> &'static str {
    match err {
        RerankError::ModelNotFound(_) => {
            "Run `ollama pull <model>` to install, or set rerank.model in config."
        }
        RerankError::NetworkError(_) => "Is Ollama running? Try `ollama serve`.",
        RerankError::Timeout => "Check Ollama server load.",
        RerankError::ApiError { .. } => "Check Ollama logs.",
        RerankError::InvalidResponse(_) => "Check model compatibility.",
        RerankError::ConfigError(_) => "Check rerank settings in commandindex.toml.",
        RerankError::PartialTimeout { .. } => "Some candidates were not scored due to timeout.",
    }
}

/// reason 文字列をサニタイズする（制御文字除去、HTMLコメント破壊防止、長さ制限）
fn sanitize_reason(reason: &str) -> String {
    let sanitized: String = reason
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // HTMLコメント破壊防止: --> をエスケープ
    let sanitized = sanitized.replace("-->", "--&gt;");
    // 長さ制限（200文字）
    if sanitized.len() > 200 {
        format!("{}...", &sanitized[..197])
    } else {
        sanitized
    }
}

/// stdout 向けメタデータを生成する（json/llm のみ）
fn build_rerank_stdout_prefix(status: &RerankStatus, format: OutputFormat) -> Option<String> {
    match (status, format) {
        (RerankStatus::Skipped { reason }, OutputFormat::Json) => {
            let sanitized = sanitize_reason(reason);
            let meta = serde_json::json!({
                "type": "metadata",
                "rerank_status": "skipped",
                "rerank_warnings": [sanitized],
            });
            serde_json::to_string(&meta).ok()
        }
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Json) => {
            let sanitized = sanitize_reason(warning);
            let meta = serde_json::json!({
                "type": "metadata",
                "rerank_status": "partial",
                "rerank_warnings": [sanitized],
            });
            serde_json::to_string(&meta).ok()
        }
        (RerankStatus::Skipped { reason }, OutputFormat::Llm) => {
            let sanitized = sanitize_reason(reason);
            Some(format!("<!-- rerank skipped: {sanitized} -->"))
        }
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Llm) => {
            let sanitized = sanitize_reason(warning);
            Some(format!("<!-- rerank warning: {sanitized} -->"))
        }
        _ => None,
    }
}

/// stderr 向け警告メッセージを生成する（human/path のみ）
fn build_rerank_stderr_message(status: &RerankStatus, format: OutputFormat) -> Option<String> {
    match (status, format) {
        (RerankStatus::Skipped { reason }, OutputFormat::Human | OutputFormat::Path) => {
            let sanitized = sanitize_reason(reason);
            let hint = rerank_error_hint_from_reason(reason);
            Some(format!(
                "[rerank] Reranking skipped: {sanitized}\n[rerank] Hint: {hint}"
            ))
        }
        (RerankStatus::AppliedPartially { warning }, OutputFormat::Human | OutputFormat::Path) => {
            let sanitized = sanitize_reason(warning);
            Some(format!("[rerank] Warning: {sanitized}"))
        }
        _ => None,
    }
}

/// reason 文字列のパターンから対応するヒントを返す
fn rerank_error_hint_from_reason(reason: &str) -> &'static str {
    if reason.contains("Model not found") {
        "Run `ollama pull <model>` to install, or set rerank.model in config."
    } else if reason.contains("Network error") {
        "Is Ollama running? Try `ollama serve`."
    } else if reason.contains("Request timeout") {
        "Check Ollama server load."
    } else if reason.contains("API error") {
        "Check Ollama logs."
    } else if reason.contains("Invalid response") {
        "Check model compatibility."
    } else if reason.contains("Config error") {
        "Check rerank settings in commandindex.toml."
    } else if reason.contains("Timeout") {
        "Some candidates were not scored due to timeout."
    } else {
        "Check Ollama configuration and server status."
    }
}

// ---------------------------------------------------------------------------
// SearchContext
// ---------------------------------------------------------------------------

pub struct SearchContext {
    pub base_path: PathBuf,
    pub commandindex_dir: PathBuf,
    pub config: AppConfig,
}

impl SearchContext {
    /// New constructor: resolves index path from CLI option, config, and base_path
    pub fn new(base_path: &Path, index_path: Option<&Path>) -> Result<Self, SearchError> {
        let config = load_config(base_path)?;
        let commandindex_dir =
            crate::indexer::resolve_index_path(index_path, config.index.path.as_deref(), base_path)
                .map_err(|e| SearchError::Config(e.to_string()))?;
        Ok(Self {
            base_path: base_path.to_path_buf(),
            commandindex_dir,
            config,
        })
    }

    /// Convenience: from_path with no CLI index_path override
    pub fn from_path(base_path: &Path) -> Result<Self, SearchError> {
        Self::new(base_path, None)
    }

    pub fn index_dir(&self) -> PathBuf {
        crate::indexer::index_dir(&self.commandindex_dir)
    }

    pub fn symbol_db_path(&self) -> PathBuf {
        crate::indexer::symbol_db_path(&self.commandindex_dir)
    }

    pub fn embeddings_db_path(&self) -> PathBuf {
        crate::indexer::embeddings_db_path(&self.commandindex_dir)
    }
}

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
    EmbeddingStore(crate::embedding::store::EmbeddingStoreError),
    NoEmbeddings,
    Config(String),
    Workspace(crate::config::workspace::WorkspaceConfigError),
    Stdin(crate::cli::stdin::StdinError),
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
            SearchError::EmbeddingStore(e) => write!(f, "Embedding store error: {e}"),
            SearchError::NoEmbeddings => {
                write!(f, "No embeddings found. Run `commandindex embed` first.")
            }
            SearchError::Config(msg) => write!(f, "Config error: {msg}"),
            SearchError::Workspace(e) => write!(f, "Workspace error: {e}"),
            SearchError::Stdin(e) => write!(f, "{e}"),
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
            SearchError::EmbeddingStore(e) => Some(e),
            SearchError::NoEmbeddings => None,
            SearchError::Config(_) => None,
            SearchError::Workspace(e) => Some(e),
            SearchError::Stdin(e) => Some(e),
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

impl From<crate::embedding::store::EmbeddingStoreError> for SearchError {
    fn from(e: crate::embedding::store::EmbeddingStoreError) -> Self {
        // Map "no such table" SQLite errors to NoEmbeddings
        if let crate::embedding::store::EmbeddingStoreError::Sqlite(ref sqlite_err) = e {
            let msg = sqlite_err.to_string();
            if msg.contains("no such table: embeddings") {
                return SearchError::NoEmbeddings;
            }
        }
        SearchError::EmbeddingStore(e)
    }
}

impl From<ConfigError> for SearchError {
    fn from(e: ConfigError) -> Self {
        SearchError::Config(e.to_string())
    }
}

impl From<crate::config::workspace::WorkspaceConfigError> for SearchError {
    fn from(e: crate::config::workspace::WorkspaceConfigError) -> Self {
        SearchError::Workspace(e)
    }
}

impl From<crate::cli::stdin::StdinError> for SearchError {
    fn from(e: crate::cli::stdin::StdinError) -> Self {
        SearchError::Stdin(e)
    }
}

impl From<crate::indexer::ResolveIndexPathError> for SearchError {
    fn from(e: crate::indexer::ResolveIndexPathError) -> Self {
        SearchError::Config(e.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    ctx: &SearchContext,
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
    rerank: bool,
    rerank_top: Option<usize>,
    max_tokens: Option<usize>,
    llm_options: &LlmFormatOptions,
) -> Result<(), SearchError> {
    let tantivy_dir = ctx.index_dir();
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;

    // Use config from SearchContext
    let config = &ctx.config;

    // rerank有効時、検索前にlimitを拡大して候補を多く取得
    let original_limit = options.limit;
    let rerank_top_resolved = rerank_top.unwrap_or(config.rerank.top_candidates);
    let effective_options = if rerank {
        let mut opts = options.clone();
        opts.limit = std::cmp::max(options.limit, rerank_top_resolved);
        opts
    } else {
        options.clone()
    };

    // BM25検索実行
    let results = reader.search_with_options(&effective_options, filters)?;

    // ハイブリッド判定: no_semanticでなく、heading指定がない場合にハイブリッド統合
    let use_hybrid = !effective_options.no_semantic && effective_options.heading.is_none();

    let final_results = if use_hybrid {
        try_hybrid_search(
            results,
            &effective_options,
            filters,
            config,
            &ctx.commandindex_dir,
        )?
    } else {
        results
    };

    // Reranking適用
    let (final_results, rerank_status) = if rerank {
        let (reranked, status) = try_rerank(
            final_results,
            &effective_options.query,
            rerank_top_resolved,
            config,
        );
        (
            reranked.into_iter().take(original_limit).collect(),
            Some(status),
        )
    } else {
        (final_results, None)
    };

    // トークン予算適用（--max-tokens）
    let final_results = if let Some(max_tok) = max_tokens {
        crate::output::token_budget::apply_token_budget(final_results, max_tok, |r| {
            crate::output::estimate_tokens(&r.body)
        })
    } else {
        final_results
    };

    if final_results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    // stdout prefix（format_results() の前に出力）
    if let Some(ref status) = rerank_status
        && let Some(prefix) = build_rerank_stdout_prefix(status, format)
    {
        writeln!(handle, "{prefix}").map_err(OutputError::from)?;
    }

    match format {
        OutputFormat::Human => {
            output::human::format_human(&final_results, &mut handle, snippet_config)?;
        }
        _ => {
            output::format_results(&final_results, format, &mut handle, llm_options)?;
        }
    }

    // stderr 警告（human/path のみ）
    if let Some(ref status) = rerank_status
        && let Some(msg) = build_rerank_stderr_message(status, format)
    {
        eprintln!("{msg}");
    }

    Ok(())
}

pub fn run_symbol_search(
    symbol_name: &str,
    limit: usize,
    format: OutputFormat,
    ctx: Option<&SearchContext>,
    max_tokens: Option<usize>,
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

    let db_path = if let Some(c) = ctx {
        c.symbol_db_path()
    } else {
        let default_dir = Path::new(".").join(crate::INDEX_DIR_NAME);
        crate::indexer::symbol_db_path(&default_dir)
    };
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

    // トークン予算適用（--max-tokens）
    let results = if let Some(max_tok) = max_tokens {
        crate::output::token_budget::apply_token_budget(results, max_tok, |r| {
            estimate_symbol_result_tokens(r)
        })
    } else {
        results
    };

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_symbol_results(&results, format, &mut handle)?;
    Ok(())
}

pub fn run_related_search(
    file_paths: &[String],
    limit: usize,
    format: OutputFormat,
    ctx: Option<&SearchContext>,
    snippet_options: crate::cli::snippet_helper::SnippetOptions,
    max_tokens: Option<usize>,
) -> Result<(), SearchError> {
    super::validate_file_paths(file_paths, 100)?;

    let (tantivy_dir, db_path) = if let Some(c) = ctx {
        (c.index_dir(), c.symbol_db_path())
    } else {
        let default_dir = Path::new(".").join(crate::INDEX_DIR_NAME);
        (
            crate::indexer::index_dir(&default_dir),
            crate::indexer::symbol_db_path(&default_dir),
        )
    };
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    let mut results = super::context::collect_related_context(file_paths, &reader, &store)?;
    results.truncate(limit);

    if results.is_empty() {
        let files_list: String = file_paths
            .iter()
            .map(|p| crate::output::strip_control_chars(p))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("No related files found for: {files_list}");
        return Ok(());
    }

    // limit 適用後にスニペット一括付与
    crate::cli::snippet_helper::enrich_related_with_snippets(
        &mut results,
        &reader,
        &snippet_options,
        format,
    );

    // トークン予算適用（--max-tokens）
    let results = if let Some(max_tok) = max_tokens {
        crate::output::token_budget::apply_token_budget(results, max_tok, |r| {
            let mut tokens = crate::output::estimate_tokens(&r.file_path);
            if let Some(ref snippet) = r.snippet {
                tokens = tokens.saturating_add(crate::output::estimate_tokens(snippet));
            }
            tokens
        })
    } else {
        results
    };

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}

/// stdin からの複数ファイル関連検索
pub fn run_related_search_from_stdin(
    limit: usize,
    format: OutputFormat,
    snippet_options: crate::cli::snippet_helper::SnippetOptions,
    max_tokens: Option<usize>,
) -> Result<(), SearchError> {
    let files = crate::cli::stdin::read_file_paths_from_stdin(500)?;

    // 存在チェック + warning
    let (valid_files, warnings) = crate::cli::stdin::filter_existing_files(&files);
    for w in &warnings {
        eprintln!("Warning: {w}");
    }
    if valid_files.is_empty() {
        return Err(SearchError::Stdin(
            crate::cli::stdin::StdinError::NoValidPaths,
        ));
    }

    // インデックス確認（resolve_index_path で設定ファイル対応）
    let config = crate::config::load_config(Path::new(".")).ok();
    let config_index_path = config.as_ref().and_then(|c| c.index.path.as_deref());
    let commandindex_dir =
        crate::indexer::resolve_index_path(None, config_index_path, Path::new("."))
            .unwrap_or_else(|_| Path::new(".").join(crate::INDEX_DIR_NAME));
    let tantivy_dir = crate::indexer::index_dir(&commandindex_dir);
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(&commandindex_dir);
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    // 集約（context.rs の merge_related_results と同じロジック）
    let engine = crate::search::related::RelatedSearchEngine::new(&reader, &store);
    let mut results = crate::cli::context::collect_and_merge_related(&engine, &valid_files, limit)?;

    if results.is_empty() {
        eprintln!("No related files found.");
        return Ok(());
    }

    // limit 適用後にスニペット一括付与
    crate::cli::snippet_helper::enrich_related_with_snippets(
        &mut results,
        &reader,
        &snippet_options,
        format,
    );

    // トークン予算適用（--max-tokens）
    let results = if let Some(max_tok) = max_tokens {
        crate::output::token_budget::apply_token_budget(results, max_tok, |r| {
            let mut tokens = crate::output::estimate_tokens(&r.file_path);
            if let Some(ref snippet) = r.snippet {
                tokens = tokens.saturating_add(crate::output::estimate_tokens(snippet));
            }
            tokens
        })
    } else {
        results
    };

    // 既存の format_related_results で出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_related_results(&results, format, &mut handle)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_semantic_search(
    query: &str,
    limit: usize,
    format: OutputFormat,
    tag: Option<&str>,
    filters: &SearchFilters,
    ctx: Option<&SearchContext>,
    max_tokens: Option<usize>,
    snippet_config: SnippetConfig,
    llm_options: &LlmFormatOptions,
) -> Result<(), SearchError> {
    if query.is_empty() {
        return Err(SearchError::InvalidArgument(
            "Semantic search query cannot be empty".to_string(),
        ));
    }

    let (tantivy_dir, embeddings_db_path, config) = if let Some(c) = ctx {
        (c.index_dir(), c.embeddings_db_path(), c.config.clone())
    } else {
        let default_dir = Path::new(".").join(crate::INDEX_DIR_NAME);
        let cfg = load_config(Path::new("."))?;
        (
            crate::indexer::index_dir(&default_dir),
            crate::indexer::embeddings_db_path(&default_dir),
            cfg,
        )
    };
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }
    if !embeddings_db_path.exists() {
        return Err(SearchError::NoEmbeddings);
    }
    let provider = crate::embedding::create_provider(&config.embedding)?;

    // Check embeddings exist
    let emb_store = match crate::embedding::store::EmbeddingStore::open(&embeddings_db_path) {
        Ok(s) => s,
        Err(crate::embedding::store::EmbeddingStoreError::SchemaVersionMismatch { .. }) => {
            return Err(SearchError::SchemaVersionMismatch);
        }
        Err(e) => return Err(e.into()),
    };
    if emb_store.count()? == 0 {
        return Err(SearchError::NoEmbeddings);
    }

    // Generate query embedding
    let query_texts = [query.to_string()];
    let query_embeddings = provider.embed(&query_texts)?;
    let query_embedding = query_embeddings.first().ok_or_else(|| {
        SearchError::InvalidArgument("Failed to generate query embedding".to_string())
    })?;

    // Search similar with oversampling
    let search_output = emb_store.search_similar(query_embedding, limit.saturating_mul(5))?;
    if search_output.should_warn_dimension_mismatch() {
        eprintln!(
            "Warning: {}/{} embeddings were skipped due to dimension mismatch. \
             Consider re-running 'commandindex embed' after model change.",
            search_output.skipped_dimension_mismatch, search_output.total_records
        );
    }
    let similar_results = search_output.results;

    // Enrich with metadata from tantivy
    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let enriched = enrich_with_metadata(&similar_results, &reader)?;

    // Apply filters and truncate to limit
    let final_results: Vec<SemanticSearchResult> = apply_semantic_filters(enriched, tag, filters)
        .into_iter()
        .take(limit)
        .collect();

    // トークン予算適用（--max-tokens）
    let final_results = if let Some(max_tok) = max_tokens {
        crate::output::token_budget::apply_token_budget(final_results, max_tok, |r| {
            crate::output::estimate_tokens(&r.body)
        })
    } else {
        final_results
    };

    if final_results.is_empty() {
        eprintln!("No results found.");
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_semantic_results(
        &final_results,
        format,
        &mut handle,
        snippet_config,
        llm_options,
    )?;
    Ok(())
}

fn enrich_with_metadata(
    similar_results: &[crate::embedding::store::EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<SemanticSearchResult>, SearchError> {
    use std::collections::HashMap;

    // Group by file_path
    let mut groups: HashMap<&str, Vec<&crate::embedding::store::EmbeddingSimilarityResult>> =
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
                // Fallback: use the first section's body/tags/heading_level if available
                let fallback = sections.first();
                enriched.push(SemanticSearchResult {
                    path: item.file_path.clone(),
                    heading: item.section_heading.clone(),
                    similarity: item.similarity,
                    body: fallback.map(|s| s.body.clone()).unwrap_or_default(),
                    tags: fallback.map(|s| s.tags.clone()).unwrap_or_default(),
                    heading_level: fallback.map(|s| s.heading_level).unwrap_or(0),
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
    config: &AppConfig,
    commandindex_dir: &Path,
) -> Result<Vec<crate::indexer::reader::SearchResult>, SearchError> {
    use crate::search::hybrid::{HYBRID_OVERSAMPLING_FACTOR, rrf_merge};

    // 1. EmbeddingStore を開く
    let embeddings_db_path = crate::indexer::embeddings_db_path(commandindex_dir);
    if !embeddings_db_path.exists() {
        eprintln!("[hybrid] Embedding database not found, using BM25 only.");
        return Ok(bm25_results);
    }
    let emb_store = match crate::embedding::store::EmbeddingStore::open(&embeddings_db_path) {
        Ok(s) => s,
        Err(crate::embedding::store::EmbeddingStoreError::SchemaVersionMismatch { .. }) => {
            return Err(SearchError::SchemaVersionMismatch);
        }
        Err(_) => {
            eprintln!("[hybrid] Embedding database not available, using BM25 only.");
            return Ok(bm25_results);
        }
    };

    // 2. Embeddingが存在するか確認
    match emb_store.count() {
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
    let provider = match crate::embedding::create_provider(&config.embedding) {
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
    let search_output = match emb_store.search_similar(
        query_embedding,
        options.limit.saturating_mul(HYBRID_OVERSAMPLING_FACTOR),
    ) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[hybrid] Similarity search failed, using BM25 only.");
            return Ok(bm25_results);
        }
    };
    if search_output.should_warn_dimension_mismatch() {
        eprintln!(
            "Warning: {}/{} embeddings were skipped due to dimension mismatch. \
             Consider re-running 'commandindex embed' after model change.",
            search_output.skipped_dimension_mismatch, search_output.total_records
        );
    }
    let similar_results = search_output.results;

    // 6. セマンティック結果をSearchResult型に変換
    let tantivy_dir = crate::indexer::index_dir(commandindex_dir);
    let reader = match IndexReaderWrapper::open(&tantivy_dir) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("[hybrid] Failed to open index reader, using BM25 only.");
            return Ok(bm25_results);
        }
    };
    let semantic_search_results = match enrich_semantic_to_search_results(&similar_results, &reader)
    {
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

    // 8. RRFマージ（BM25=0件の場合はセマンティックフォールバック）
    if bm25_results.is_empty() && !filtered_semantic.is_empty() {
        eprintln!("[hybrid] BM25 returned 0 results, using semantic-only results.");
        Ok(crate::search::hybrid::semantic_fallback(
            &filtered_semantic,
            &similar_results,
            options.limit,
        ))
    } else {
        Ok(rrf_merge(&bm25_results, &filtered_semantic, options.limit))
    }
}

/// セマンティック検索結果をSearchResult型に変換する（ハイブリッド検索用）
/// 実装は `crate::search::semantic::enrich_semantic_to_search_results` に移動済み。
fn enrich_semantic_to_search_results(
    semantic_results: &[crate::embedding::store::EmbeddingSimilarityResult],
    reader: &IndexReaderWrapper,
) -> Result<Vec<crate::indexer::reader::SearchResult>, SearchError> {
    Ok(crate::search::semantic::enrich_semantic_to_search_results(
        semantic_results,
        reader,
    )?)
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

/// シンボル検索結果のトークン数を推定する（非再帰版）
fn estimate_symbol_result_tokens(r: &SymbolSearchResult) -> usize {
    let text = format!("{} {} {}", r.name, r.kind, r.file_path);
    let children_tokens: usize = r
        .children
        .iter()
        .map(|c| {
            let child_text = format!("{} {} {}", c.name, c.kind, c.file_path);
            crate::output::estimate_tokens(&child_text)
        })
        .sum();
    crate::output::estimate_tokens(&text) + children_tokens
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

/// Reranking を試行する。失敗時は元の結果と RerankStatus を返す。
fn try_rerank(
    results: Vec<crate::indexer::reader::SearchResult>,
    query: &str,
    rerank_top: usize,
    config: &AppConfig,
) -> (Vec<crate::indexer::reader::SearchResult>, RerankStatus) {
    // 1. Use config's rerank settings
    let rerank_config = &config.rerank;

    // 2. Provider生成
    let provider = match crate::rerank::ollama::create_rerank_provider(rerank_config) {
        Ok(p) => p,
        Err(e) => {
            return (
                results,
                RerankStatus::Skipped {
                    reason: e.to_string(),
                },
            );
        }
    };

    // 3. 候補を RerankCandidate に変換（上位 rerank_top 件）
    let candidates: Vec<crate::rerank::RerankCandidate> = results
        .iter()
        .take(rerank_top)
        .enumerate()
        .map(|(i, r)| crate::rerank::RerankCandidate {
            document_text: crate::rerank::build_document_text(&r.heading, &r.body),
            original_index: i,
        })
        .collect();

    // 4. Rerank実行
    let (rerank_results, status) = match provider.rerank(query, &candidates) {
        Ok(r) => (r, RerankStatus::Applied),
        Err(RerankError::PartialTimeout {
            results: partial,
            scored,
            total,
        }) => {
            if partial.is_empty() {
                return (
                    results,
                    RerankStatus::Skipped {
                        reason: format!("Timeout: no candidates scored (0 of {total})"),
                    },
                );
            }
            (
                partial,
                RerankStatus::AppliedPartially {
                    warning: format!("Timeout: scored {scored} of {total} candidates"),
                },
            )
        }
        Err(e) => {
            return (
                results,
                RerankStatus::Skipped {
                    reason: e.to_string(),
                },
            );
        }
    };

    // 5. Rerankされた順序でSearchResultを再構築（範囲外indexはスキップ）
    let mut reranked: Vec<crate::indexer::reader::SearchResult> = rerank_results
        .iter()
        .filter_map(|rr| {
            if rr.index >= results.len() {
                return None;
            }
            results.get(rr.index).map(|sr| {
                let mut new_sr = sr.clone();
                new_sr.score = rr.score;
                new_sr
            })
        })
        .collect();

    // rerank対象外の結果を末尾に追加
    let reranked_indices: std::collections::HashSet<usize> = rerank_results
        .iter()
        .filter(|r| r.index < results.len())
        .map(|r| r.index)
        .collect();
    for (i, r) in results.iter().enumerate() {
        if !reranked_indices.contains(&i) {
            reranked.push(r.clone());
        }
    }

    (reranked, status)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rerank_error_hint_all_variants() {
        // Verify all RerankError variants have a hint
        let errors: Vec<RerankError> = vec![
            RerankError::ModelNotFound("llama3".to_string()),
            RerankError::NetworkError("connection refused".to_string()),
            RerankError::Timeout,
            RerankError::ApiError {
                status: 500,
                message: "internal".to_string(),
            },
            RerankError::InvalidResponse("bad json".to_string()),
            RerankError::ConfigError("missing field".to_string()),
            RerankError::PartialTimeout {
                results: vec![],
                scored: 0,
                total: 5,
            },
        ];
        for err in &errors {
            let hint = rerank_error_hint(err);
            assert!(
                !hint.is_empty(),
                "Hint should not be empty for error: {err}"
            );
        }
    }

    #[test]
    fn test_sanitize_reason_removes_control_chars() {
        assert_eq!(sanitize_reason("hello\x00world"), "hello world");
        assert_eq!(sanitize_reason("line1\nline2"), "line1 line2");
        assert_eq!(sanitize_reason("normal text"), "normal text");
    }

    #[test]
    fn test_build_rerank_stdout_prefix_json_skipped() {
        let status = RerankStatus::Skipped {
            reason: "Network error: connection refused".to_string(),
        };
        let prefix = build_rerank_stdout_prefix(&status, OutputFormat::Json);
        assert!(prefix.is_some());
        let json: serde_json::Value = serde_json::from_str(&prefix.unwrap()).unwrap();
        assert_eq!(json["type"], "metadata");
        assert_eq!(json["rerank_status"], "skipped");
        assert!(json["rerank_warnings"].is_array());
    }

    #[test]
    fn test_build_rerank_stdout_prefix_json_partial() {
        let status = RerankStatus::AppliedPartially {
            warning: "Timeout: scored 3 of 10 candidates".to_string(),
        };
        let prefix = build_rerank_stdout_prefix(&status, OutputFormat::Json);
        assert!(prefix.is_some());
        let json: serde_json::Value = serde_json::from_str(&prefix.unwrap()).unwrap();
        assert_eq!(json["type"], "metadata");
        assert_eq!(json["rerank_status"], "partial");
    }

    #[test]
    fn test_build_rerank_stdout_prefix_llm_skipped() {
        let status = RerankStatus::Skipped {
            reason: "Model not found: llama3".to_string(),
        };
        let prefix = build_rerank_stdout_prefix(&status, OutputFormat::Llm);
        assert!(prefix.is_some());
        let prefix = prefix.unwrap();
        assert!(prefix.starts_with("<!-- rerank skipped:"));
        assert!(prefix.ends_with("-->"));
    }

    #[test]
    fn test_build_rerank_stdout_prefix_llm_partial() {
        let status = RerankStatus::AppliedPartially {
            warning: "Timeout: scored 3 of 10 candidates".to_string(),
        };
        let prefix = build_rerank_stdout_prefix(&status, OutputFormat::Llm);
        assert!(prefix.is_some());
        let prefix = prefix.unwrap();
        assert!(prefix.starts_with("<!-- rerank warning:"));
    }

    #[test]
    fn test_build_rerank_stdout_prefix_human_returns_none() {
        let status = RerankStatus::Skipped {
            reason: "test".to_string(),
        };
        assert!(build_rerank_stdout_prefix(&status, OutputFormat::Human).is_none());
    }

    #[test]
    fn test_build_rerank_stdout_prefix_applied_returns_none() {
        assert!(build_rerank_stdout_prefix(&RerankStatus::Applied, OutputFormat::Json).is_none());
    }

    #[test]
    fn test_build_rerank_stderr_message_human_skipped() {
        let status = RerankStatus::Skipped {
            reason: "Network error: connection refused".to_string(),
        };
        let msg = build_rerank_stderr_message(&status, OutputFormat::Human);
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert!(msg.contains("[rerank] Reranking skipped:"));
        assert!(msg.contains("[rerank] Hint:"));
    }

    #[test]
    fn test_build_rerank_stderr_message_human_partial() {
        let status = RerankStatus::AppliedPartially {
            warning: "Timeout: scored 3 of 10 candidates".to_string(),
        };
        let msg = build_rerank_stderr_message(&status, OutputFormat::Human);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("[rerank] Warning:"));
    }

    #[test]
    fn test_build_rerank_stderr_message_json_returns_none() {
        let status = RerankStatus::Skipped {
            reason: "test".to_string(),
        };
        assert!(build_rerank_stderr_message(&status, OutputFormat::Json).is_none());
    }
}

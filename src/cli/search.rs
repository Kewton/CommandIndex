use std::fmt;
use std::path::Path;

use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchFilters, SearchOptions};
use crate::indexer::symbol_store::{SymbolInfo, SymbolStore, SymbolStoreError};
use crate::output::{self, OutputError, OutputFormat, SymbolSearchResult};

#[derive(Debug)]
pub enum SearchError {
    IndexNotFound,
    Reader(ReaderError),
    Output(OutputError),
    SymbolStore(SymbolStoreError),
    SymbolDbNotFound,
    InvalidArgument(String),
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

impl From<SymbolStoreError> for SearchError {
    fn from(e: SymbolStoreError) -> Self {
        SearchError::SymbolStore(e)
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

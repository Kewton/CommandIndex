use std::fmt;
use std::path::Path;

use crate::indexer::reader::{IndexReaderWrapper, ReaderError, SearchFilters, SearchOptions};
use crate::output::{self, OutputError, OutputFormat};

#[derive(Debug)]
pub enum SearchError {
    IndexNotFound,
    Reader(ReaderError),
    Output(OutputError),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::IndexNotFound => {
                write!(f, "Index not found. Run `commandindex index` first.")
            }
            SearchError::Reader(e) => write!(f, "{e}"),
            SearchError::Output(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SearchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SearchError::IndexNotFound => None,
            SearchError::Reader(e) => Some(e),
            SearchError::Output(e) => Some(e),
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

use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::{OutputError, RelatedSearchResult, SemanticSearchResult, SymbolSearchResult};

/// Path形式で検索結果を出力する（重複除去）
pub fn format_path(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for result in results {
        if seen.insert(&result.path) {
            writeln!(writer, "{}", result.path)?;
        }
    }
    Ok(())
}

/// セマンティック検索結果をpath形式で出力する（重複除去）
pub fn format_semantic_path(
    results: &[SemanticSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for result in results {
        if seen.insert(&result.path) {
            writeln!(writer, "{}", result.path)?;
        }
    }
    Ok(())
}

/// 関連検索結果をpath形式で出力する（重複除去）
pub fn format_related_path(
    results: &[RelatedSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for result in results {
        if seen.insert(&result.file_path) {
            writeln!(writer, "{}", result.file_path)?;
        }
    }
    Ok(())
}

/// シンボル検索結果をpath:line形式で出力する（重複除去）
pub fn format_symbol_path(
    results: &[SymbolSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for result in results {
        let entry = format!("{}:{}", result.file_path, result.line_start);
        if seen.insert(entry.clone()) {
            writeln!(writer, "{entry}")?;
        }
    }
    Ok(())
}

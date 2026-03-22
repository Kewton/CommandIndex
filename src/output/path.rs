use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::{
    OutputError, RelatedSearchResult, SemanticSearchResult, SymbolSearchResult,
    WorkspaceSearchResult,
};

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

/// ワークスペース横断検索結果をpath形式で出力する（重複除去キーは(repository, path)）
pub fn format_workspace_path(
    results: &[WorkspaceSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for ws_result in results {
        let key = (ws_result.repository.clone(), ws_result.result.path.clone());
        if seen.insert(key) {
            writeln!(
                writer,
                "[{}] {}",
                ws_result.repository, ws_result.result.path
            )?;
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

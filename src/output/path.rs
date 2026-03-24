use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::{
    DiffResult, OutputError, RelatedSearchResult, SemanticSearchResult, SymbolSearchResult,
    WorkspaceSearchResult,
};

/// Remove control characters (except tab) from a string for safe terminal output.
fn sanitize_path(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}

/// Path形式で検索結果を出力する（重複除去）
pub fn format_path(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for result in results {
        if seen.insert(&result.path) {
            writeln!(writer, "{}", sanitize_path(&result.path))?;
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
                sanitize_path(&ws_result.repository),
                sanitize_path(&ws_result.result.path)
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
            writeln!(writer, "{}", sanitize_path(&result.path))?;
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
            writeln!(writer, "{}", sanitize_path(&result.file_path))?;
        }
    }
    Ok(())
}

/// impact 結果をpath形式で出力する（重複除去）
pub fn format_impact_path(
    result: &crate::output::ImpactResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for file in &result.impacted_files {
        if seen.insert(&file.file_path) {
            writeln!(writer, "{}", sanitize_path(&file.file_path))?;
        }
    }
    Ok(())
}

/// before-change 結果をpath形式で出力する（重複除去）
pub fn format_before_change_path(
    result: &crate::output::BeforeChangeResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let mut seen = std::collections::HashSet::new();
    for finding in &result.findings {
        if seen.insert(&finding.doc_path) {
            writeln!(writer, "{}", sanitize_path(&finding.doc_path))?;
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
            writeln!(writer, "{}", sanitize_path(&entry))?;
        }
    }
    Ok(())
}

/// Diff結果をpath形式で出力する（overlapのみ）
pub fn format_diff_path(result: &DiffResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    for path in &result.overlap {
        writeln!(writer, "{}", sanitize_path(path))?;
    }
    Ok(())
}

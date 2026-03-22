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

/// impact 結果をpath形式で出力する（union, max スコア降順, strip_control_chars）
pub fn format_impact_path(
    result: &crate::output::ImpactResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    // Collect all impacted paths with max score
    let mut path_scores: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    for per_file in &result.impact {
        for rel in &per_file.related {
            let entry = path_scores.entry(rel.path.clone()).or_insert(0.0);
            if rel.score > *entry {
                *entry = rel.score;
            }
        }
    }

    // Sort by max score descending
    let mut paths: Vec<(String, f32)> = path_scores.into_iter().collect();
    paths.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    for (path, _) in paths {
        writeln!(writer, "{}", crate::output::strip_control_chars(&path))?;
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

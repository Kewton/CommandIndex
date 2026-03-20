use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::{OutputError, SymbolSearchResult, parse_tags};

/// JSONL形式で検索結果を出力する
pub fn format_json(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError> {
    for result in results {
        let tags: Vec<&str> = parse_tags(&result.tags);
        let json_value = serde_json::json!({
            "path": result.path,
            "heading": result.heading,
            "heading_level": result.heading_level,
            "body": result.body,
            "tags": tags,
            "line_start": result.line_start,
            "score": result.score,
        });
        serde_json::to_writer(&mut *writer, &json_value)?;
        writeln!(writer)?;
    }
    Ok(())
}

/// シンボル検索結果をJSONL形式で出力する
pub fn format_symbol_json(
    results: &[SymbolSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    for result in results {
        let json_value = serde_json::json!({
            "name": result.name,
            "kind": result.kind.to_lowercase(),
            "path": result.file_path,
            "line_start": result.line_start,
            "line_end": result.line_end,
        });
        serde_json::to_writer(&mut *writer, &json_value)?;
        writeln!(writer)?;
        for child in &result.children {
            let child_json = serde_json::json!({
                "name": child.name,
                "kind": child.kind.to_lowercase(),
                "path": child.file_path,
                "line_start": child.line_start,
                "line_end": child.line_end,
                "parent": result.name,
            });
            serde_json::to_writer(&mut *writer, &child_json)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

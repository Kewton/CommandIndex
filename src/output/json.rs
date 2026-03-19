use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::{OutputError, parse_tags};

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

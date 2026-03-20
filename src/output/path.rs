use std::io::Write;

use crate::indexer::reader::SearchResult;
use crate::output::OutputError;

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

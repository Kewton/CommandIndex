use std::io::Write;

use colored::Colorize;

use crate::indexer::reader::SearchResult;
use crate::output::{OutputError, parse_tags, strip_control_chars, truncate_body};

/// Human形式で検索結果を出力する
pub fn format_human(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError> {
    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }

        // パス:行番号 [見出し]
        let location = format!("{}:{}", result.path, result.line_start);
        let heading_display = format!(
            "[{} {}]",
            "#".repeat(result.heading_level as usize),
            strip_control_chars(&result.heading)
        );
        writeln!(writer, "{} {}", location.green(), heading_display.bold())?;

        // 本文スニペット（最大2行）
        let snippet = truncate_body(&strip_control_chars(&result.body), 2, 120);
        for line in snippet.lines() {
            writeln!(writer, "  {line}")?;
        }

        // タグ（存在する場合のみ）
        let tags = parse_tags(&result.tags);
        if !tags.is_empty() {
            let tags_str = tags.join(", ");
            writeln!(writer, "  {}", format!("Tags: {tags_str}").dimmed())?;
        }
    }
    Ok(())
}

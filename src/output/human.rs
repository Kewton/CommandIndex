use std::io::Write;

use colored::Colorize;

use crate::indexer::reader::SearchResult;
use crate::output::{
    OutputError, RelatedSearchResult, SnippetConfig, SymbolSearchResult, parse_tags,
    strip_control_chars, truncate_body,
};

/// Human形式で検索結果を出力する
pub fn format_human(
    results: &[SearchResult],
    writer: &mut dyn Write,
    snippet_config: SnippetConfig,
) -> Result<(), OutputError> {
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

        // 本文スニペット
        let body_cleaned = strip_control_chars(&result.body);
        let snippet = if snippet_config.lines == 0 && snippet_config.chars == 0 {
            body_cleaned
        } else if snippet_config.lines == 0 {
            truncate_body(&body_cleaned, usize::MAX, snippet_config.chars)
        } else if snippet_config.chars == 0 {
            truncate_body(&body_cleaned, snippet_config.lines, usize::MAX)
        } else {
            truncate_body(&body_cleaned, snippet_config.lines, snippet_config.chars)
        };
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

/// 関連検索結果をhuman形式で出力する
pub fn format_related_human(
    results: &[RelatedSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let path = strip_control_chars(&result.file_path);
        let score = format!("{:.2}", result.score);
        let relations: Vec<String> = result
            .relation_types
            .iter()
            .map(|r| match r {
                crate::output::RelationType::MarkdownLink => "link".to_string(),
                crate::output::RelationType::ImportDependency => "import".to_string(),
                crate::output::RelationType::TagMatch { matched_tags } => {
                    format!("tags:{}", matched_tags.join(","))
                }
                crate::output::RelationType::PathSimilarity => "path".to_string(),
                crate::output::RelationType::DirectoryProximity => "dir".to_string(),
            })
            .collect();
        writeln!(
            writer,
            "{} {} [{}]",
            path.green(),
            format!("(score: {score})").dimmed(),
            relations.join(", ")
        )?;
    }
    Ok(())
}

/// シンボル検索結果をhuman形式で出力する
pub fn format_symbol_human(
    results: &[SymbolSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let kind = strip_control_chars(&result.kind).to_lowercase();
        let name = strip_control_chars(&result.name);
        let path = strip_control_chars(&result.file_path);
        writeln!(writer, "{} {}", format!("[{kind}]").green(), name.bold())?;
        writeln!(writer, "  {path}:{}-{}", result.line_start, result.line_end)?;

        for child in &result.children {
            let child_kind = strip_control_chars(&child.kind).to_lowercase();
            let child_name = strip_control_chars(&child.name);
            writeln!(
                writer,
                "    [{child_kind}] {child_name} (line {}-{})",
                child.line_start, child.line_end
            )?;
        }
    }
    Ok(())
}

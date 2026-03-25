use std::io::Write;

use colored::Colorize;

use crate::indexer::reader::SearchResult;
use crate::output::{
    BeforeChangeResult, DiffResult, OutputError, RelatedSearchResult, SemanticSearchResult,
    SnippetConfig, SymbolSearchResult, WorkspaceSearchResult, parse_tags, strip_control_chars,
    truncate_body,
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

/// ワークスペース横断検索結果をhuman形式で出力する
pub fn format_workspace_human(
    results: &[WorkspaceSearchResult],
    writer: &mut dyn Write,
    snippet_config: SnippetConfig,
) -> Result<(), OutputError> {
    for (i, ws_result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }

        let result = &ws_result.result;
        let repo = strip_control_chars(&ws_result.repository);

        // [alias] パス:行番号 [見出し]
        let location = format!("[{}] {}:{}", repo, result.path, result.line_start);
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
                crate::output::RelationType::KnowledgeGraph => "knowledge".to_string(),
            })
            .collect();
        writeln!(
            writer,
            "{} {} [{}]",
            path.green(),
            format!("(score: {score})").dimmed(),
            relations.join(", ")
        )?;
        if let Some(ref snippet) = result.snippet
            && !snippet.is_empty()
        {
            for line in snippet.lines() {
                writeln!(writer, "  {}", line.dimmed())?;
            }
        }
    }
    Ok(())
}

/// impact 結果をhuman形式で出力する
pub fn format_impact_human(
    result: &crate::output::ImpactResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    writeln!(
        writer,
        "{}",
        format!(
            "Impact analysis: {} input file(s), {} impacted file(s)",
            result.total_input_files, result.total_impacted_files
        )
        .bold()
    )?;
    writeln!(writer)?;

    for (i, file) in result.impacted_files.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let path = strip_control_chars(&file.file_path);
        let score = format!("{:.2}", file.score);
        let relations = file.relation_types.join(", ");
        writeln!(
            writer,
            "{} {} [{}]",
            path.green(),
            format!("(score: {score})").dimmed(),
            relations
        )?;
        if !file.impacted_by.is_empty() {
            let by = file
                .impacted_by
                .iter()
                .map(|s| strip_control_chars(s))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(writer, "  {}", format!("impacted by: {by}").dimmed())?;
        }
        if let Some(ref snippet) = file.snippet
            && !snippet.is_empty()
        {
            for line in snippet.lines() {
                writeln!(writer, "  {}", line.dimmed())?;
            }
        }
    }
    Ok(())
}

/// why 結果をhuman形式で出力する
pub fn format_why_human(
    result: &crate::output::WhyResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let file = strip_control_chars(&result.file_path);
    writeln!(writer, "{}", format!("Why: {file}").bold())?;
    writeln!(writer)?;

    if result.issues.is_empty() {
        writeln!(writer, "  No related issues found.")?;
        return Ok(());
    }

    for (i, issue) in result.issues.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let issue_label = match &issue.title {
            Some(t) => format!(
                "Issue #{} ({})",
                strip_control_chars(&issue.issue_number),
                strip_control_chars(t)
            ),
            None => format!("Issue #{}", strip_control_chars(&issue.issue_number)),
        };
        writeln!(writer, "  {}", issue_label.green())?;

        for doc in &issue.documents {
            let relation_label = relation_display_label(&doc.relation, doc.doc_subtype.as_ref());
            let doc_path = strip_control_chars(&doc.file_path);
            writeln!(
                writer,
                "    {} {}",
                format!("[{relation_label}]").dimmed(),
                doc_path
            )?;
        }
        if let Some(count) = issue.modifies_count {
            writeln!(writer, "    [modifies] modifies: {count} files")?;
        }
    }
    Ok(())
}

/// relation文字列を人間が読みやすいラベルに変換する
/// doc_subtype が存在する場合はそちらを優先する
pub(crate) fn relation_display_label<'a>(
    relation: &'a str,
    doc_subtype: Option<&crate::indexer::knowledge::DocSubtype>,
) -> &'a str {
    if let Some(subtype) = doc_subtype {
        return subtype.display_label_en();
    }
    match relation {
        "has_design" => "design",
        "has_review" => "review",
        "has_workplan" => "workplan",
        "has_progress" => "progress",
        other => other,
    }
}

/// セマンティック検索結果をhuman形式で出力する
pub fn format_semantic_human(
    results: &[SemanticSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }

        let similarity = format!("[{:.2}]", result.similarity);
        let path = strip_control_chars(&result.path);
        let heading = strip_control_chars(&result.heading);
        writeln!(
            writer,
            "{} {} > {}",
            similarity.green(),
            path.green(),
            heading.bold()
        )?;

        // Body snippet (max 2 lines)
        let snippet = truncate_body(&strip_control_chars(&result.body), 2, 120);
        for line in snippet.lines() {
            writeln!(writer, "  {line}")?;
        }

        // Tags
        let tags = parse_tags(&result.tags);
        if !tags.is_empty() {
            let tags_str = tags.join(", ");
            writeln!(writer, "  {}", format!("Tags: {tags_str}").dimmed())?;
        }
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

/// before-change 結果をhuman形式で出力する
pub fn format_before_change_human(
    result: &BeforeChangeResult,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    let file = strip_control_chars(&result.file_path);
    writeln!(
        writer,
        "{}",
        format!(
            "Before-change: {} ({} issue(s), {} finding(s))",
            file,
            result.total_issues,
            result.findings.len()
        )
        .bold()
    )?;

    if result.findings.is_empty() {
        writeln!(writer, "  No related design documents found.")?;
        return Ok(());
    }

    if result.displayed_issues < result.total_issues {
        writeln!(
            writer,
            "  showing {} of {} issues (limited by --limit)",
            result.displayed_issues, result.total_issues
        )?;
    }

    writeln!(writer)?;

    for (i, finding) in result.findings.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let doc_path = strip_control_chars(&finding.doc_path);
        let relation = strip_control_chars(&finding.relation);
        let issue = strip_control_chars(&finding.issue_number);
        let sim_str = finding
            .similarity
            .map(|s| format!(" (similarity: {s:.2})"))
            .unwrap_or_default();
        writeln!(
            writer,
            "{} {} [#{issue}, {relation}]",
            doc_path.green(),
            sim_str.dimmed(),
        )?;
        if let Some(ref title) = finding.doc_title {
            let title_cleaned = strip_control_chars(title);
            writeln!(writer, "  {}", title_cleaned.dimmed())?;
        }
    }
    Ok(())
}

/// Diff結果をhuman形式で出力する
pub fn format_diff_human(result: &DiffResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    let file_a = strip_control_chars(&result.file_a);
    let file_b = strip_control_chars(&result.file_b);

    writeln!(writer, "=== Diff: {} vs {} ===", file_a, file_b)?;
    writeln!(writer)?;

    writeln!(writer, "Only in {}:", file_a)?;
    if result.only_a.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.only_a {
            writeln!(writer, "  {}", strip_control_chars(path))?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "Only in {}:", file_b)?;
    if result.only_b.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.only_b {
            writeln!(writer, "  {}", strip_control_chars(path))?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "Overlap ({} files):", result.overlap.len())?;
    if result.overlap.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for path in &result.overlap {
            writeln!(writer, "  {}", strip_control_chars(path))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::knowledge::DocSubtype;

    #[test]
    fn test_relation_display_label_with_doc_subtype() {
        assert_eq!(
            relation_display_label("has_progress", Some(&DocSubtype::ProgressReport)),
            "progress"
        );
        assert_eq!(
            relation_display_label("has_review", Some(&DocSubtype::IssueReview)),
            "review"
        );
        assert_eq!(
            relation_display_label("has_review", Some(&DocSubtype::DesignReview)),
            "review"
        );
        assert_eq!(
            relation_display_label("has_design", Some(&DocSubtype::DesignPolicy)),
            "design"
        );
        assert_eq!(
            relation_display_label("has_workplan", Some(&DocSubtype::WorkPlan)),
            "workplan"
        );
        assert_eq!(
            relation_display_label("has_review", Some(&DocSubtype::StageReview)),
            "review"
        );
    }

    #[test]
    fn test_relation_display_label_without_doc_subtype() {
        assert_eq!(relation_display_label("has_design", None), "design");
        assert_eq!(relation_display_label("has_review", None), "review");
        assert_eq!(relation_display_label("has_workplan", None), "workplan");
        assert_eq!(relation_display_label("has_progress", None), "progress");
        assert_eq!(relation_display_label("modifies", None), "modifies");
    }
}

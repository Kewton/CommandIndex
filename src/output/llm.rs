use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::indexer::reader::SearchResult;
use crate::output::{
    DiffResult, ImpactResult, OutputError, RelatedSearchResult, SemanticSearchResult,
    SymbolSearchResult, WorkspaceSearchResult, estimate_tokens, strip_control_chars,
};

/// body内のバッククォート連続数を検査し、フェンスに必要なバッククォート数を返す
fn fence_backticks(body: &str) -> usize {
    let mut max_run = 0usize;
    let mut current_run = 0usize;
    for ch in body.chars() {
        if ch == '`' {
            current_run += 1;
            if current_run > max_run {
                max_run = current_run;
            }
        } else {
            current_run = 0;
        }
    }
    if max_run < 3 { 3 } else { max_run + 1 }
}

/// ファイル拡張子からコードフェンスの言語指定を推定する
fn detect_language(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("rb") => "ruby",
        Some("sh") | Some("bash") => "bash",
        Some("sql") => "sql",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("html") | Some("htm") => "html",
        Some("css") => "css",
        Some("c") => "c",
        Some("cpp") | Some("cc") | Some("cxx") => "cpp",
        Some("h") | Some("hpp") => "cpp",
        Some("swift") => "swift",
        Some("kt") | Some("kts") => "kotlin",
        Some("scala") => "scala",
        Some("php") => "php",
        Some("r") | Some("R") => "r",
        Some("lua") => "lua",
        Some("zig") => "zig",
        Some("ex") | Some("exs") => "elixir",
        _ => "",
    }
}

/// コードファイルかどうかを判定する
fn is_code_file(path: &str) -> bool {
    let lang = detect_language(path);
    !lang.is_empty() && lang != "markdown"
}

/// 同一パスの結果をグループ化する（出現順保持）
fn group_by_path(results: &[SearchResult]) -> Vec<(&str, Vec<&SearchResult>)> {
    let mut map: HashMap<&str, Vec<&SearchResult>> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for result in results {
        if !map.contains_key(result.path.as_str()) {
            order.push(&result.path);
        }
        map.entry(&result.path).or_default().push(result);
    }
    order
        .into_iter()
        .map(|p| (p, map.remove(p).unwrap()))
        .collect()
}

/// bodyをMarkdown形式で書き出す（コードファイルならフェンス付き）
fn write_body(writer: &mut dyn Write, path: &str, body: &str) -> Result<(), OutputError> {
    let cleaned = strip_control_chars(body);
    if cleaned.is_empty() {
        return Ok(());
    }
    if is_code_file(path) {
        let lang = detect_language(path);
        let backtick_count = fence_backticks(&cleaned);
        let fence: String = "`".repeat(backtick_count);
        writeln!(writer, "{fence}{lang}")?;
        writeln!(writer, "{cleaned}")?;
        writeln!(writer, "{fence}")?;
    } else {
        writeln!(writer, "{cleaned}")?;
    }
    Ok(())
}

/// LLM向けMarkdown形式で検索結果を出力する
pub fn format_llm(results: &[SearchResult], writer: &mut dyn Write) -> Result<(), OutputError> {
    if results.is_empty() {
        return Ok(());
    }

    // 全体のトークン推定を先に計算
    let total_text: String = results
        .iter()
        .map(|r| r.body.as_str())
        .collect::<Vec<_>>()
        .join("");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    let groups = group_by_path(results);
    for (i, (path, items)) in groups.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        writeln!(writer, "## {path}")?;
        for item in items {
            let heading = strip_control_chars(&item.heading);
            if !heading.is_empty() {
                writeln!(writer, "### {heading}")?;
            }
            writeln!(writer)?;
            write_body(writer, path, &item.body)?;
        }
    }
    Ok(())
}

/// ワークスペース横断検索結果をLLM向けMarkdown形式で出力する
pub fn format_workspace_llm(
    results: &[WorkspaceSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    if results.is_empty() {
        return Ok(());
    }

    let total_text: String = results
        .iter()
        .map(|r| r.result.body.as_str())
        .collect::<Vec<_>>()
        .join("");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    for (i, ws_result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let repo = strip_control_chars(&ws_result.repository);
        let path = &ws_result.result.path;
        writeln!(writer, "## [{repo}] {path}")?;

        let heading = strip_control_chars(&ws_result.result.heading);
        if !heading.is_empty() {
            writeln!(writer, "### {heading}")?;
        }
        writeln!(writer)?;
        write_body(writer, path, &ws_result.result.body)?;
    }
    Ok(())
}

/// セマンティック検索結果をLLM向けMarkdown形式で出力する
pub fn format_semantic_llm(
    results: &[SemanticSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    if results.is_empty() {
        return Ok(());
    }

    let total_text: String = results
        .iter()
        .map(|r| r.body.as_str())
        .collect::<Vec<_>>()
        .join("");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        let path = strip_control_chars(&result.path);
        writeln!(writer, "## {path}")?;

        let heading = strip_control_chars(&result.heading);
        if !heading.is_empty() {
            writeln!(writer, "### {heading}")?;
        }
        writeln!(writer)?;
        write_body(writer, &result.path, &result.body)?;
    }
    Ok(())
}

/// シンボル検索結果をLLM向けMarkdown形式で出力する
pub fn format_symbol_llm(
    results: &[SymbolSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    if results.is_empty() {
        return Ok(());
    }

    // シンボル名のテキスト量でトークン推定
    let total_text: String = results
        .iter()
        .map(|r| format!("{} {} {}", r.file_path, r.kind, r.name))
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    // ファイルパスでグループ化
    let mut map: HashMap<&str, Vec<&SymbolSearchResult>> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for result in results {
        if !map.contains_key(result.file_path.as_str()) {
            order.push(&result.file_path);
        }
        map.entry(&result.file_path).or_default().push(result);
    }

    for (i, path) in order.iter().enumerate() {
        if i > 0 {
            writeln!(writer)?;
        }
        writeln!(writer, "## {path}")?;
        for sym in map.get(path).unwrap() {
            let kind = strip_control_chars(&sym.kind).to_lowercase();
            let name = strip_control_chars(&sym.name);
            writeln!(
                writer,
                "- `{kind}` **{name}** (L{}-L{})",
                sym.line_start, sym.line_end
            )?;
            for child in &sym.children {
                let child_kind = strip_control_chars(&child.kind).to_lowercase();
                let child_name = strip_control_chars(&child.name);
                writeln!(
                    writer,
                    "  - `{child_kind}` **{child_name}** (L{}-L{})",
                    child.line_start, child.line_end
                )?;
            }
        }
    }
    Ok(())
}

/// 関連検索結果をLLM向けMarkdown形式で出力する
pub fn format_related_llm(
    results: &[RelatedSearchResult],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    if results.is_empty() {
        return Ok(());
    }

    let total_text: String = results
        .iter()
        .map(|r| r.file_path.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    for result in results {
        let path = strip_control_chars(&result.file_path);
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
        writeln!(writer, "- {path} ({})", relations.join(", "))?;
    }
    Ok(())
}

/// Diff結果をLLM向けMarkdown形式で出力する
pub fn format_diff_llm(result: &DiffResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    let total_text = format!(
        "{} {} {}",
        result.only_a.join(" "),
        result.only_b.join(" "),
        result.overlap.join(" ")
    );
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    let file_a = strip_control_chars(&result.file_a);
    let file_b = strip_control_chars(&result.file_b);
    writeln!(writer, "## Diff: {file_a} vs {file_b}")?;
    writeln!(writer)?;

    writeln!(writer, "### Only in {file_a}")?;
    if result.only_a.is_empty() {
        writeln!(writer, "(none)")?;
    } else {
        for path in &result.only_a {
            writeln!(writer, "- {}", strip_control_chars(path))?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "### Only in {file_b}")?;
    if result.only_b.is_empty() {
        writeln!(writer, "(none)")?;
    } else {
        for path in &result.only_b {
            writeln!(writer, "- {}", strip_control_chars(path))?;
        }
    }
    writeln!(writer)?;

    writeln!(writer, "### Overlap ({} files)", result.overlap.len())?;
    if result.overlap.is_empty() {
        writeln!(writer, "(none)")?;
    } else {
        for path in &result.overlap {
            writeln!(writer, "- {}", strip_control_chars(path))?;
        }
    }
    Ok(())
}

/// Impact結果をLLM向けMarkdown形式で出力する
pub fn format_impact_llm(result: &ImpactResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    let total_text: String = result
        .impacted_files
        .iter()
        .map(|f| f.file_path.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = estimate_tokens(&total_text);
    writeln!(writer, "<!-- estimated tokens: ~{tokens} -->")?;

    writeln!(
        writer,
        "## Impact: {} input file(s) → {} impacted file(s)",
        result.total_input_files, result.total_impacted_files
    )?;
    writeln!(writer)?;

    for file in &result.impacted_files {
        let path = strip_control_chars(&file.file_path);
        let relations = file.relation_types.join(", ");
        let impacted_by = file
            .impacted_by
            .iter()
            .map(|s| strip_control_chars(s))
            .collect::<Vec<_>>()
            .join(", ");
        if impacted_by.is_empty() {
            writeln!(writer, "- {path} ({relations})")?;
        } else {
            writeln!(writer, "- {path} ({relations}) ← {impacted_by}")?;
        }
    }
    Ok(())
}

use std::fmt;
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::indexer::knowledge::{DocSubtype, IssueDocumentEntry, KnowledgeRelation};
use crate::indexer::symbol_store::{IssueListRow, SymbolStore, SymbolStoreError};
use crate::output::{OutputError, OutputFormat, strip_control_chars};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum IssueCommandError {
    SymbolStore(SymbolStoreError),
    Output(OutputError),
    NotFound { issue_number: u64 },
    CorruptedMetadata { file_path: String, reason: String },
}

impl fmt::Display for IssueCommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SymbolStore(e) => write!(f, "{e}"),
            Self::Output(e) => write!(f, "{e}"),
            Self::NotFound { issue_number } => {
                write!(f, "No documents found for issue #{issue_number}")
            }
            Self::CorruptedMetadata { file_path, reason } => {
                write!(f, "Corrupted metadata for {file_path}: {reason}")
            }
        }
    }
}

impl std::error::Error for IssueCommandError {}

impl From<SymbolStoreError> for IssueCommandError {
    fn from(e: SymbolStoreError) -> Self {
        Self::SymbolStore(e)
    }
}

impl From<OutputError> for IssueCommandError {
    fn from(e: OutputError) -> Self {
        Self::Output(e)
    }
}

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentsResult {
    pub issue_number: String,
    pub documents: Vec<IssueDocumentEntry>,
}

impl IssueDocumentsResult {
    /// カテゴリ別にグループ化した結果を返す
    pub fn grouped(&self) -> Vec<(&'static str, Vec<&IssueDocumentEntry>)> {
        let categories = ["設計", "レビュー", "作業計画", "進捗レポート"];
        categories
            .iter()
            .filter_map(|&cat| {
                let docs: Vec<_> = self
                    .documents
                    .iter()
                    .filter(|d| display_label(&d.doc_subtype) == cat)
                    .collect();
                if docs.is_empty() {
                    None
                } else {
                    Some((cat, docs))
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Issue list types
// ---------------------------------------------------------------------------

/// CLI出力用のIssue一覧エントリ
#[derive(Debug, Clone, Serialize)]
pub struct IssueListEntry {
    pub number: u64,
    pub doc_count: u32,
    pub label: String,
    pub has_design: bool,
    pub has_review: bool,
    pub has_workplan: bool,
    pub has_progress: bool,
}

// ---------------------------------------------------------------------------
// Label extraction
// ---------------------------------------------------------------------------

static DESIGN_LABEL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"issue-\d+-(.+)-design-policy\.md$").expect("BUG: invalid regex pattern")
});

fn extract_label_from_design_path(path: &str) -> String {
    DESIGN_LABEL_RE
        .captures(path)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn open_symbol_store(commandindex_dir: &Path) -> Result<SymbolStore, IssueCommandError> {
    let db_path = crate::indexer::symbol_db_path(commandindex_dir);
    if !db_path.exists() {
        return Err(IssueCommandError::SymbolStore(SymbolStoreError::Io(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Symbol database not found: {}. Run `commandindex index` first.",
                    db_path.display()
                ),
            ),
        )));
    }
    SymbolStore::open(&db_path).map_err(IssueCommandError::SymbolStore)
}

fn sanitize_label(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect()
}

fn convert_row_to_entry(row: IssueListRow) -> IssueListEntry {
    let label = row
        .design_file_path
        .as_deref()
        .map(extract_label_from_design_path)
        .unwrap_or_default();
    IssueListEntry {
        number: row.number,
        doc_count: row.doc_count,
        label: sanitize_label(&label),
        has_design: row.has_design,
        has_review: row.has_review,
        has_workplan: row.has_workplan,
        has_progress: row.has_progress,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn display_label(subtype: &DocSubtype) -> &'static str {
    match subtype {
        DocSubtype::DesignPolicy => "設計",
        DocSubtype::IssueReview | DocSubtype::DesignReview | DocSubtype::StageReview => "レビュー",
        DocSubtype::WorkPlan => "作業計画",
        DocSubtype::ProgressReport => "進捗レポート",
    }
}

fn sort_order(entry: &IssueDocumentEntry) -> (u8, u8) {
    let relation_order = match entry.relation {
        KnowledgeRelation::HasDesign => 1,
        KnowledgeRelation::HasReview => 2,
        KnowledgeRelation::HasWorkplan => 3,
        KnowledgeRelation::HasProgress => 4,
        KnowledgeRelation::Modifies => 5,
    };
    let subtype_order = match entry.doc_subtype {
        DocSubtype::DesignPolicy => 1,
        DocSubtype::IssueReview => 2,
        DocSubtype::DesignReview => 3,
        DocSubtype::WorkPlan => 4,
        DocSubtype::ProgressReport => 5,
        DocSubtype::StageReview => 6,
    };
    (relation_order, subtype_order)
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run_show(
    issue_number: u64,
    format: OutputFormat,
    commandindex_dir: &Path,
) -> Result<(), IssueCommandError> {
    let store = open_symbol_store(commandindex_dir)?;
    let issue_str = issue_number.to_string();
    let mut documents = store.find_documents_by_issue(&issue_str)?;

    if documents.is_empty() {
        return Err(IssueCommandError::NotFound { issue_number });
    }

    // Sort by relation + subtype order
    documents.sort_by_key(sort_order);

    let result = IssueDocumentsResult {
        issue_number: format!("{issue_number}"),
        documents,
    };

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    format_issue_documents(&result, format, &mut writer)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Run list
// ---------------------------------------------------------------------------

pub fn run_list(format: OutputFormat, commandindex_dir: &Path) -> Result<(), IssueCommandError> {
    let store = open_symbol_store(commandindex_dir)?;
    let rows = store.list_all_issues()?;
    let entries: Vec<IssueListEntry> = rows.into_iter().map(convert_row_to_entry).collect();

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    format_list(&entries, format, &mut writer)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// List formatters
// ---------------------------------------------------------------------------

fn format_list(
    entries: &[IssueListEntry],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => format_list_human(entries, writer),
        OutputFormat::Json => format_list_json(entries, writer),
        OutputFormat::Path => format_list_path(entries, writer),
        OutputFormat::Llm => format_list_llm(entries, writer),
    }
}

fn format_list_human(
    entries: &[IssueListEntry],
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    if entries.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }
    // Calculate width for alignment
    let max_num_width = entries
        .iter()
        .map(|e| format!("{}", e.number).len())
        .max()
        .unwrap_or(1);
    for entry in entries {
        let num_str = format!("{}", entry.number);
        let padding = " ".repeat(max_num_width - num_str.len());
        if entry.label.is_empty() {
            writeln!(
                writer,
                "Issue #{num_str}{padding}  ({} docs)",
                entry.doc_count
            )?;
        } else {
            writeln!(
                writer,
                "Issue #{num_str}{padding}  ({} docs)  {}",
                entry.doc_count,
                strip_control_chars(&entry.label)
            )?;
        }
    }
    writeln!(writer, "Total: {} issues", entries.len())?;
    Ok(())
}

fn format_list_json(entries: &[IssueListEntry], writer: &mut dyn Write) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(entries).map_err(OutputError::Json)?;
    writeln!(writer, "{json}")?;
    Ok(())
}

fn format_list_path(entries: &[IssueListEntry], writer: &mut dyn Write) -> Result<(), OutputError> {
    for entry in entries {
        writeln!(writer, "{}", entry.number)?;
    }
    Ok(())
}

fn format_list_llm(entries: &[IssueListEntry], writer: &mut dyn Write) -> Result<(), OutputError> {
    if entries.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }
    writeln!(
        writer,
        "| Issue | Docs | Label | Design | Review | Workplan | Progress |"
    )?;
    writeln!(
        writer,
        "|-------|------|-------|--------|--------|----------|----------|"
    )?;
    for entry in entries {
        writeln!(
            writer,
            "| #{} | {} | {} | {} | {} | {} | {} |",
            entry.number,
            entry.doc_count,
            strip_control_chars(&entry.label),
            if entry.has_design { "Yes" } else { "No" },
            if entry.has_review { "Yes" } else { "No" },
            if entry.has_workplan { "Yes" } else { "No" },
            if entry.has_progress { "Yes" } else { "No" },
        )?;
    }
    writeln!(writer)?;
    writeln!(writer, "Total: {} issues", entries.len())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Output formatters (show)
// ---------------------------------------------------------------------------

fn format_issue_documents(
    result: &IssueDocumentsResult,
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => format_human(result, writer),
        OutputFormat::Json => format_json(result, writer),
        OutputFormat::Llm => format_llm(result, writer),
        OutputFormat::Path => format_path(result, writer),
    }
}

fn format_human(result: &IssueDocumentsResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    writeln!(
        writer,
        "Issue #{} 関連ドキュメント:",
        strip_control_chars(&result.issue_number)
    )?;
    for (category, docs) in result.grouped() {
        writeln!(writer, "\n  {category}:")?;
        for doc in docs {
            writeln!(writer, "    {}", strip_control_chars(&doc.file_path))?;
        }
    }
    Ok(())
}

fn format_json(result: &IssueDocumentsResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    // Build grouped JSON structure
    let grouped = result.grouped();
    let mut categories = serde_json::Map::new();
    for (category, docs) in &grouped {
        let paths: Vec<&str> = docs.iter().map(|d| d.file_path.as_str()).collect();
        categories.insert(
            (*category).to_string(),
            serde_json::Value::Array(
                paths
                    .into_iter()
                    .map(|p| serde_json::Value::String(p.to_string()))
                    .collect(),
            ),
        );
    }
    let output = serde_json::json!({
        "issue_number": result.issue_number,
        "documents": categories,
    });
    let json = serde_json::to_string_pretty(&output).map_err(OutputError::Json)?;
    writeln!(writer, "{json}")?;
    Ok(())
}

fn format_llm(result: &IssueDocumentsResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    writeln!(
        writer,
        "# Issue #{} 関連ドキュメント",
        strip_control_chars(&result.issue_number)
    )?;
    for (category, docs) in result.grouped() {
        writeln!(writer, "\n## {category}")?;
        for doc in docs {
            writeln!(writer, "- {}", strip_control_chars(&doc.file_path))?;
        }
    }
    Ok(())
}

fn format_path(result: &IssueDocumentsResult, writer: &mut dyn Write) -> Result<(), OutputError> {
    for doc in &result.documents {
        writeln!(writer, "{}", strip_control_chars(&doc.file_path))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_label() {
        assert_eq!(display_label(&DocSubtype::DesignPolicy), "設計");
        assert_eq!(display_label(&DocSubtype::IssueReview), "レビュー");
        assert_eq!(display_label(&DocSubtype::DesignReview), "レビュー");
        assert_eq!(display_label(&DocSubtype::WorkPlan), "作業計画");
        assert_eq!(display_label(&DocSubtype::ProgressReport), "進捗レポート");
        assert_eq!(display_label(&DocSubtype::StageReview), "レビュー");
    }

    #[test]
    fn test_sort_order() {
        let design = IssueDocumentEntry {
            file_path: "a.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
        };
        let review = IssueDocumentEntry {
            file_path: "b.md".to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::IssueReview,
        };
        let workplan = IssueDocumentEntry {
            file_path: "c.md".to_string(),
            relation: KnowledgeRelation::HasWorkplan,
            doc_subtype: DocSubtype::WorkPlan,
        };
        let stage_review = IssueDocumentEntry {
            file_path: "d.md".to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::StageReview,
        };
        assert!(sort_order(&design) < sort_order(&review));
        assert!(sort_order(&review) < sort_order(&workplan));
        assert!(sort_order(&review) < sort_order(&stage_review));
    }

    #[test]
    fn test_grouped() {
        let result = IssueDocumentsResult {
            issue_number: "100".to_string(),
            documents: vec![
                IssueDocumentEntry {
                    file_path: "design.md".to_string(),
                    relation: KnowledgeRelation::HasDesign,
                    doc_subtype: DocSubtype::DesignPolicy,
                },
                IssueDocumentEntry {
                    file_path: "review.md".to_string(),
                    relation: KnowledgeRelation::HasReview,
                    doc_subtype: DocSubtype::IssueReview,
                },
                IssueDocumentEntry {
                    file_path: "progress.md".to_string(),
                    relation: KnowledgeRelation::HasProgress,
                    doc_subtype: DocSubtype::ProgressReport,
                },
                IssueDocumentEntry {
                    file_path: "stage-review.md".to_string(),
                    relation: KnowledgeRelation::HasReview,
                    doc_subtype: DocSubtype::StageReview,
                },
            ],
        };
        let grouped = result.grouped();
        assert_eq!(grouped.len(), 3); // 設計, レビュー (IssueReview + StageReview), 進捗レポート
        assert_eq!(grouped[0].0, "設計");
        assert_eq!(grouped[1].0, "レビュー");
        assert_eq!(grouped[2].0, "進捗レポート");
    }

    #[test]
    fn test_issue_command_error_display() {
        let err = IssueCommandError::NotFound { issue_number: 42 };
        assert_eq!(err.to_string(), "No documents found for issue #42");

        let err = IssueCommandError::CorruptedMetadata {
            file_path: "test.md".to_string(),
            reason: "bad json".to_string(),
        };
        assert_eq!(err.to_string(), "Corrupted metadata for test.md: bad json");
    }

    #[test]
    fn test_format_human() {
        let result = IssueDocumentsResult {
            issue_number: "140".to_string(),
            documents: vec![
                IssueDocumentEntry {
                    file_path: "design.md".to_string(),
                    relation: KnowledgeRelation::HasDesign,
                    doc_subtype: DocSubtype::DesignPolicy,
                },
                IssueDocumentEntry {
                    file_path: "work-plan.md".to_string(),
                    relation: KnowledgeRelation::HasWorkplan,
                    doc_subtype: DocSubtype::WorkPlan,
                },
            ],
        };
        let mut buf = Vec::new();
        format_human(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Issue #140 関連ドキュメント:"));
        assert!(output.contains("設計:"));
        assert!(output.contains("design.md"));
        assert!(output.contains("作業計画:"));
        assert!(output.contains("work-plan.md"));
    }

    #[test]
    fn test_format_json() {
        let result = IssueDocumentsResult {
            issue_number: "140".to_string(),
            documents: vec![IssueDocumentEntry {
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            }],
        };
        let mut buf = Vec::new();
        format_json(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["issue_number"], "140");
        assert!(parsed["documents"]["設計"].is_array());
    }

    #[test]
    fn test_format_llm() {
        let result = IssueDocumentsResult {
            issue_number: "140".to_string(),
            documents: vec![IssueDocumentEntry {
                file_path: "design.md".to_string(),
                relation: KnowledgeRelation::HasDesign,
                doc_subtype: DocSubtype::DesignPolicy,
            }],
        };
        let mut buf = Vec::new();
        format_llm(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("# Issue #140 関連ドキュメント"));
        assert!(output.contains("## 設計"));
        assert!(output.contains("- design.md"));
    }

    #[test]
    fn test_format_path() {
        let result = IssueDocumentsResult {
            issue_number: "140".to_string(),
            documents: vec![
                IssueDocumentEntry {
                    file_path: "a.md".to_string(),
                    relation: KnowledgeRelation::HasDesign,
                    doc_subtype: DocSubtype::DesignPolicy,
                },
                IssueDocumentEntry {
                    file_path: "b.md".to_string(),
                    relation: KnowledgeRelation::HasWorkplan,
                    doc_subtype: DocSubtype::WorkPlan,
                },
            ],
        };
        let mut buf = Vec::new();
        format_path(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "a.md");
        assert_eq!(lines[1], "b.md");
    }

    // --- extract_label_from_design_path tests ---

    #[test]
    fn test_extract_label_basic() {
        assert_eq!(
            extract_label_from_design_path(
                "dev-reports/design/issue-47-terminal-search-design-policy.md"
            ),
            "terminal-search"
        );
    }

    #[test]
    fn test_extract_label_complex() {
        assert_eq!(
            extract_label_from_design_path(
                "dev-reports/design/issue-99-markdown-editor-display-improvement-design-policy.md"
            ),
            "markdown-editor-display-improvement"
        );
    }

    #[test]
    fn test_extract_label_no_match() {
        assert_eq!(
            extract_label_from_design_path("dev-reports/issue/47/work-plan.md"),
            ""
        );
    }

    #[test]
    fn test_extract_label_empty() {
        assert_eq!(extract_label_from_design_path(""), "");
    }

    // --- convert_row_to_entry tests ---

    #[test]
    fn test_convert_row_to_entry_with_design() {
        let row = IssueListRow {
            number: 47,
            doc_count: 3,
            design_file_path: Some(
                "dev-reports/design/issue-47-terminal-search-design-policy.md".to_string(),
            ),
            has_design: true,
            has_review: false,
            has_workplan: true,
            has_progress: false,
        };
        let entry = convert_row_to_entry(row);
        assert_eq!(entry.number, 47);
        assert_eq!(entry.doc_count, 3);
        assert_eq!(entry.label, "terminal-search");
        assert!(entry.has_design);
        assert!(!entry.has_review);
    }

    #[test]
    fn test_convert_row_to_entry_no_design() {
        let row = IssueListRow {
            number: 50,
            doc_count: 1,
            design_file_path: None,
            has_design: false,
            has_review: false,
            has_workplan: true,
            has_progress: false,
        };
        let entry = convert_row_to_entry(row);
        assert_eq!(entry.label, "");
    }

    // --- format_list tests ---

    fn sample_entries() -> Vec<IssueListEntry> {
        vec![
            IssueListEntry {
                number: 47,
                doc_count: 5,
                label: "terminal-search".to_string(),
                has_design: true,
                has_review: true,
                has_workplan: true,
                has_progress: false,
            },
            IssueListEntry {
                number: 99,
                doc_count: 5,
                label: "markdown-editor-display-improvement".to_string(),
                has_design: true,
                has_review: false,
                has_workplan: true,
                has_progress: false,
            },
        ]
    }

    #[test]
    fn test_format_list_human() {
        let entries = sample_entries();
        let mut buf = Vec::new();
        format_list_human(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("Issue #47"));
        assert!(output.contains("(5 docs)"));
        assert!(output.contains("terminal-search"));
        assert!(output.contains("Issue #99"));
        assert!(output.contains("Total: 2 issues"));
    }

    #[test]
    fn test_format_list_human_empty() {
        let entries: Vec<IssueListEntry> = vec![];
        let mut buf = Vec::new();
        format_list_human(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output.trim(), "No issues found.");
    }

    #[test]
    fn test_format_list_json() {
        let entries = sample_entries();
        let mut buf = Vec::new();
        format_list_json(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["number"], 47);
        assert_eq!(parsed[0]["label"], "terminal-search");
        assert_eq!(parsed[1]["number"], 99);
    }

    #[test]
    fn test_format_list_json_empty() {
        let entries: Vec<IssueListEntry> = vec![];
        let mut buf = Vec::new();
        format_list_json(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_format_list_path() {
        let entries = sample_entries();
        let mut buf = Vec::new();
        format_list_path(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines, vec!["47", "99"]);
    }

    #[test]
    fn test_format_list_path_empty() {
        let entries: Vec<IssueListEntry> = vec![];
        let mut buf = Vec::new();
        format_list_path(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_list_llm() {
        let entries = sample_entries();
        let mut buf = Vec::new();
        format_list_llm(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("| Issue | Docs | Label |"));
        assert!(output.contains("| #47 |"));
        assert!(output.contains("| Yes |"));
        assert!(output.contains("Total: 2 issues"));
    }

    #[test]
    fn test_format_list_llm_empty() {
        let entries: Vec<IssueListEntry> = vec![];
        let mut buf = Vec::new();
        format_list_llm(&entries, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output.trim(), "No issues found.");
    }
}

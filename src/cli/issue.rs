use std::fmt;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::indexer::knowledge::{DocSubtype, IssueDocumentEntry, KnowledgeRelation};
use crate::indexer::symbol_store::{SymbolStore, SymbolStoreError};
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
// Helpers
// ---------------------------------------------------------------------------

fn display_label(subtype: &DocSubtype) -> &'static str {
    match subtype {
        DocSubtype::DesignPolicy => "設計",
        DocSubtype::IssueReview | DocSubtype::DesignReview => "レビュー",
        DocSubtype::WorkPlan => "作業計画",
        DocSubtype::ProgressReport => "進捗レポート",
    }
}

fn sort_order(entry: &IssueDocumentEntry) -> (u8, u8) {
    let relation_order = match entry.relation {
        KnowledgeRelation::HasDesign => 1,
        KnowledgeRelation::HasReview => 2,
        KnowledgeRelation::HasWorkplan => 3,
        KnowledgeRelation::Modifies => 4,
    };
    let subtype_order = match entry.doc_subtype {
        DocSubtype::DesignPolicy => 1,
        DocSubtype::IssueReview => 2,
        DocSubtype::DesignReview => 3,
        DocSubtype::WorkPlan => 4,
        DocSubtype::ProgressReport => 5,
    };
    (relation_order, subtype_order)
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

pub fn run(
    issue_number: u64,
    format: OutputFormat,
    commandindex_dir: &Path,
) -> Result<(), IssueCommandError> {
    // Check symbols.db exists
    let symbol_db = crate::indexer::symbol_db_path(commandindex_dir);
    if !symbol_db.exists() {
        return Err(IssueCommandError::SymbolStore(SymbolStoreError::Io(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Symbol database not found: {}. Run `commandindex index` first.",
                    symbol_db.display()
                ),
            ),
        )));
    }

    let store = SymbolStore::open(&symbol_db)?;
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
// Output formatters
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
        assert!(sort_order(&design) < sort_order(&review));
        assert!(sort_order(&review) < sort_order(&workplan));
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
                    relation: KnowledgeRelation::HasReview,
                    doc_subtype: DocSubtype::ProgressReport,
                },
            ],
        };
        let grouped = result.grouped();
        assert_eq!(grouped.len(), 3); // 設計, レビュー, 進捗レポート
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
}

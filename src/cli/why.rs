pub const WHY_AFTER_HELP: &str = "\
When to use:
  Understand why a file exists by finding related Issues and design documents
  via the knowledge graph.

Examples:
  commandindexdev why dev-reports/design/issue-100-design-policy.md
  commandindexdev why dev-reports/issue/100/work-plan.md --format json
  commandindexdev why dev-reports/design/issue-100-design-policy.md --format path
  commandindexdev why dev-reports/design/issue-100-design-policy.md --format llm";

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::indexer::symbol_store::{SymbolStore, SymbolStoreError};
use crate::output::{self, OutputError, OutputFormat, WhyDocumentEntry, WhyIssueEntry, WhyResult};

/// why エラー型
#[derive(Debug)]
pub enum WhyError {
    IndexNotFound,
    SymbolDbNotFound,
    SymbolStore(SymbolStoreError),
    Output(OutputError),
    InvalidArgument(String),
}

impl fmt::Display for WhyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhyError::IndexNotFound => {
                write!(f, "Index not found. Run `commandindex index` first.")
            }
            WhyError::SymbolDbNotFound => {
                write!(
                    f,
                    "Symbol database not found. Run `commandindex index` first."
                )
            }
            WhyError::SymbolStore(e) => write!(f, "{e}"),
            WhyError::Output(e) => write!(f, "{e}"),
            WhyError::InvalidArgument(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for WhyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WhyError::SymbolStore(e) => Some(e),
            WhyError::Output(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SymbolStoreError> for WhyError {
    fn from(e: SymbolStoreError) -> Self {
        WhyError::SymbolStore(e)
    }
}

impl From<OutputError> for WhyError {
    fn from(e: OutputError) -> Self {
        WhyError::Output(e)
    }
}

/// why サブコマンド実行
pub fn run_why(
    files: &[String],
    format: OutputFormat,
    index_path: Option<&Path>,
) -> Result<(), WhyError> {
    // 1. バリデーション
    crate::cli::validate_file_paths(files, 1)
        .map_err(|e| WhyError::InvalidArgument(e.to_string()))?;

    let file_path = &files[0];

    // 2. インデックスパス解決
    let config = crate::config::load_config(Path::new("."))
        .map_err(|e| WhyError::InvalidArgument(format!("Failed to load config: {e}")))?;
    let config_index_path = config.index.path.as_deref();
    let commandindex_dir =
        crate::indexer::resolve_index_path(index_path, config_index_path, Path::new("."))
            .map_err(|e| WhyError::InvalidArgument(format!("Failed to resolve index path: {e}")))?;

    let tantivy_dir = crate::indexer::index_dir(&commandindex_dir);
    if !tantivy_dir.exists() {
        return Err(WhyError::IndexNotFound);
    }

    // 3. Symbol DB
    let db_path = crate::indexer::symbol_db_path(&commandindex_dir);
    if !db_path.exists() {
        return Err(WhyError::SymbolDbNotFound);
    }
    let store = SymbolStore::open(&db_path)?;

    // 4. ナレッジグラフ検索
    let related = store.find_knowledge_related(file_path)?;

    // 5. Issue別グルーピング → WhyResult 変換
    let mut issue_map: BTreeMap<String, (Option<String>, Vec<WhyDocumentEntry>)> = BTreeMap::new();
    for r in &related {
        let entry = issue_map
            .entry(r.issue_number.clone())
            .or_insert_with(|| (r.title.clone(), Vec::new()));
        // Update title if we have one (later entries may also have it)
        if entry.0.is_none() && r.title.is_some() {
            entry.0.clone_from(&r.title);
        }
        entry.1.push(WhyDocumentEntry {
            file_path: r.file_path.clone(),
            relation: r.relation.clone(),
        });
    }

    let issues: Vec<WhyIssueEntry> = issue_map
        .into_iter()
        .map(|(issue_number, (title, documents))| WhyIssueEntry {
            issue_number,
            title,
            documents,
        })
        .collect();

    let result = WhyResult {
        file_path: file_path.clone(),
        issues,
    };

    // 6. 出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_why_results(&result, format, &mut handle)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_why_error_display() {
        let err = WhyError::IndexNotFound;
        assert!(err.to_string().contains("Index not found"));

        let err = WhyError::SymbolDbNotFound;
        assert!(err.to_string().contains("Symbol database not found"));

        let err = WhyError::InvalidArgument("test".to_string());
        assert_eq!(err.to_string(), "test");
    }

    #[test]
    fn test_why_result_grouping() {
        use crate::indexer::knowledge::KnowledgeRelatedResult;

        let related = vec![
            KnowledgeRelatedResult {
                issue_number: "100".to_string(),
                relation: "has_design".to_string(),
                file_path: "dev-reports/design/issue-100.md".to_string(),
                title: Some("Feature X".to_string()),
            },
            KnowledgeRelatedResult {
                issue_number: "100".to_string(),
                relation: "has_workplan".to_string(),
                file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                title: Some("Feature X".to_string()),
            },
            KnowledgeRelatedResult {
                issue_number: "200".to_string(),
                relation: "has_review".to_string(),
                file_path: "dev-reports/issue/200/review.md".to_string(),
                title: None,
            },
        ];

        let mut issue_map: std::collections::BTreeMap<
            String,
            (Option<String>, Vec<WhyDocumentEntry>),
        > = std::collections::BTreeMap::new();
        for r in &related {
            let entry = issue_map
                .entry(r.issue_number.clone())
                .or_insert_with(|| (r.title.clone(), Vec::new()));
            if entry.0.is_none() && r.title.is_some() {
                entry.0.clone_from(&r.title);
            }
            entry.1.push(WhyDocumentEntry {
                file_path: r.file_path.clone(),
                relation: r.relation.clone(),
            });
        }

        let issues: Vec<WhyIssueEntry> = issue_map
            .into_iter()
            .map(|(issue_number, (title, documents))| WhyIssueEntry {
                issue_number,
                title,
                documents,
            })
            .collect();

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].issue_number, "100");
        assert_eq!(issues[0].title.as_deref(), Some("Feature X"));
        assert_eq!(issues[0].documents.len(), 2);
        assert_eq!(issues[1].issue_number, "200");
        assert!(issues[1].title.is_none());
        assert_eq!(issues[1].documents.len(), 1);
    }

    #[test]
    fn test_format_why_human() {
        let result = WhyResult {
            file_path: "src/main.rs".to_string(),
            issues: vec![WhyIssueEntry {
                issue_number: "100".to_string(),
                title: Some("Add feature".to_string()),
                documents: vec![WhyDocumentEntry {
                    file_path: "dev-reports/design/issue-100.md".to_string(),
                    relation: "has_design".to_string(),
                }],
            }],
        };
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Human, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("src/main.rs"));
        assert!(output.contains("100"));
    }

    #[test]
    fn test_format_why_json() {
        let result = WhyResult {
            file_path: "src/main.rs".to_string(),
            issues: vec![WhyIssueEntry {
                issue_number: "42".to_string(),
                title: None,
                documents: vec![WhyDocumentEntry {
                    file_path: "dev-reports/issue/42/work-plan.md".to_string(),
                    relation: "has_workplan".to_string(),
                }],
            }],
        };
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Json, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["file_path"], "src/main.rs");
        assert_eq!(parsed["issues"][0]["issue_number"], "42");
    }

    #[test]
    fn test_format_why_path() {
        let result = WhyResult {
            file_path: "src/main.rs".to_string(),
            issues: vec![WhyIssueEntry {
                issue_number: "100".to_string(),
                title: None,
                documents: vec![
                    WhyDocumentEntry {
                        file_path: "dev-reports/design/issue-100.md".to_string(),
                        relation: "has_design".to_string(),
                    },
                    WhyDocumentEntry {
                        file_path: "dev-reports/issue/100/work-plan.md".to_string(),
                        relation: "has_workplan".to_string(),
                    },
                ],
            }],
        };
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Path, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3); // input file + 2 docs
        assert_eq!(lines[0], "src/main.rs");
    }

    #[test]
    fn test_format_why_llm() {
        let result = WhyResult {
            file_path: "src/main.rs".to_string(),
            issues: vec![WhyIssueEntry {
                issue_number: "100".to_string(),
                title: Some("Feature".to_string()),
                documents: vec![WhyDocumentEntry {
                    file_path: "dev-reports/design/issue-100.md".to_string(),
                    relation: "has_design".to_string(),
                }],
            }],
        };
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Llm, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("## Why: src/main.rs"));
        assert!(output.contains("Issue #100"));
    }

    #[test]
    fn test_format_why_empty_issues() {
        let result = WhyResult {
            file_path: "src/main.rs".to_string(),
            issues: vec![],
        };

        // Human
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Human, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("No related issues found"));

        // LLM
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Llm, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("No related issues found"));

        // Path: only the input file
        let mut buf = Vec::new();
        output::format_why_results(&result, OutputFormat::Path, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output.trim(), "src/main.rs");
    }
}

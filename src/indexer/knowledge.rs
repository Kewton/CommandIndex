use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::indexer::symbol_store::SymbolStoreError;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum KnowledgeError {
    Io(std::io::Error),
    Store(SymbolStoreError),
    PathValidation(String),
}

impl fmt::Display for KnowledgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Store(e) => write!(f, "Symbol store error: {e}"),
            Self::PathValidation(msg) => write!(f, "Path validation error: {msg}"),
        }
    }
}

impl std::error::Error for KnowledgeError {}

impl From<std::io::Error> for KnowledgeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<SymbolStoreError> for KnowledgeError {
    fn from(e: SymbolStoreError) -> Self {
        Self::Store(e)
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// ナレッジグラフのエッジ種別
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum KnowledgeRelation {
    HasDesign,
    HasReview,
    HasWorkplan,
}

impl KnowledgeRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HasDesign => "has_design",
            Self::HasReview => "has_review",
            Self::HasWorkplan => "has_workplan",
        }
    }

    /// Parse a relation string from the database. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "has_design" => Some(Self::HasDesign),
            "has_review" => Some(Self::HasReview),
            "has_workplan" => Some(Self::HasWorkplan),
            _ => None,
        }
    }
}

impl fmt::Display for KnowledgeRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// ドキュメントのサブタイプ
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DocSubtype {
    DesignPolicy,
    WorkPlan,
    IssueReview,
    DesignReview,
    ProgressReport,
}

impl DocSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DesignPolicy => "design_policy",
            Self::WorkPlan => "work_plan",
            Self::IssueReview => "issue_review",
            Self::DesignReview => "design_review",
            Self::ProgressReport => "progress_report",
        }
    }
}

/// パース結果
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub issue_number: String,
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
}

/// Issue関連ドキュメントの検索結果（metadataパース済みDTO）
#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentEntry {
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
}

/// search --related の戻り値用構造体
#[derive(Debug, Clone)]
pub struct KnowledgeRelatedResult {
    pub file_path: String,
    pub relation: String,
    pub issue_number: String,
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// Pattern rules
// ---------------------------------------------------------------------------

struct PatternRule {
    regex: regex::Regex,
    doc_subtype: DocSubtype,
    relation: KnowledgeRelation,
}

fn build_pattern_rules() -> Vec<PatternRule> {
    vec![
        PatternRule {
            regex: regex::Regex::new(r"^dev-reports/design/issue-(\d+)-.*-design-policy\.md$")
                .expect("invalid regex"),
            doc_subtype: DocSubtype::DesignPolicy,
            relation: KnowledgeRelation::HasDesign,
        },
        PatternRule {
            regex: regex::Regex::new(r"^dev-reports/issue/(\d+)/issue-review/summary-report\.md$")
                .expect("invalid regex"),
            doc_subtype: DocSubtype::IssueReview,
            relation: KnowledgeRelation::HasReview,
        },
        PatternRule {
            regex: regex::Regex::new(
                r"^dev-reports/issue/(\d+)/multi-stage-design-review/summary-report\.md$",
            )
            .expect("invalid regex"),
            doc_subtype: DocSubtype::DesignReview,
            relation: KnowledgeRelation::HasReview,
        },
        PatternRule {
            regex: regex::Regex::new(r"^dev-reports/issue/(\d+)/work-plan\.md$")
                .expect("invalid regex"),
            doc_subtype: DocSubtype::WorkPlan,
            relation: KnowledgeRelation::HasWorkplan,
        },
        PatternRule {
            regex: regex::Regex::new(
                r"^dev-reports/issue/(\d+)/pm-auto-dev/.+/progress-report\.md$",
            )
            .expect("invalid regex"),
            doc_subtype: DocSubtype::ProgressReport,
            relation: KnowledgeRelation::HasReview,
        },
    ]
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// パスパターンからIssue番号とドキュメント種別を抽出
pub fn parse_dev_report_path(path: &str) -> Option<KnowledgeEntry> {
    // Normalize path separators to forward slashes
    let normalized = path.replace('\\', "/");
    let rules = build_pattern_rules();

    for rule in &rules {
        if let Some(captures) = rule.regex.captures(&normalized)
            && let Some(issue_num) = captures.get(1)
        {
            return Some(KnowledgeEntry {
                issue_number: issue_num.as_str().to_string(),
                file_path: normalized.to_string(),
                relation: rule.relation.clone(),
                doc_subtype: rule.doc_subtype.clone(),
            });
        }
    }
    None
}

/// dev-reports/ ディレクトリを走査し、ナレッジエントリを抽出
pub fn scan_dev_reports(base_dir: &Path) -> Vec<KnowledgeEntry> {
    let dev_reports_dir = base_dir.join("dev-reports");
    if !dev_reports_dir.exists() {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let walker = walkdir::WalkDir::new(&dev_reports_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());

    for entry in walker {
        if !entry.file_type().is_file() {
            continue;
        }
        if let Ok(relative) = entry.path().strip_prefix(base_dir) {
            let rel_str = relative.to_string_lossy().replace('\\', "/");
            if let Some(knowledge_entry) = parse_dev_report_path(&rel_str) {
                entries.push(knowledge_entry);
            }
        }
    }
    entries
}

/// git diff から dev-reports/ 配下の変更ファイルを抽出
pub fn detect_dev_reports_changes(base_dir: &Path) -> Result<DevReportsChanges, KnowledgeError> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "HEAD",
            "--name-only",
            "--diff-filter=ACDMR",
            "--",
            "dev-reports/",
        ])
        .current_dir(base_dir)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let changed_files: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    Ok(DevReportsChanges { changed_files })
}

/// dev-reports の変更情報
#[derive(Debug)]
pub struct DevReportsChanges {
    pub changed_files: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_design_policy() {
        let result =
            parse_dev_report_path("dev-reports/design/issue-299-ipad-layout-fix-design-policy.md");
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "299");
        assert_eq!(entry.relation, KnowledgeRelation::HasDesign);
        assert_eq!(entry.doc_subtype, DocSubtype::DesignPolicy);
    }

    #[test]
    fn test_parse_issue_review_summary() {
        let result = parse_dev_report_path("dev-reports/issue/123/issue-review/summary-report.md");
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "123");
        assert_eq!(entry.relation, KnowledgeRelation::HasReview);
        assert_eq!(entry.doc_subtype, DocSubtype::IssueReview);
    }

    #[test]
    fn test_parse_design_review_summary() {
        let result = parse_dev_report_path(
            "dev-reports/issue/42/multi-stage-design-review/summary-report.md",
        );
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "42");
        assert_eq!(entry.relation, KnowledgeRelation::HasReview);
        assert_eq!(entry.doc_subtype, DocSubtype::DesignReview);
    }

    #[test]
    fn test_parse_work_plan() {
        let result = parse_dev_report_path("dev-reports/issue/99/work-plan.md");
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "99");
        assert_eq!(entry.relation, KnowledgeRelation::HasWorkplan);
        assert_eq!(entry.doc_subtype, DocSubtype::WorkPlan);
    }

    #[test]
    fn test_parse_progress_report() {
        let result = parse_dev_report_path(
            "dev-reports/issue/55/pm-auto-dev/iteration-1/progress-report.md",
        );
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "55");
        assert_eq!(entry.relation, KnowledgeRelation::HasReview);
        assert_eq!(entry.doc_subtype, DocSubtype::ProgressReport);
    }

    #[test]
    fn test_parse_non_matching_path() {
        assert!(parse_dev_report_path("src/main.rs").is_none());
        assert!(
            parse_dev_report_path("dev-reports/issue/123/issue-review/stage1-review-context.json")
                .is_none()
        );
        assert!(
            parse_dev_report_path("dev-reports/issue/123/issue-review/hypothesis-verification.md")
                .is_none()
        );
        assert!(parse_dev_report_path("README.md").is_none());
    }

    #[test]
    fn test_scan_dev_reports_with_temp_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path();

        // Create test structure
        let design_dir = base.join("dev-reports/design");
        std::fs::create_dir_all(&design_dir).unwrap();
        std::fs::write(
            design_dir.join("issue-100-test-design-policy.md"),
            "# Design",
        )
        .unwrap();

        let issue_dir = base.join("dev-reports/issue/100");
        std::fs::create_dir_all(&issue_dir).unwrap();
        std::fs::write(issue_dir.join("work-plan.md"), "# Work Plan").unwrap();

        let review_dir = base.join("dev-reports/issue/100/issue-review");
        std::fs::create_dir_all(&review_dir).unwrap();
        std::fs::write(review_dir.join("summary-report.md"), "# Summary").unwrap();
        // This should NOT be picked up
        std::fs::write(review_dir.join("stage1-review-context.json"), "{}").unwrap();

        let entries = scan_dev_reports(base);
        assert_eq!(entries.len(), 3);

        let issue_nums: Vec<&str> = entries.iter().map(|e| e.issue_number.as_str()).collect();
        assert!(issue_nums.iter().all(|n| *n == "100"));
    }

    #[test]
    fn test_scan_dev_reports_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entries = scan_dev_reports(tmp.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_knowledge_relation_as_str() {
        assert_eq!(KnowledgeRelation::HasDesign.as_str(), "has_design");
        assert_eq!(KnowledgeRelation::HasReview.as_str(), "has_review");
        assert_eq!(KnowledgeRelation::HasWorkplan.as_str(), "has_workplan");
    }

    #[test]
    fn test_knowledge_relation_parse() {
        assert_eq!(
            KnowledgeRelation::parse("has_design"),
            Some(KnowledgeRelation::HasDesign)
        );
        assert_eq!(
            KnowledgeRelation::parse("has_review"),
            Some(KnowledgeRelation::HasReview)
        );
        assert_eq!(
            KnowledgeRelation::parse("has_workplan"),
            Some(KnowledgeRelation::HasWorkplan)
        );
        assert_eq!(KnowledgeRelation::parse("unknown"), None);
        assert_eq!(KnowledgeRelation::parse(""), None);
    }

    #[test]
    fn test_knowledge_relation_display() {
        assert_eq!(format!("{}", KnowledgeRelation::HasDesign), "has_design");
        assert_eq!(format!("{}", KnowledgeRelation::HasReview), "has_review");
        assert_eq!(
            format!("{}", KnowledgeRelation::HasWorkplan),
            "has_workplan"
        );
    }

    #[test]
    fn test_doc_subtype_as_str() {
        assert_eq!(DocSubtype::DesignPolicy.as_str(), "design_policy");
        assert_eq!(DocSubtype::WorkPlan.as_str(), "work_plan");
        assert_eq!(DocSubtype::IssueReview.as_str(), "issue_review");
        assert_eq!(DocSubtype::DesignReview.as_str(), "design_review");
        assert_eq!(DocSubtype::ProgressReport.as_str(), "progress_report");
    }
}

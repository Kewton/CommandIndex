use std::collections::HashSet;
use std::fmt;
use std::io::{BufRead, BufReader, Read as _};
use std::path::Path;
use std::sync::LazyLock;

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
    GitLog(String),
}

impl fmt::Display for KnowledgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Store(e) => write!(f, "Symbol store error: {e}"),
            Self::GitLog(e) => write!(f, "Git log error: {e}"),
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
// Shared ISSUE_RE regex and extraction
// ---------------------------------------------------------------------------

/// Statically compiled regex for issue number extraction.
/// Shared between `before_change.rs` and `knowledge.rs`.
pub static ISSUE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:#(\d+)|\(#(\d+)\)|fixes\s+#(\d+)|refs\s+#(\d+)|issue[- ]?(\d+))")
        .expect("ISSUE_RE is a valid regex literal")
});

/// Extract issue numbers from text using `ISSUE_RE`.
/// Returns a list of issue number strings (may contain duplicates if the same
/// issue appears multiple times in different patterns).
pub fn extract_issue_numbers(text: &str) -> Vec<String> {
    ISSUE_RE
        .captures_iter(text)
        .filter_map(|cap| {
            cap.get(1)
                .or(cap.get(2))
                .or(cap.get(3))
                .or(cap.get(4))
                .or(cap.get(5))
                .map(|m| m.as_str().to_string())
        })
        .collect()
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
    HasProgress,
    Modifies,
}

impl KnowledgeRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HasDesign => "has_design",
            Self::HasReview => "has_review",
            Self::HasWorkplan => "has_workplan",
            Self::HasProgress => "has_progress",
            Self::Modifies => "modifies",
        }
    }

    /// Relation priority for sorting (lower = higher priority).
    /// HasProgress / Modifies はフィルタで除外されることが多いが、
    /// 型の網羅性（exhaustive match）を保証するために優先度を定義している。
    pub fn priority(&self) -> u8 {
        match self {
            Self::HasDesign => 0,
            Self::HasWorkplan => 1,
            Self::HasReview => 2,
            Self::HasProgress => 3,
            Self::Modifies => 4,
        }
    }

    /// Parse a relation string from the database. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "has_design" => Some(Self::HasDesign),
            "has_review" => Some(Self::HasReview),
            "has_workplan" => Some(Self::HasWorkplan),
            "has_progress" => Some(Self::HasProgress),
            "modifies" => Some(Self::Modifies),
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
    StageReview,
}

impl DocSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DesignPolicy => "design_policy",
            Self::WorkPlan => "work_plan",
            Self::IssueReview => "issue_review",
            Self::DesignReview => "design_review",
            Self::ProgressReport => "progress_report",
            Self::StageReview => "stage_review",
        }
    }

    pub fn display_label_en(&self) -> &'static str {
        match self {
            Self::DesignPolicy => "design",
            Self::WorkPlan => "workplan",
            Self::IssueReview => "review",
            Self::DesignReview => "review",
            Self::ProgressReport => "progress",
            Self::StageReview => "review",
        }
    }

    /// Parse a doc subtype string from the database. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "design_policy" => Some(Self::DesignPolicy),
            "work_plan" => Some(Self::WorkPlan),
            "issue_review" => Some(Self::IssueReview),
            "design_review" => Some(Self::DesignReview),
            "progress_report" => Some(Self::ProgressReport),
            "stage_review" => Some(Self::StageReview),
            _ => None,
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
    pub date: Option<String>,
}

/// Issue関連ドキュメントの検索結果（metadataパース済みDTO）
#[derive(Debug, Clone, Serialize)]
pub struct IssueDocumentEntry {
    pub file_path: String,
    pub relation: KnowledgeRelation,
    pub doc_subtype: DocSubtype,
    pub date: Option<String>,
    pub snippet: Option<String>,
}

/// search --related の戻り値用構造体
#[derive(Debug, Clone)]
pub struct KnowledgeRelatedResult {
    pub file_path: String,
    pub relation: String,
    pub issue_number: String,
    pub title: Option<String>,
    pub doc_subtype: Option<DocSubtype>,
    pub date: Option<String>,
}

/// git log から抽出した (issue, file) ペア
#[derive(Debug, Clone, PartialEq)]
pub struct FileModifiesEntry {
    pub issue_number: String,
    pub file_path: String,
}

// ---------------------------------------------------------------------------
// git log → file-modifies extraction
// ---------------------------------------------------------------------------

/// Maximum lines to read from git log output for file-modifies extraction.
const MAX_GIT_OUTPUT_LINES: usize = 50_000;

/// Maximum number of (issue, file) entries to keep.
const MAX_ENTRIES: usize = 100_000;

/// Validate a file path from git log output.
/// Rejects paths with `..`, absolute paths, null bytes, and overly long paths.
fn validate_git_file_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path.len() > 1024 {
        return false;
    }
    if path.contains('\0') {
        return false;
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    if path.contains("..") {
        return false;
    }
    true
}

/// Extract (issue_number, file_path) pairs from git log.
///
/// Runs `git log --all --format='COMMIT_START%n%s%n%b%nCOMMIT_END' --name-only`
/// and parses commit messages for issue references, associating them with changed files.
pub fn extract_file_modifies_from_git_log(
    repo_path: &Path,
) -> Result<Vec<FileModifiesEntry>, KnowledgeError> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("git")
        .args([
            "log",
            "--all",
            "--format=COMMIT_START%n%s%n%b%nCOMMIT_END",
            "--name-only",
        ])
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| KnowledgeError::Io(std::io::Error::other("Failed to capture stdout")))?;
    let stderr_pipe = child.stderr.take();

    // Read stderr in a separate thread to prevent deadlock
    let stderr_thread = std::thread::spawn(move || -> String {
        let Some(stderr) = stderr_pipe else {
            return String::new();
        };
        let mut buf = String::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_string(&mut buf);
        buf
    });

    let mut seen = HashSet::new();
    let mut current_issues: Vec<String> = Vec::new();
    let mut in_commit = false;
    let mut reading_files = false;
    let mut line_count = 0;

    let reader = BufReader::new(stdout);
    for line_result in reader.lines() {
        line_count += 1;
        if line_count > MAX_GIT_OUTPUT_LINES {
            break;
        }
        let line = line_result?;

        if line == "COMMIT_START" {
            in_commit = true;
            reading_files = false;
            current_issues.clear();
            continue;
        }

        if line == "COMMIT_END" {
            reading_files = true;
            in_commit = false;
            continue;
        }

        if in_commit {
            // Parse subject/body lines for issue numbers
            let nums = extract_issue_numbers(&line);
            for num in nums {
                if !current_issues.contains(&num) {
                    current_issues.push(num);
                }
            }
            continue;
        }

        if reading_files {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Skip empty lines — git log places them between format output and file list,
                // and between file lists of consecutive commits. The next COMMIT_START
                // will naturally reset reading_files.
                continue;
            }

            if validate_git_file_path(trimmed) {
                for issue in &current_issues {
                    let key = (issue.clone(), trimmed.to_string());
                    if seen.len() < MAX_ENTRIES && seen.insert(key) {
                        // inserted new entry
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| KnowledgeError::GitLog(e.to_string()))?;
    let stderr_output = stderr_thread.join().unwrap_or_default();

    if !status.success() {
        // Non-zero exit is not fatal for modifies graph — log a warning but return
        // whatever we collected so far. Common cause: shallow clone or missing refs.
        eprintln!(
            "Warning: git log exited with status {}{}",
            status,
            if stderr_output.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr_output.trim())
            }
        );
    }

    let entries: Vec<FileModifiesEntry> = seen
        .into_iter()
        .map(|(issue_number, file_path)| FileModifiesEntry {
            issue_number,
            file_path,
        })
        .collect();

    Ok(entries)
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
            relation: KnowledgeRelation::HasProgress,
        },
        // Note: issue{N} uses no hyphen separator, matching the review tool's output naming convention
        PatternRule {
            regex: regex::Regex::new(
                r"^dev-reports/review/\d{4}-\d{2}-\d{2}-issue(\d+)-[^/]*\.md$",
            )
            .expect("invalid regex"),
            doc_subtype: DocSubtype::StageReview,
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
                date: None,
            });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Date extraction utilities
// ---------------------------------------------------------------------------

/// ファイル名先頭の YYYY-MM-DD パターン（コンパイル済みキャッシュ）
static DATE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(\d{4}-\d{2}-\d{2})").expect("DATE_RE is a valid regex literal")
});

/// ファイル名から先頭の YYYY-MM-DD パターンを抽出（^アンカー付き）
fn extract_date_from_filename(file_path: &str) -> Option<String> {
    let filename = Path::new(file_path).file_name()?.to_str()?;
    let date_str = DATE_RE.captures(filename).map(|c| c[1].to_string())?;
    // chrono::NaiveDate でバリデーション（不正な月・日を拒否）
    chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
    Some(date_str)
}

/// git log から最終コミット日を取得
fn extract_date_from_git_log(file_path: &str, repo_root: &Path) -> Option<String> {
    if !validate_git_file_path(file_path) {
        tracing::debug!("Invalid file path for git log: {}", file_path);
        return None;
    }
    let output = std::process::Command::new("git")
        .args(["log", "--format=%ai", "-1", "--", file_path])
        .current_dir(repo_root)
        .output()
        .map_err(|e| {
            tracing::debug!("git log failed for {}: {}", file_path, e);
            e
        })
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();
    if line.is_empty() {
        tracing::debug!("git log returned empty for {}", file_path);
        return None;
    }
    // "%ai" format: "2026-03-20 10:30:00 +0900" → "2026-03-20"
    let date_str = line.get(..10)?;
    // chrono::NaiveDate でバリデーション
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    Some(date_str.to_string())
}

/// ファイルパスから日付を取得する
/// 1. ファイル名の先頭 YYYY-MM-DD パターンを正規表現で抽出
/// 2. マッチしなければ git log --format=%ai -1 -- <path> にフォールバック
/// 3. いずれも失敗した場合は None
pub(crate) fn extract_date_from_path(file_path: &str, repo_root: &Path) -> Option<String> {
    if let Some(date) = extract_date_from_filename(file_path) {
        return Some(date);
    }
    extract_date_from_git_log(file_path, repo_root)
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
            if let Some(mut knowledge_entry) = parse_dev_report_path(&rel_str) {
                knowledge_entry.date = extract_date_from_path(&rel_str, base_dir);
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
        assert_eq!(entry.relation, KnowledgeRelation::HasProgress);
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

        // Stage review file in dev-reports/review/
        let stage_review_dir = base.join("dev-reports/review");
        std::fs::create_dir_all(&stage_review_dir).unwrap();
        std::fs::write(
            stage_review_dir.join("2024-01-15-issue100-design-review-stage1.md"),
            "# Stage Review",
        )
        .unwrap();

        let entries = scan_dev_reports(base);
        assert_eq!(entries.len(), 4);

        let issue_nums: Vec<&str> = entries.iter().map(|e| e.issue_number.as_str()).collect();
        assert!(issue_nums.iter().all(|n| *n == "100"));
    }

    #[test]
    fn test_parse_stage_review() {
        let result = parse_dev_report_path(
            "dev-reports/review/2026-02-18-issue299-security-review-stage4.md",
        );
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "299");
        assert_eq!(entry.relation, KnowledgeRelation::HasReview);
        assert_eq!(entry.doc_subtype, DocSubtype::StageReview);
    }

    #[test]
    fn test_parse_stage_review_multi_digit_issue() {
        let result = parse_dev_report_path("dev-reports/review/2024-01-01-issue1234-test.md");
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "1234");
        assert_eq!(entry.doc_subtype, DocSubtype::StageReview);
    }

    #[test]
    fn test_parse_stage_review_hyphenated_desc() {
        let result = parse_dev_report_path(
            "dev-reports/review/2024-01-01-issue42-long-desc-with-hyphens-stage1.md",
        );
        assert!(result.is_some());
        let entry = result.unwrap();
        assert_eq!(entry.issue_number, "42");
        assert_eq!(entry.doc_subtype, DocSubtype::StageReview);
    }

    #[test]
    fn test_parse_stage_review_non_matching() {
        // No date prefix
        assert!(parse_dev_report_path("dev-reports/review/no-date-issue100.md").is_none());
        // No issue number
        assert!(
            parse_dev_report_path("dev-reports/review/2024-01-01-no-issue-number.md").is_none()
        );
        // JSON file
        assert!(
            parse_dev_report_path("dev-reports/review/2024-01-01-issue100-test.json").is_none()
        );
    }

    #[test]
    fn test_doc_subtype_parse() {
        assert_eq!(
            DocSubtype::parse("design_policy"),
            Some(DocSubtype::DesignPolicy)
        );
        assert_eq!(DocSubtype::parse("work_plan"), Some(DocSubtype::WorkPlan));
        assert_eq!(
            DocSubtype::parse("issue_review"),
            Some(DocSubtype::IssueReview)
        );
        assert_eq!(
            DocSubtype::parse("design_review"),
            Some(DocSubtype::DesignReview)
        );
        assert_eq!(
            DocSubtype::parse("progress_report"),
            Some(DocSubtype::ProgressReport)
        );
        assert_eq!(
            DocSubtype::parse("stage_review"),
            Some(DocSubtype::StageReview)
        );
        assert_eq!(DocSubtype::parse("unknown"), None);
        assert_eq!(DocSubtype::parse(""), None);
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
        assert_eq!(KnowledgeRelation::HasProgress.as_str(), "has_progress");
        assert_eq!(KnowledgeRelation::Modifies.as_str(), "modifies");
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
        assert_eq!(
            KnowledgeRelation::parse("has_progress"),
            Some(KnowledgeRelation::HasProgress)
        );
        assert_eq!(
            KnowledgeRelation::parse("modifies"),
            Some(KnowledgeRelation::Modifies)
        );
        assert_eq!(KnowledgeRelation::parse("unknown"), None);
        assert_eq!(KnowledgeRelation::parse(""), None);
    }

    #[test]
    fn test_knowledge_relation_modifies_roundtrip() {
        let relation = KnowledgeRelation::Modifies;
        let s = relation.as_str();
        let parsed = KnowledgeRelation::parse(s);
        assert_eq!(parsed, Some(KnowledgeRelation::Modifies));
    }

    #[test]
    fn test_knowledge_relation_display() {
        assert_eq!(format!("{}", KnowledgeRelation::HasDesign), "has_design");
        assert_eq!(format!("{}", KnowledgeRelation::HasReview), "has_review");
        assert_eq!(
            format!("{}", KnowledgeRelation::HasWorkplan),
            "has_workplan"
        );
        assert_eq!(
            format!("{}", KnowledgeRelation::HasProgress),
            "has_progress"
        );
        assert_eq!(format!("{}", KnowledgeRelation::Modifies), "modifies");
    }

    // --- extract_issue_numbers tests ---

    #[test]
    fn test_extract_issue_numbers_hash() {
        let nums = extract_issue_numbers("fix #123 and #456");
        assert!(nums.contains(&"123".to_string()));
        assert!(nums.contains(&"456".to_string()));
    }

    #[test]
    fn test_extract_issue_numbers_parens() {
        let nums = extract_issue_numbers("feat: add feature (#789)");
        assert_eq!(nums, vec!["789".to_string()]);
    }

    #[test]
    fn test_extract_issue_numbers_fixes() {
        let nums = extract_issue_numbers("fixes #42");
        assert_eq!(nums, vec!["42".to_string()]);
    }

    #[test]
    fn test_extract_issue_numbers_refs() {
        let nums = extract_issue_numbers("refs #100");
        assert_eq!(nums, vec!["100".to_string()]);
    }

    #[test]
    fn test_extract_issue_numbers_case_insensitive() {
        let nums = extract_issue_numbers("Fixes #10 REFS #20");
        assert!(nums.contains(&"10".to_string()));
        assert!(nums.contains(&"20".to_string()));
    }

    #[test]
    fn test_extract_issue_numbers_no_match() {
        let nums = extract_issue_numbers("no issue reference here");
        assert!(nums.is_empty());
    }

    #[test]
    fn test_extract_issue_numbers_empty() {
        let nums = extract_issue_numbers("");
        assert!(nums.is_empty());
    }

    #[test]
    fn test_extract_issue_numbers_multiple_patterns() {
        let nums = extract_issue_numbers("#1 (#2) fixes #3 refs #4");
        assert_eq!(nums.len(), 4);
        assert!(nums.contains(&"1".to_string()));
        assert!(nums.contains(&"2".to_string()));
        assert!(nums.contains(&"3".to_string()));
        assert!(nums.contains(&"4".to_string()));
    }

    // --- Bug #151: issue-NNN pattern not matched ---

    #[test]
    fn test_extract_issue_numbers_issue_dash_pattern() {
        // feat(issue-99): ... → should extract 99
        let nums = extract_issue_numbers("feat(issue-99): add sidebar toggle");
        assert!(
            nums.contains(&"99".to_string()),
            "issue-99 not matched: {nums:?}"
        );
    }

    #[test]
    fn test_extract_issue_numbers_issue_space_pattern() {
        // Issue #525, #526 → should extract 525 and 526
        let nums = extract_issue_numbers("Issue #525, #526, #168");
        assert!(
            nums.contains(&"525".to_string()),
            "#525 not matched: {nums:?}"
        );
        assert!(
            nums.contains(&"526".to_string()),
            "#526 not matched: {nums:?}"
        );
        assert!(
            nums.contains(&"168".to_string()),
            "#168 not matched: {nums:?}"
        );
    }

    #[test]
    fn test_extract_issue_numbers_fix_parens_hash() {
        // fix(#299): ... → should extract 299
        let nums = extract_issue_numbers("fix(#299): unify z-index system");
        assert!(
            nums.contains(&"299".to_string()),
            "#299 not matched: {nums:?}"
        );
    }

    // --- validate_git_file_path tests ---

    #[test]
    fn test_validate_git_file_path_normal() {
        assert!(validate_git_file_path("src/main.rs"));
        assert!(validate_git_file_path("README.md"));
        assert!(validate_git_file_path("a/b/c/d.txt"));
    }

    #[test]
    fn test_validate_git_file_path_rejects_dotdot() {
        assert!(!validate_git_file_path("../etc/passwd"));
        assert!(!validate_git_file_path("src/../secret"));
    }

    #[test]
    fn test_validate_git_file_path_rejects_absolute() {
        assert!(!validate_git_file_path("/etc/passwd"));
        assert!(!validate_git_file_path("\\windows\\system32"));
    }

    #[test]
    fn test_validate_git_file_path_rejects_empty() {
        assert!(!validate_git_file_path(""));
    }

    #[test]
    fn test_validate_git_file_path_rejects_null_byte() {
        assert!(!validate_git_file_path("src/\0evil.rs"));
    }

    #[test]
    fn test_validate_git_file_path_rejects_long_path() {
        let long_path = "a".repeat(1025);
        assert!(!validate_git_file_path(&long_path));
    }

    #[test]
    fn test_doc_subtype_display_label_en() {
        assert_eq!(DocSubtype::DesignPolicy.display_label_en(), "design");
        assert_eq!(DocSubtype::WorkPlan.display_label_en(), "workplan");
        assert_eq!(DocSubtype::IssueReview.display_label_en(), "review");
        assert_eq!(DocSubtype::DesignReview.display_label_en(), "review");
        assert_eq!(DocSubtype::ProgressReport.display_label_en(), "progress");
        assert_eq!(DocSubtype::StageReview.display_label_en(), "review");
    }

    #[test]
    fn test_doc_subtype_as_str() {
        assert_eq!(DocSubtype::DesignPolicy.as_str(), "design_policy");
        assert_eq!(DocSubtype::WorkPlan.as_str(), "work_plan");
        assert_eq!(DocSubtype::IssueReview.as_str(), "issue_review");
        assert_eq!(DocSubtype::DesignReview.as_str(), "design_review");
        assert_eq!(DocSubtype::ProgressReport.as_str(), "progress_report");
        assert_eq!(DocSubtype::StageReview.as_str(), "stage_review");
    }

    // --- extract_date_from_filename tests ---

    #[test]
    fn test_extract_date_from_filename_normal() {
        assert_eq!(
            extract_date_from_filename("dev-reports/review/2026-03-20-issue140-review.md"),
            Some("2026-03-20".to_string())
        );
    }

    #[test]
    fn test_extract_date_from_filename_no_date() {
        assert_eq!(
            extract_date_from_filename("dev-reports/design/issue-140-design-policy.md"),
            None
        );
    }

    #[test]
    fn test_extract_date_from_filename_invalid_date() {
        // Month 13, day 45 — chrono should reject
        assert_eq!(
            extract_date_from_filename("dev-reports/review/2026-13-45-invalid.md"),
            None
        );
    }

    #[test]
    fn test_extract_date_from_filename_not_at_start() {
        // Date not at start of filename — should not match
        assert_eq!(
            extract_date_from_filename("dev-reports/review/report-for-2026-03-20.md"),
            None
        );
    }
}

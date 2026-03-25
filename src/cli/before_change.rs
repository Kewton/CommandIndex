pub const BEFORE_CHANGE_AFTER_HELP: &str = "\
When to use:
  Before modifying a file, to understand related design decisions and review history.
  Shows design constraints, review findings, and work plans linked via knowledge graph.

  --limit controls the maximum number of issues (not documents) to show.
  Each issue includes up to 2 representative documents (design policy + work plan).

Examples:
  commandindexdev before-change src/auth.rs
  commandindexdev before-change src/auth.rs --format json
  commandindexdev before-change src/auth.rs --format llm --limit 5
  commandindexdev before-change src/auth.rs --max-commits 500";

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use crate::cli::git::GitError;
use crate::cli::stdin::{StdinError, validate_file_path};
use crate::embedding::store::{EmbeddingRecord, EmbeddingStore, cosine_similarity};
use crate::indexer::ResolveIndexPathError;
use crate::indexer::knowledge::KnowledgeRelation;
use crate::indexer::symbol_store::{KnowledgeDocResult, SymbolStore, SymbolStoreError};
use crate::output::{BeforeChangeFinding, BeforeChangeResult, OutputError, OutputFormat};

/// Maximum lines to read from git log output
const MAX_GIT_OUTPUT_LINES: usize = 5000;

/// Maximum documents to select per issue (design + workplan)
const MAX_DOCS_PER_ISSUE: usize = 2;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BeforeChangeError {
    InvalidInput(String),
    IndexNotFound,
    SymbolDbNotFound,
    SymbolStore(SymbolStoreError),
    Git(GitError),
    Output(OutputError),
    ResolveIndexPath(ResolveIndexPathError),
    Config(String),
    Io(std::io::Error),
    NotGitRepository,
}

impl fmt::Display for BeforeChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "{msg}"),
            Self::IndexNotFound => {
                write!(f, "Index not found. Run `commandindex index` first.")
            }
            Self::SymbolDbNotFound => {
                write!(
                    f,
                    "Symbol database not found. Run `commandindex index` first."
                )
            }
            Self::SymbolStore(e) => write!(f, "{e}"),
            Self::Git(e) => write!(f, "{e}"),
            Self::Output(e) => write!(f, "{e}"),
            Self::ResolveIndexPath(e) => write!(f, "{e}"),
            Self::Config(msg) => write!(f, "Config error: {msg}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::NotGitRepository => {
                write!(f, "Not a git repository")
            }
        }
    }
}

impl std::error::Error for BeforeChangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SymbolStore(e) => Some(e),
            Self::Git(e) => Some(e),
            Self::Output(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::ResolveIndexPath(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SymbolStoreError> for BeforeChangeError {
    fn from(e: SymbolStoreError) -> Self {
        Self::SymbolStore(e)
    }
}

impl From<GitError> for BeforeChangeError {
    fn from(e: GitError) -> Self {
        Self::Git(e)
    }
}

impl From<OutputError> for BeforeChangeError {
    fn from(e: OutputError) -> Self {
        Self::Output(e)
    }
}

impl From<ResolveIndexPathError> for BeforeChangeError {
    fn from(e: ResolveIndexPathError) -> Self {
        Self::ResolveIndexPath(e)
    }
}

impl From<std::io::Error> for BeforeChangeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

/// Validate file path for before-change command.
/// Uses shared validate_file_path() plus git-specific leading dash check.
fn validate_before_change_input(file: &str) -> Result<(), BeforeChangeError> {
    validate_file_path(file)
        .map_err(|e: StdinError| BeforeChangeError::InvalidInput(format!("{e}")))?;
    if file.starts_with('-') {
        return Err(BeforeChangeError::InvalidInput(
            "file path must not start with '-'".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Git log scanning
// ---------------------------------------------------------------------------

/// Extract issue numbers from git log for a file.
/// Returns unique issue numbers sorted.
fn extract_issues_from_git_log(
    file_path: &str,
    max_commits: usize,
) -> Result<Vec<String>, BeforeChangeError> {
    use std::io::{BufRead, BufReader, Read as _};
    use std::process::{Command, Stdio};

    let mut child = Command::new("git")
        .args([
            "log",
            "--max-count",
            &max_commits.to_string(),
            "--format=%s%n%b",
            "--",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| BeforeChangeError::Git(GitError::GitNotFound))?;

    let stdout = child
        .stdout
        .take()
        .ok_or(BeforeChangeError::Git(GitError::CommandFailed))?;
    let stderr_pipe = child.stderr.take();

    // Read stderr in a separate thread to prevent deadlock when pipe buffer fills
    let stderr_thread = std::thread::spawn(move || -> String {
        let Some(stderr) = stderr_pipe else {
            return String::new();
        };
        let mut buf = String::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_string(&mut buf);
        buf
    });

    let mut issues = HashSet::new();
    {
        let reader = BufReader::new(stdout);
        for line in reader.lines().take(MAX_GIT_OUTPUT_LINES) {
            let line = line.map_err(|_| BeforeChangeError::Git(GitError::CommandFailed))?;
            for num in crate::indexer::knowledge::extract_issue_numbers(&line) {
                issues.insert(num);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|_| BeforeChangeError::Git(GitError::CommandFailed))?;

    let stderr_output = stderr_thread.join().unwrap_or_default();

    if !status.success() {
        if stderr_output.contains("not a git repository") {
            return Err(BeforeChangeError::NotGitRepository);
        }
        return Err(BeforeChangeError::Git(GitError::CommandFailed));
    }

    let mut sorted: Vec<String> = issues.into_iter().collect();
    sorted.sort();
    Ok(sorted)
}

// ---------------------------------------------------------------------------
// Semantic ranking
// ---------------------------------------------------------------------------

/// Rank documents by max cosine similarity (max pooling).
/// Returns findings sorted by similarity descending, with None at the end.
fn rank_by_max_similarity(
    file_embs: &[EmbeddingRecord],
    docs: &[KnowledgeDocResult],
    embedding_store: &EmbeddingStore,
) -> Vec<BeforeChangeFinding> {
    let mut with_score: Vec<BeforeChangeFinding> = Vec::new();
    let mut without_score: Vec<BeforeChangeFinding> = Vec::new();

    for doc in docs {
        let doc_embs = match embedding_store.find_by_path(&doc.file_path) {
            Ok(embs) => embs,
            Err(_) => {
                // Non-fatal: treat as no embedding
                without_score.push(BeforeChangeFinding {
                    issue_number: doc.issue_number.clone(),
                    relation: doc.relation.to_string(),
                    doc_path: doc.file_path.clone(),
                    doc_title: doc.title.clone(),
                    similarity: None,
                });
                continue;
            }
        };

        if file_embs.is_empty() || doc_embs.is_empty() {
            without_score.push(BeforeChangeFinding {
                issue_number: doc.issue_number.clone(),
                relation: doc.relation.to_string(),
                doc_path: doc.file_path.clone(),
                doc_title: doc.title.clone(),
                similarity: None,
            });
            continue;
        }

        let mut max_sim: f32 = f32::NEG_INFINITY;
        for f_emb in file_embs {
            for d_emb in &doc_embs {
                let sim = cosine_similarity(&f_emb.embedding, &d_emb.embedding);
                if sim > max_sim {
                    max_sim = sim;
                }
            }
        }

        with_score.push(BeforeChangeFinding {
            issue_number: doc.issue_number.clone(),
            relation: doc.relation.to_string(),
            doc_path: doc.file_path.clone(),
            doc_title: doc.title.clone(),
            similarity: Some(max_sim),
        });
    }

    // Combine all findings
    let mut all_findings = with_score;
    all_findings.extend(without_score);

    // Compute issue-level max similarity
    let mut issue_max_sim: HashMap<String, f32> = HashMap::new();
    for finding in &all_findings {
        let sim = finding.similarity.unwrap_or(f32::NEG_INFINITY);
        let entry = issue_max_sim
            .entry(finding.issue_number.clone())
            .or_insert(f32::NEG_INFINITY);
        if sim > *entry {
            *entry = sim;
        }
    }

    // Sort by: issue max similarity descending, then issue_number, then relation_priority
    all_findings.sort_by(|a, b| {
        let a_issue_sim = issue_max_sim
            .get(&a.issue_number)
            .unwrap_or(&f32::NEG_INFINITY);
        let b_issue_sim = issue_max_sim
            .get(&b.issue_number)
            .unwrap_or(&f32::NEG_INFINITY);
        b_issue_sim
            .partial_cmp(a_issue_sim)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.issue_number.cmp(&b.issue_number))
            .then_with(|| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)))
    });

    all_findings
}

/// Create findings without semantic ranking (fallback).
/// Sorted by issue_number descending (newer issues first), then relation priority.
fn findings_without_ranking(docs: &[KnowledgeDocResult]) -> Vec<BeforeChangeFinding> {
    let mut findings: Vec<BeforeChangeFinding> = docs
        .iter()
        .map(|doc| BeforeChangeFinding {
            issue_number: doc.issue_number.clone(),
            relation: doc.relation.to_string(),
            doc_path: doc.file_path.clone(),
            doc_title: doc.title.clone(),
            similarity: None,
        })
        .collect();

    // Sort by issue_number descending (numeric), then relation priority
    findings.sort_by(|a, b| {
        let a_num = a.issue_number.parse::<u64>().unwrap_or(0);
        let b_num = b.issue_number.parse::<u64>().unwrap_or(0);
        b_num
            .cmp(&a_num)
            .then_with(|| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)))
    });

    findings
}

/// Relation priority for fallback sort (lower = higher priority).
fn relation_priority(relation: &str) -> u8 {
    match relation {
        "has_design" => 0,
        "has_workplan" => 1,
        "has_review" => 2,
        "has_progress" => 3,
        "modifies" => 4,
        _ => 5,
    }
}

// ---------------------------------------------------------------------------
// Issue-level grouping and limit
// ---------------------------------------------------------------------------

/// Group findings by issue, apply issue-level limit, and select representative
/// documents per issue. Input findings must be pre-sorted (by similarity or
/// issue_number). Issue order is determined by first occurrence in the input.
fn group_and_limit_by_issue(
    findings: Vec<BeforeChangeFinding>,
    limit: usize,
) -> Vec<BeforeChangeFinding> {
    // 1. Group by issue, preserving first-occurrence order
    let mut issue_order: Vec<String> = Vec::new();
    let mut issue_groups: HashMap<String, Vec<BeforeChangeFinding>> = HashMap::new();

    for finding in findings {
        if !issue_groups.contains_key(&finding.issue_number) {
            issue_order.push(finding.issue_number.clone());
        }
        issue_groups
            .entry(finding.issue_number.clone())
            .or_default()
            .push(finding);
    }

    // 2. Sort each issue's docs by relation_priority
    for docs in issue_groups.values_mut() {
        docs.sort_by(|a, b| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)));
    }

    // 3. Apply issue-level limit and select up to MAX_DOCS_PER_ISSUE per issue
    let mut result: Vec<BeforeChangeFinding> = Vec::new();
    for issue_num in issue_order.iter().take(limit) {
        if let Some(docs) = issue_groups.get(issue_num) {
            result.extend(docs.iter().take(MAX_DOCS_PER_ISSUE).cloned());
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub fn run_before_change(
    file: &str,
    format: OutputFormat,
    index_path: Option<&Path>,
    limit: usize,
    max_commits: usize,
) -> Result<(), BeforeChangeError> {
    // 1. Input validation
    validate_before_change_input(file)?;

    // 2. Git log scanning
    let issues = extract_issues_from_git_log(file, max_commits)?;

    if issues.is_empty() {
        // No issues found - output empty result
        let result = BeforeChangeResult {
            file_path: file.to_string(),
            findings: Vec::new(),
            total_issues: 0,
            displayed_issues: 0,
            has_embeddings: false,
        };
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        crate::output::format_before_change_results(&result, format, &mut handle)?;
        return Ok(());
    }

    // 3. Resolve index path
    let config = crate::config::load_config(Path::new("."))
        .map_err(|e| BeforeChangeError::Config(format!("{e}")))?;
    let config_index_path = config.index.path.as_deref();
    let commandindex_dir = crate::indexer::resolve_index_path(
        index_path,
        config_index_path,
        &std::env::current_dir()?,
    )?;

    // 4. Check symbol DB
    let db_path = crate::indexer::symbol_db_path(&commandindex_dir);
    if !db_path.exists() {
        return Err(BeforeChangeError::SymbolDbNotFound);
    }
    let store = SymbolStore::open(&db_path)?;

    // 5. Knowledge graph query
    let mut docs = store.find_knowledge_by_issue(&issues)?;
    // Filter out modifies entries (file nodes) - they are not relevant for before-change
    docs.retain(|d| d.relation != KnowledgeRelation::Modifies);

    // Compute total_issues as unique issue count with at least one document
    let total_issues = {
        let unique: HashSet<&str> = docs.iter().map(|d| d.issue_number.as_str()).collect();
        unique.len()
    };

    if docs.is_empty() {
        let result = BeforeChangeResult {
            file_path: file.to_string(),
            findings: Vec::new(),
            total_issues,
            displayed_issues: 0,
            has_embeddings: false,
        };
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        crate::output::format_before_change_results(&result, format, &mut handle)?;
        return Ok(());
    }

    // 6. Semantic ranking (optional)
    let emb_db_path = crate::indexer::embeddings_db_path(&commandindex_dir);
    let (findings, has_embeddings) = if emb_db_path.exists() {
        match EmbeddingStore::open(&emb_db_path) {
            Ok(emb_store) => {
                let file_embs = match emb_store.find_by_path(file) {
                    Ok(embs) => embs,
                    Err(e) => {
                        eprintln!("Warning: failed to load embeddings for '{file}': {e}");
                        Vec::new()
                    }
                };
                let findings = rank_by_max_similarity(&file_embs, &docs, &emb_store);
                (findings, !file_embs.is_empty())
            }
            Err(e) => {
                eprintln!("Warning: embedding store error: {e}");
                (findings_without_ranking(&docs), false)
            }
        }
    } else {
        (findings_without_ranking(&docs), false)
    };

    // 7. Apply limit (Issue-level)
    let limited_findings = group_and_limit_by_issue(findings, limit);

    // 8. Compute displayed_issues
    let displayed_issues = {
        let unique: HashSet<&str> = limited_findings
            .iter()
            .map(|f| f.issue_number.as_str())
            .collect();
        unique.len()
    };

    // 9. Output
    let result = BeforeChangeResult {
        file_path: file.to_string(),
        findings: limited_findings,
        total_issues,
        displayed_issues,
        has_embeddings,
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    crate::output::format_before_change_results(&result, format, &mut handle)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Input validation tests ---

    #[test]
    fn test_validate_valid_path() {
        assert!(validate_before_change_input("src/main.rs").is_ok());
        assert!(validate_before_change_input("docs/guide.md").is_ok());
    }

    #[test]
    fn test_validate_empty_path() {
        assert!(validate_before_change_input("").is_err());
    }

    #[test]
    fn test_validate_leading_dash() {
        let err = validate_before_change_input("-flag").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("must not start with '-'"));
    }

    #[test]
    fn test_validate_dotdot() {
        assert!(validate_before_change_input("../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_absolute_path() {
        assert!(validate_before_change_input("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_backslash() {
        assert!(validate_before_change_input("src\\main.rs").is_err());
    }

    // --- Findings fallback sort tests ---

    #[test]
    fn test_findings_without_ranking_sort_order() {
        use crate::indexer::knowledge::KnowledgeRelation;

        let docs = vec![
            KnowledgeDocResult {
                issue_number: "100".to_string(),
                relation: KnowledgeRelation::HasWorkplan,
                file_path: "wp.md".to_string(),
                title: None,
            },
            KnowledgeDocResult {
                issue_number: "100".to_string(),
                relation: KnowledgeRelation::HasDesign,
                file_path: "design.md".to_string(),
                title: None,
            },
            KnowledgeDocResult {
                issue_number: "100".to_string(),
                relation: KnowledgeRelation::HasReview,
                file_path: "review.md".to_string(),
                title: None,
            },
        ];

        let findings = findings_without_ranking(&docs);
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].relation, "has_design");
        assert_eq!(findings[1].relation, "has_workplan");
        assert_eq!(findings[2].relation, "has_review");
    }

    // --- Rank by max similarity tests ---

    #[test]
    fn test_rank_by_max_similarity_with_empty_file_embs() {
        use crate::indexer::knowledge::KnowledgeRelation;

        let docs = vec![KnowledgeDocResult {
            issue_number: "100".to_string(),
            relation: KnowledgeRelation::HasDesign,
            file_path: "design.md".to_string(),
            title: None,
        }];

        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("embeddings.db");
        let emb_store = EmbeddingStore::open(&db_path).unwrap();
        emb_store.create_tables().unwrap();

        let findings = rank_by_max_similarity(&[], &docs, &emb_store);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].similarity.is_none());
    }

    // --- BeforeChangeError Display tests ---

    #[test]
    fn test_error_display_invalid_input() {
        let err = BeforeChangeError::InvalidInput("bad input".to_string());
        assert_eq!(format!("{err}"), "bad input");
    }

    #[test]
    fn test_error_display_index_not_found() {
        let err = BeforeChangeError::IndexNotFound;
        assert!(format!("{err}").contains("Index not found"));
    }

    #[test]
    fn test_error_display_symbol_db_not_found() {
        let err = BeforeChangeError::SymbolDbNotFound;
        assert!(format!("{err}").contains("Symbol database not found"));
    }

    #[test]
    fn test_error_display_not_git_repository() {
        let err = BeforeChangeError::NotGitRepository;
        assert!(format!("{err}").contains("Not a git repository"));
    }

    // --- relation_priority tests ---

    #[test]
    fn test_relation_priority_order() {
        assert!(relation_priority("has_design") < relation_priority("has_workplan"));
        assert!(relation_priority("has_workplan") < relation_priority("has_review"));
        assert!(relation_priority("has_review") < relation_priority("has_progress"));
        assert!(relation_priority("has_progress") < relation_priority("modifies"));
        assert!(relation_priority("modifies") < relation_priority("unknown"));
    }

    // --- findings_without_ranking descending tests ---

    #[test]
    fn test_findings_without_ranking_descending() {
        use crate::indexer::knowledge::KnowledgeRelation;

        let docs = vec![
            KnowledgeDocResult {
                issue_number: "50".to_string(),
                relation: KnowledgeRelation::HasDesign,
                file_path: "d50.md".to_string(),
                title: None,
            },
            KnowledgeDocResult {
                issue_number: "200".to_string(),
                relation: KnowledgeRelation::HasDesign,
                file_path: "d200.md".to_string(),
                title: None,
            },
            KnowledgeDocResult {
                issue_number: "100".to_string(),
                relation: KnowledgeRelation::HasDesign,
                file_path: "d100.md".to_string(),
                title: None,
            },
        ];

        let findings = findings_without_ranking(&docs);
        assert_eq!(findings.len(), 3);
        // Should be descending: 200, 100, 50
        assert_eq!(findings[0].issue_number, "200");
        assert_eq!(findings[1].issue_number, "100");
        assert_eq!(findings[2].issue_number, "50");
    }

    // --- group_and_limit_by_issue tests ---

    #[test]
    fn test_group_and_limit_by_issue_basic() {
        // 3 issues, limit=2 -> only 2 issues returned
        let findings = vec![
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_workplan".to_string(),
                doc_path: "w100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_review".to_string(),
                doc_path: "r100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "200".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d200.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "300".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d300.md".to_string(),
                doc_title: None,
                similarity: None,
            },
        ];

        let result = group_and_limit_by_issue(findings, 2);
        // Should have issue 100 (2 docs) + issue 200 (1 doc) = 3 docs
        let unique_issues: HashSet<&str> = result.iter().map(|f| f.issue_number.as_str()).collect();
        assert_eq!(unique_issues.len(), 2);
        assert!(unique_issues.contains("100"));
        assert!(unique_issues.contains("200"));
        // Issue 300 should be excluded
        assert!(!unique_issues.contains("300"));
    }

    #[test]
    fn test_group_and_limit_by_issue_max_docs() {
        // Issue with 4 docs -> only MAX_DOCS_PER_ISSUE (2) selected
        let findings = vec![
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_review".to_string(),
                doc_path: "r100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_workplan".to_string(),
                doc_path: "w100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "modifies".to_string(),
                doc_path: "m100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
        ];

        let result = group_and_limit_by_issue(findings, 10);
        assert_eq!(result.len(), MAX_DOCS_PER_ISSUE);
        // After sorting by relation_priority: has_design, has_workplan
        assert_eq!(result[0].relation, "has_design");
        assert_eq!(result[1].relation, "has_workplan");
    }

    #[test]
    fn test_group_and_limit_by_issue_preserves_order() {
        // Input order: issue 200 first, then issue 100 -> output preserves this
        let findings = vec![
            BeforeChangeFinding {
                issue_number: "200".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d200.md".to_string(),
                doc_title: None,
                similarity: None,
            },
            BeforeChangeFinding {
                issue_number: "100".to_string(),
                relation: "has_design".to_string(),
                doc_path: "d100.md".to_string(),
                doc_title: None,
                similarity: None,
            },
        ];

        let result = group_and_limit_by_issue(findings, 10);
        assert_eq!(result.len(), 2);
        // Order preserved: 200 before 100
        assert_eq!(result[0].issue_number, "200");
        assert_eq!(result[1].issue_number, "100");
    }
}

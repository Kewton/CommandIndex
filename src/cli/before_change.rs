pub const BEFORE_CHANGE_AFTER_HELP: &str = "\
When to use:
  Before modifying a file, to understand related design decisions and review history.
  Shows design constraints, review findings, and work plans linked via knowledge graph.

Examples:
  commandindexdev before-change src/auth.rs
  commandindexdev before-change src/auth.rs --format json
  commandindexdev before-change src/auth.rs --format llm --limit 5
  commandindexdev before-change src/auth.rs --max-commits 500";

use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use crate::cli::git::GitError;
use crate::cli::stdin::{StdinError, validate_file_path};
use crate::embedding::store::{EmbeddingRecord, EmbeddingStore, cosine_similarity};
use crate::indexer::ResolveIndexPathError;
use crate::indexer::symbol_store::{KnowledgeDocResult, SymbolStore, SymbolStoreError};
use crate::output::{BeforeChangeFinding, BeforeChangeResult, OutputError, OutputFormat};

/// Maximum lines to read from git log output
const MAX_GIT_OUTPUT_LINES: usize = 5000;

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
/// Statically compiled regex for issue number extraction.
static ISSUE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:#(\d+)|\(#(\d+)\)|fixes\s+#(\d+)|refs\s+#(\d+))")
        .expect("ISSUE_RE is a valid regex literal")
});

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
            for cap in ISSUE_RE.captures_iter(&line) {
                // Each capture group corresponds to a different pattern
                for i in 1..=4 {
                    if let Some(m) = cap.get(i) {
                        issues.insert(m.as_str().to_string());
                    }
                }
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

    // Sort with_score by similarity descending
    with_score.sort_by(|a, b| {
        b.similarity
            .unwrap_or(0.0)
            .partial_cmp(&a.similarity.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sort without_score by issue_number, then relation
    without_score.sort_by(|a, b| {
        a.issue_number
            .cmp(&b.issue_number)
            .then_with(|| a.relation.cmp(&b.relation))
    });

    with_score.extend(without_score);
    with_score
}

/// Create findings without semantic ranking (fallback).
/// Sorted by issue_number, then relation priority (has_design > has_review > has_workplan).
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

    findings.sort_by(|a, b| {
        a.issue_number
            .cmp(&b.issue_number)
            .then_with(|| relation_priority(&a.relation).cmp(&relation_priority(&b.relation)))
    });

    findings
}

/// Relation priority for fallback sort (lower = higher priority).
fn relation_priority(relation: &str) -> u8 {
    match relation {
        "has_design" => 0,
        "has_review" => 1,
        "has_workplan" => 2,
        _ => 3,
    }
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
    let docs = store.find_knowledge_by_issue(&issues)?;

    if docs.is_empty() {
        let result = BeforeChangeResult {
            file_path: file.to_string(),
            findings: Vec::new(),
            total_issues: issues.len(),
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

    // 7. Apply limit
    let limited_findings: Vec<BeforeChangeFinding> = findings.into_iter().take(limit).collect();

    // 8. Output
    let result = BeforeChangeResult {
        file_path: file.to_string(),
        findings: limited_findings,
        total_issues: issues.len(),
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
        assert_eq!(findings[1].relation, "has_review");
        assert_eq!(findings[2].relation, "has_workplan");
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
}

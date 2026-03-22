use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::cli::status::git_info::validate_commit_hash;
use crate::cli::stdin::validate_file_path;

/// Git operation errors
#[derive(Debug)]
pub enum GitError {
    GitNotFound,
    CommandFailed,
    InvalidInput(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::GitNotFound => write!(f, "git command not found"),
            GitError::CommandFailed => write!(f, "git command failed"),
            GitError::InvalidInput(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for GitError {}

/// Maximum lines to read from git log output
const MAX_GIT_OUTPUT_LINES: usize = 5000;

/// Validate --changed-since input value
pub fn validate_changed_since_input(input: &str) -> Result<(), GitError> {
    if input.is_empty() {
        return Err(GitError::InvalidInput(
            "input must not be empty".to_string(),
        ));
    }
    if input.len() > 256 {
        return Err(GitError::InvalidInput(
            "input too long (max 256 chars)".to_string(),
        ));
    }
    if input.bytes().any(|b| b < 0x20) {
        return Err(GitError::InvalidInput(
            "control characters not allowed".to_string(),
        ));
    }
    if input.starts_with('-') {
        return Err(GitError::InvalidInput(
            "input must not start with '-'".to_string(),
        ));
    }
    Ok(())
}

/// Get list of files changed since a given point (time expression or commit hash)
pub fn get_changed_files(repo_path: &Path, since: &str) -> Result<Vec<String>, GitError> {
    validate_changed_since_input(since)?;

    let is_commit_hash = validate_commit_hash(since);
    let since_arg = if is_commit_hash {
        format!("{since}..HEAD")
    } else {
        format!("--since={since}")
    };

    let args: Vec<&str> = if is_commit_hash {
        vec!["log", "--name-only", "--format=", &since_arg]
    } else {
        vec!["log", &since_arg, "--name-only", "--format="]
    };

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| GitError::GitNotFound)?;

    let stdout = child.stdout.take().ok_or(GitError::CommandFailed)?;
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();

    // Scope BufReader so it is dropped (and stdout pipe closed) before child.wait()
    // This prevents potential pipe hang if the child is still writing.
    {
        let reader = BufReader::new(stdout);
        for line in reader.lines().take(MAX_GIT_OUTPUT_LINES) {
            let line = line.map_err(|_| GitError::CommandFailed)?;
            if line.is_empty() {
                continue;
            }
            // Validate each path from git log output to prevent path traversal
            if validate_file_path(&line).is_err() {
                continue;
            }
            if seen.insert(line.clone()) {
                files.push(line);
            }
        }
    }

    let status = child.wait().map_err(|_| GitError::CommandFailed)?;
    if !status.success() {
        return Err(GitError::CommandFailed);
    }

    files.sort();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_changed_since_input tests ---

    #[test]
    fn valid_time_expression() {
        assert!(validate_changed_since_input("12 hours ago").is_ok());
    }

    #[test]
    fn valid_yesterday() {
        assert!(validate_changed_since_input("yesterday").is_ok());
    }

    #[test]
    fn valid_commit_hash_input() {
        assert!(validate_changed_since_input("abc1234").is_ok());
    }

    #[test]
    fn valid_full_commit_hash() {
        assert!(validate_changed_since_input("a".repeat(40).as_str()).is_ok());
    }

    #[test]
    fn reject_empty_input() {
        let err = validate_changed_since_input("").unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
    }

    #[test]
    fn reject_leading_dash() {
        let err = validate_changed_since_input("--exec=evil").unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
        let msg = format!("{err}");
        assert!(msg.contains("start with '-'"));
    }

    #[test]
    fn reject_control_characters() {
        let err = validate_changed_since_input("12 hours\x00ago").unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
        let msg = format!("{err}");
        assert!(msg.contains("control characters"));
    }

    #[test]
    fn reject_newline() {
        let err = validate_changed_since_input("12 hours\nago").unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
    }

    #[test]
    fn reject_too_long_input() {
        let long_input = "a".repeat(257);
        let err = validate_changed_since_input(&long_input).unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
        let msg = format!("{err}");
        assert!(msg.contains("too long"));
    }

    #[test]
    fn accept_256_chars() {
        let input = "a".repeat(256);
        assert!(validate_changed_since_input(&input).is_ok());
    }

    #[test]
    fn reject_tab_character() {
        let err = validate_changed_since_input("12\thours ago").unwrap_err();
        assert!(matches!(err, GitError::InvalidInput(_)));
    }
}

mod common;

use predicates::prelude::*;

/// Create a temp dir with git repo, commit files, build index, then add a second commit.
/// Returns (dir, first_commit_hash).
fn setup_changed_since() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");

    // First commit with initial files
    let first_hash = common::git_init_with_commit(dir.path());

    // Build index
    common::run_index(dir.path());

    // Add a second file and commit
    std::fs::write(
        dir.path().join("docs").join("c.md"),
        "---\ntags: security\n---\n# Page C\nNew page after index\n",
    )
    .unwrap();

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("git add");

    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "add c.md",
        ])
        .current_dir(dir.path())
        .output()
        .expect("git commit");
    assert!(output.status.success(), "second git commit failed");

    // Re-index to include c.md
    common::run_index(dir.path());

    (dir, first_hash)
}

#[test]
fn changed_since_commit_hash_json() {
    let (dir, first_hash) = setup_changed_since();
    let output = common::cmd()
        .args(["search", "--changed-since", &first_hash, "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // changed_files should contain docs/c.md
    let changed = parsed["changed_files"].as_array().unwrap();
    let has_c = changed
        .iter()
        .any(|v| v.as_str().map(|s| s.ends_with("c.md")).unwrap_or(false));
    assert!(has_c, "changed_files should contain c.md, got: {changed:?}");
}

#[test]
fn changed_since_no_changes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let _hash = common::git_init_with_commit(dir.path());
    common::run_index(dir.path());

    // Use "1 second ago" - since no commits after last one, should show no changes
    // Actually use the HEAD hash itself - no changes since HEAD
    let hash_output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .expect("git rev-parse HEAD");
    let head_hash = String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string();

    let output = common::cmd()
        .args(["search", "--changed-since", &head_hash])
        .current_dir(dir.path())
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("No files changed"),
        "should show no files changed message, got: {stderr}"
    );
}

#[test]
fn changed_since_human_output() {
    let (dir, first_hash) = setup_changed_since();
    let output = common::cmd()
        .args([
            "search",
            "--changed-since",
            &first_hash,
            "--format",
            "human",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("Impact analysis") || stdout.contains("changed file"),
        "human format should contain impact text, got: {stdout}"
    );
}

#[test]
fn changed_since_path_output() {
    let (dir, first_hash) = setup_changed_since();
    // path format outputs impacted file paths (related files of changed files)
    // The changed file (c.md) may or may not have related files depending on index content.
    // Just verify the command succeeds without error.
    common::cmd()
        .args(["search", "--changed-since", &first_hash, "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn changed_since_rejects_leading_dash() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let _hash = common::git_init_with_commit(dir.path());
    common::run_index(dir.path());

    common::cmd()
        .args(["search", "--changed-since", "--exec=evil"])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn changed_since_not_git_repo() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // Create some files and index, but no git repo
    std::fs::write(dir.path().join("test.md"), "# Test\nContent\n").unwrap();
    common::run_index(dir.path());

    common::cmd()
        .args(["search", "--changed-since", "12 hours ago"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("git command failed"));
}

// Note: NUL bytes cannot be passed as command-line arguments (OS limitation),
// so control character validation is tested via unit tests in src/cli/git.rs.

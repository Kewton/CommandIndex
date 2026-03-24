mod common;

use predicates::prelude::*;

/// Helper: create git repo with a commit referencing issue #100, build index, insert knowledge entries
fn setup_before_change_env() -> tempfile::TempDir {
    use std::process::Command;

    let dir = tempfile::tempdir().expect("create temp dir");
    let base = dir.path();

    // 1. git init
    Command::new("git")
        .args(["init"])
        .current_dir(base)
        .output()
        .expect("git init");

    // 2. Create source file
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("auth.rs"), "fn authenticate() {}\n").unwrap();

    // 3. Create dev-reports for knowledge graph
    let design_dir = base.join("dev-reports/design");
    std::fs::create_dir_all(&design_dir).unwrap();
    std::fs::write(
        design_dir.join("issue-100-auth-design-policy.md"),
        "# Design Policy for Auth\n\nAuthentication flow design.\n",
    )
    .unwrap();

    let issue_dir = base.join("dev-reports/issue/100");
    std::fs::create_dir_all(&issue_dir).unwrap();
    std::fs::write(
        issue_dir.join("work-plan.md"),
        "# Work Plan\n\nTasks for issue 100.\n",
    )
    .unwrap();

    // 4. git add + commit referencing #100
    Command::new("git")
        .args(["add", "."])
        .current_dir(base)
        .output()
        .expect("git add");

    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "feat(auth): add authentication (#100)",
        ])
        .current_dir(base)
        .output()
        .expect("git commit");
    assert!(output.status.success(), "git commit failed");

    // 5. Build index (this also populates knowledge graph)
    common::run_index(base);

    dir
}

#[test]
fn before_change_normal_human_output() {
    let dir = setup_before_change_env();
    let output = common::cmd()
        .args(["before-change", "src/auth.rs", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Before-change"));
    assert!(stdout.contains("src/auth.rs"));
}

#[test]
fn before_change_json_output() {
    let dir = setup_before_change_env();
    let output = common::cmd()
        .args(["before-change", "src/auth.rs", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["file_path"], "src/auth.rs");
    assert!(parsed["total_issues"].as_u64().unwrap() >= 1);
    assert!(parsed["findings"].as_array().is_some());
}

#[test]
fn before_change_path_output() {
    let dir = setup_before_change_env();
    let output = common::cmd()
        .args(["before-change", "src/auth.rs", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    // Each line should be a doc path
    for line in &lines {
        assert!(
            line.contains("dev-reports/"),
            "path format should only show doc paths, got: {line}"
        );
    }
}

#[test]
fn before_change_llm_output() {
    let dir = setup_before_change_env();
    let output = common::cmd()
        .args(["before-change", "src/auth.rs", "--format", "llm"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Before-change"));
}

#[test]
fn before_change_no_index_shows_error() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Create a minimal git repo
    use std::process::Command;
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init");

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("git add");

    Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "init (#1)",
        ])
        .current_dir(dir.path())
        .output()
        .expect("git commit");

    common::cmd()
        .args(["before-change", "src/main.rs"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Symbol database not found"));
}

#[test]
fn before_change_invalid_input_empty() {
    common::cmd()
        .args(["before-change", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("empty"));
}

#[test]
fn before_change_invalid_input_leading_dash() {
    common::cmd()
        .args(["before-change", "--", "-flag"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must not start with '-'"));
}

#[test]
fn before_change_non_git_repo() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // No git init - not a git repo

    // Need index first though - create one
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();
    common::run_index(dir.path());

    common::cmd()
        .args(["before-change", "src/main.rs"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Not a git repository")
                .or(predicate::str::contains("git command failed")),
        );
}

#[test]
fn before_change_no_issues_found() {
    use std::process::Command;

    let dir = tempfile::tempdir().expect("create temp dir");

    // Create git repo with commit that has no issue reference
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init");

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .expect("git add");

    Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "initial commit without issue reference",
        ])
        .current_dir(dir.path())
        .output()
        .expect("git commit");

    common::run_index(dir.path());

    let output = common::cmd()
        .args(["before-change", "src/main.rs", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["total_issues"], 0);
    assert!(parsed["findings"].as_array().unwrap().is_empty());
}

#[test]
fn before_change_limit_respected() {
    let dir = setup_before_change_env();
    let output = common::cmd()
        .args([
            "before-change",
            "src/auth.rs",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let findings = parsed["findings"].as_array().unwrap();
    assert!(
        findings.len() <= 1,
        "limit=1 should return at most 1 finding, got {}",
        findings.len()
    );
}

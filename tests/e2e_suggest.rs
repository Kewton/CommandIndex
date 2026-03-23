mod common;

use predicates::prelude::*;

#[test]
fn suggest_with_index_human_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    // Set up git repo and index
    common::git_init_with_commit(tmp.path());
    common::run_index(tmp.path());

    common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", "authentication feature"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Suggested search strategy:"));
}

#[test]
fn suggest_with_index_json_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::git_init_with_commit(tmp.path());
    common::run_index(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", "authentication", "--format", "json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed["strategy"].is_array(), "strategy should be an array");
    assert_eq!(parsed["query"], "authentication");
}

#[test]
fn suggest_with_index_path_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::git_init_with_commit(tmp.path());
    common::run_index(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", "authentication", "--format", "path"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    // Path format should have at least one line of command output
    assert!(
        !stdout.trim().is_empty(),
        "Path format should produce output"
    );
}

#[test]
fn suggest_without_index_shows_error() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", "test task"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Index not found").or(predicate::str::contains("not found")),
        );
}

#[test]
fn suggest_empty_for_shows_validation_error() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid input"));
}

#[test]
fn suggest_whitespace_for_shows_validation_error() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["suggest", "--for", "   "])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid input"));
}

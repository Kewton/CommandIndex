mod common;

use predicates::prelude::*;

#[test]
fn verify_normal_index() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n\nContent\n").unwrap();
    common::run_index(dir.path());

    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap(), "--verify"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Index Verification"))
        .stdout(predicate::str::contains("State:     OK"))
        .stdout(predicate::str::contains("Tantivy:   OK"));
}

#[test]
fn verify_corrupted_index() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n\nContent\n").unwrap();
    common::run_index(dir.path());

    // Corrupt tantivy directory by removing it
    let tantivy_dir = dir.path().join(".commandindex").join("tantivy");
    std::fs::remove_dir_all(&tantivy_dir).unwrap();

    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap(), "--verify"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Tantivy:   FAIL"));
}

#[test]
fn verify_json_format() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n\nContent\n").unwrap();
    common::run_index(dir.path());

    let output = common::cmd()
        .args([
            "status",
            "--path",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--verify",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert!(
        parsed.get("state_valid").is_some(),
        "should contain state_valid key"
    );
    assert!(
        parsed["state_valid"].as_bool().unwrap(),
        "state should be valid"
    );
    assert!(
        parsed["tantivy_valid"].as_bool().unwrap(),
        "tantivy should be valid"
    );
}

#[test]
fn verify_without_flag_no_verify_output() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n\nContent\n").unwrap();
    common::run_index(dir.path());

    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Index Verification").not());
}

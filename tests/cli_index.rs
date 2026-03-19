mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_test_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Create a markdown file
    fs::write(
        dir.path().join("test.md"),
        "---\ntags:\n  - rust\n  - cli\n---\n# Hello\n\nWorld\n\n## Section 2\n\nBody text\n",
    )
    .unwrap();

    dir
}

fn setup_test_dir_multiple() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    fs::write(dir.path().join("file1.md"), "# File 1\n\nContent 1\n").unwrap();

    fs::write(dir.path().join("file2.md"), "# File 2\n\nContent 2\n").unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    fs::write(dir.path().join("sub/file3.md"), "# File 3\n\nContent 3\n").unwrap();

    dir
}

#[test]
fn index_creates_commandindex_dir() {
    let dir = setup_test_dir();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(dir.path().join(".commandindex").is_dir());
}

#[test]
fn index_creates_tantivy_index() {
    let dir = setup_test_dir();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(dir.path().join(".commandindex/tantivy").is_dir());
}

#[test]
fn index_creates_manifest() {
    let dir = setup_test_dir();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let manifest_path = dir.path().join(".commandindex/manifest.json");
    assert!(manifest_path.exists());

    let content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Should have 1 file entry
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "test.md");
    assert!(files[0]["hash"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(files[0]["sections"], 2); // # Hello + ## Section 2
}

#[test]
fn index_creates_state() {
    let dir = setup_test_dir();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    let state_path = dir.path().join(".commandindex/state.json");
    assert!(state_path.exists());

    let content = fs::read_to_string(&state_path).unwrap();
    let state: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(state["total_files"], 1);
    assert_eq!(state["total_sections"], 2);
    assert_eq!(state["schema_version"], 1);
}

#[test]
fn index_applies_cmindexignore() {
    let dir = setup_test_dir_multiple();

    // Create .cmindexignore that excludes sub/
    fs::write(dir.path().join(".cmindexignore"), "sub/\n").unwrap();

    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ignored: 1 files"));

    let manifest_path = dir.path().join(".commandindex/manifest.json");
    let content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

    // sub/file3.md should be excluded
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn index_displays_summary() {
    let dir = setup_test_dir();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned: 1 files"))
        .stdout(predicate::str::contains("Indexed: 2 sections"))
        .stdout(predicate::str::contains("Duration:"))
        .stdout(predicate::str::contains("Index saved to .commandindex/"));
}

#[test]
fn index_rebuilds_on_existing() {
    let dir = setup_test_dir();
    let path_str = dir.path().to_str().unwrap();

    // First index
    common::cmd()
        .args(["index", "--path", path_str])
        .assert()
        .success();

    // Add another file
    fs::write(dir.path().join("second.md"), "# Second\n\nAnother file\n").unwrap();

    // Second index (rebuild)
    common::cmd()
        .args(["index", "--path", path_str])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned: 2 files"));

    let manifest_path = dir.path().join(".commandindex/manifest.json");
    let content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
}

#[test]
fn index_with_path_option() {
    let dir = setup_test_dir();

    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned: 1 files"));
}

#[test]
fn index_nonexistent_path() {
    common::cmd()
        .args(["index", "--path", "/nonexistent/path/that/does/not/exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:"));
}

#[test]
fn index_empty_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned: 0 files"))
        .stdout(predicate::str::contains("Indexed: 0 sections"));
}

#[test]
fn index_multiple_files_with_subdirectories() {
    let dir = setup_test_dir_multiple();

    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scanned: 3 files"))
        .stdout(predicate::str::contains("Indexed: 3 sections"));
}

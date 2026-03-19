mod common;
use predicates::prelude::*;

#[test]
fn clean_removes_commandindex_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // First, create an index
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();
    assert!(dir.path().join(".commandindex").is_dir());

    // Then clean
    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed index at .commandindex/"));

    assert!(!dir.path().join(".commandindex").exists());
}

#[test]
fn clean_with_no_index_succeeds() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No index found. Nothing to clean.",
        ));
}

#[test]
fn clean_with_path_option() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(!dir.path().join(".commandindex").exists());
}

#[test]
fn clean_default_path_is_current_dir() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // Just verify clean runs without error on a dir with no index
    common::cmd()
        .current_dir(dir.path())
        .arg("clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("No index found"));
}

#[test]
fn clean_then_reindex_succeeds() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // Create a markdown file for indexing
    std::fs::write(dir.path().join("test.md"), "# Test\nContent").unwrap();

    // index -> clean -> index round trip
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(!dir.path().join(".commandindex").exists());

    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(dir.path().join(".commandindex").is_dir());
}

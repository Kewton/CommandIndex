mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create the shared E2E update test directory structure.
///
/// ```text
/// ├── guide.md   (Japanese content)
/// └── api.md     (API reference)
/// ```
fn setup_update_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    fs::write(
        dir.path().join("guide.md"),
        "# ガイド\n\nガイドの本文です。Rustプログラミング。\n",
    )
    .unwrap();

    fs::write(
        dir.path().join("api.md"),
        "# API\n\nAPIリファレンスの本文です。\n",
    )
    .unwrap();

    dir
}

#[test]
fn e2e_update_add_new_file() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Add a new file
    fs::write(
        dir.path().join("new.md"),
        "# 新規\n\n新しいコンテンツ追加。\n",
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update and assert it reports the addition
    common::run_update(dir.path())
        .success()
        .stdout(predicate::str::contains("Added:"));

    // Verify new content is searchable
    let results = common::run_search_jsonl(dir.path(), "新規");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("new.md")));
    assert!(
        found,
        "search for '新規' should find new.md, got: {results:?}"
    );
}

#[test]
fn e2e_update_modify_file() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Overwrite guide.md with new content
    fs::write(
        dir.path().join("guide.md"),
        "# ガイド改訂\n\n改訂されたガイド内容。\n",
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update and assert it reports modification
    common::run_update(dir.path())
        .success()
        .stdout(predicate::str::contains("Modified:"));

    // Verify new content is searchable
    let results = common::run_search_jsonl(dir.path(), "改訂");
    assert!(
        !results.is_empty(),
        "search for '改訂' should return results"
    );

    // Verify old content is no longer found in guide.md
    let results = common::run_search_jsonl(dir.path(), "Rustプログラミング");
    let guide_found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("guide.md")));
    assert!(
        !guide_found,
        "search for 'Rustプログラミング' should NOT find guide.md after modification"
    );
}

#[test]
fn e2e_update_delete_file() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Delete api.md
    fs::remove_file(dir.path().join("api.md")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update and assert it reports deletion
    common::run_update(dir.path())
        .success()
        .stdout(predicate::str::contains("Deleted:"));

    // Verify deleted content is not found
    let results = common::run_search_jsonl(dir.path(), "APIリファレンス");
    let api_found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("api.md")));
    assert!(
        !api_found,
        "search for 'APIリファレンス' should NOT find api.md after deletion"
    );
}

#[test]
fn e2e_update_no_changes() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Run update with no changes
    common::run_update(dir.path()).success().stdout(
        predicate::str::contains("Added:")
            .and(predicate::str::contains("0 files"))
            .and(predicate::str::contains("Modified:"))
            .and(predicate::str::contains("Deleted:"))
            .and(predicate::str::contains("Unchanged:")),
    );
}

#[test]
fn e2e_update_no_index_shows_error() {
    let dir = setup_update_dir();
    // Do NOT run index first — no existing index

    common::cmd()
        .args(["update", "--path", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No index found"));
}

#[test]
fn e2e_update_status_after_add() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Capture status before adding a file
    let before = common::run_status_json(dir.path());
    let before_files = before["total_files"].as_u64().unwrap();
    let before_sections = before["total_sections"].as_u64().unwrap();

    // Add a new file
    fs::write(dir.path().join("extra.md"), "# Extra\n\nExtra content.\n").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    common::run_update(dir.path()).success();

    // Capture status after
    let after = common::run_status_json(dir.path());
    let after_files = after["total_files"].as_u64().unwrap();
    let after_sections = after["total_sections"].as_u64().unwrap();

    assert_eq!(
        after_files,
        before_files + 1,
        "total_files should increase by 1"
    );
    assert!(
        after_sections > before_sections,
        "total_sections should increase by at least 1 (before={before_sections}, after={after_sections})"
    );
}

#[test]
fn e2e_update_cmindexignore() {
    let dir = setup_update_dir();
    common::run_index(dir.path());

    // Create .cmindexignore that excludes api.md
    fs::write(
        dir.path().join(".cmindexignore"),
        ".commandindex/\napi.md\n",
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update — api.md should be reported as deleted
    common::run_update(dir.path())
        .success()
        .stdout(predicate::str::contains("Deleted:").and(predicate::str::contains("1 files")));

    // Verify api.md content is no longer searchable
    let results = common::run_search_jsonl(dir.path(), "APIリファレンス");
    assert!(
        results.is_empty(),
        "search for 'APIリファレンス' should return empty after api.md is ignored, got: {results:?}"
    );
}

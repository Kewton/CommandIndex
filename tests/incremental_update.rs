mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_single_file() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(
        dir.path().join("doc.md"),
        "# Original Title\n\nOriginal content\n",
    )
    .unwrap();
    dir
}

fn run_index(dir: &TempDir) {
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

fn run_update(dir: &TempDir) -> assert_cmd::assert::Assert {
    common::cmd()
        .args(["update", "--path", dir.path().to_str().unwrap()])
        .assert()
}

#[test]
fn update_adds_new_files() {
    let dir = setup_single_file();
    run_index(&dir);

    // Add a new file
    fs::write(
        dir.path().join("new.md"),
        "# New File\n\nNew content about Rust programming\n",
    )
    .unwrap();

    run_update(&dir)
        .success()
        .stdout(predicate::str::contains("Added:").and(predicate::str::contains("1 files")));

    // Verify new content is searchable
    common::cmd()
        .args(["search", "Rust programming", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("New File"));
}

#[test]
fn update_modifies_existing_files() {
    let dir = setup_single_file();
    run_index(&dir);

    // Modify the file
    fs::write(
        dir.path().join("doc.md"),
        "# Updated Title\n\nUpdated content about Kubernetes\n",
    )
    .unwrap();

    run_update(&dir)
        .success()
        .stdout(predicate::str::contains("Modified:").and(predicate::str::contains("1 files")));

    // Verify updated content is searchable
    common::cmd()
        .args(["search", "Kubernetes", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated Title"));

    // Verify old content is NOT searchable
    common::cmd()
        .args(["search", "Original content", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Original Title").not());
}

#[test]
fn update_removes_deleted_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("keep.md"), "# Keep\n\nThis stays\n").unwrap();
    fs::write(
        dir.path().join("remove.md"),
        "# Remove\n\nThis will be removed\n",
    )
    .unwrap();
    run_index(&dir);

    // Delete one file
    fs::remove_file(dir.path().join("remove.md")).unwrap();

    run_update(&dir)
        .success()
        .stdout(predicate::str::contains("Deleted:").and(predicate::str::contains("1 files")));

    // Verify deleted content is NOT searchable
    common::cmd()
        .args(["search", "removed", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Remove").not());
}

#[test]
fn update_skips_unchanged_files() {
    let dir = setup_single_file();
    run_index(&dir);

    // No changes, run update
    run_update(&dir)
        .success()
        .stdout(predicate::str::contains("Unchanged: 1"));
}

#[test]
fn update_fallback_to_full_index() {
    let dir = setup_single_file();
    // Do NOT run index first - no existing index

    run_update(&dir)
        .success()
        .stderr(predicate::str::contains("full index"));
}

#[test]
fn update_manifest_updated() {
    let dir = setup_single_file();
    run_index(&dir);

    // Add a new file
    fs::write(dir.path().join("extra.md"), "# Extra\n\nExtra content\n").unwrap();

    run_update(&dir).success();

    // Check manifest
    let manifest_path = dir.path().join(".commandindex/manifest.json");
    let content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

    let files = manifest["files"].as_array().unwrap();
    assert_eq!(
        files.len(),
        2,
        "manifest should have 2 files after adding one"
    );

    // Check that extra.md is in the manifest
    let has_extra = files.iter().any(|f| f["path"].as_str() == Some("extra.md"));
    assert!(has_extra, "manifest should contain extra.md");
}

#[test]
fn update_state_no_underflow() {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("keep.md"), "# Keep\n\nKeep content\n").unwrap();
    fs::write(dir.path().join("remove.md"), "# Remove\n\nRemove content\n").unwrap();
    run_index(&dir);

    // Corrupt state.json: set total_files and total_sections to 0
    let state_path = dir.path().join(".commandindex/state.json");
    let state_content = fs::read_to_string(&state_path).unwrap();
    let mut state: serde_json::Value = serde_json::from_str(&state_content).unwrap();
    state["total_files"] = serde_json::Value::from(0u64);
    state["total_sections"] = serde_json::Value::from(0u64);
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Delete one file — this would cause underflow without saturating_sub
    fs::remove_file(dir.path().join("remove.md")).unwrap();

    // Should not panic, should succeed
    run_update(&dir).success();

    // Verify state values are >= 0 (u64 so always true, but not wrapped to huge values)
    let final_state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    let total_files = final_state["total_files"].as_u64().unwrap();
    let total_sections = final_state["total_sections"].as_u64().unwrap();

    // With saturating_sub, values should be 0 (not wrapped to u64::MAX - N)
    assert!(
        total_files <= 10,
        "total_files should not be a huge wrapped value, got {total_files}"
    );
    assert!(
        total_sections <= 10,
        "total_sections should not be a huge wrapped value, got {total_sections}"
    );
}

#[test]
fn update_state_updated() {
    let dir = setup_single_file();
    run_index(&dir);

    // Read initial state
    let state_path = dir.path().join(".commandindex/state.json");
    let initial_state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();
    let initial_updated = initial_state["last_updated_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Add a new file
    fs::write(dir.path().join("extra.md"), "# Extra\n\nExtra content\n").unwrap();

    // Small delay to ensure timestamp differs
    std::thread::sleep(std::time::Duration::from_millis(100));

    run_update(&dir).success();

    // Check state
    let updated_state: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&state_path).unwrap()).unwrap();

    assert_eq!(updated_state["total_files"], 2);
    assert_eq!(updated_state["total_sections"], 2);

    let updated_at = updated_state["last_updated_at"].as_str().unwrap();
    assert_ne!(
        updated_at, initial_updated,
        "last_updated_at should change after update"
    );
}

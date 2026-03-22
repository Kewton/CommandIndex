mod common;

use predicates::prelude::*;

// ===== Helper functions =====

/// Create a commandindex.toml (team shared config) in the given directory.
fn write_commandindex_toml(base_path: &std::path::Path, content: &str) {
    std::fs::write(base_path.join("commandindex.toml"), content).expect("write commandindex.toml");
}

/// Create .commandindex/config.local.toml (local personal config) in the given directory.
fn write_config_local_toml(base_path: &std::path::Path, content: &str) {
    let dir = base_path.join(".commandindex");
    std::fs::create_dir_all(&dir).expect("create .commandindex");
    std::fs::write(dir.join("config.local.toml"), content).expect("write config.local.toml");
}

/// Set up a test markdown file for indexing.
fn setup_test_markdown(base_path: &std::path::Path) {
    std::fs::write(
        base_path.join("guide.md"),
        "# Team Guide\n\nThis is the team onboarding guide for new members.\n\n## Setup\n\nFollow these steps to get started.\n",
    )
    .expect("write guide.md");
}

// ===== Scenario 1: Shared config full flow =====

#[test]
fn e2e_team_config_full_flow() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: commandindex.toml with custom search.default_limit
    write_commandindex_toml(
        dir.path(),
        "[search]\ndefault_limit = 5\nsnippet_lines = 1\n",
    );
    setup_test_markdown(dir.path());

    // Act: index then config show
    common::run_index(dir.path());

    let output = common::cmd()
        .args(["config", "show"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Assert: config show reflects the team config values
    assert!(
        stdout.contains("default_limit = 5"),
        "config show should reflect team config default_limit=5, got: {stdout}"
    );
    assert!(
        stdout.contains("snippet_lines = 1"),
        "config show should reflect team config snippet_lines=1, got: {stdout}"
    );
}

// ===== Scenario 2: Config priority (local > team > default) =====

#[test]
fn e2e_config_priority() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: team config with default_limit=5, local config overrides to 3
    write_commandindex_toml(
        dir.path(),
        "[search]\ndefault_limit = 5\nsnippet_lines = 4\n",
    );
    setup_test_markdown(dir.path());
    common::run_index(dir.path());

    // Create local config that overrides default_limit
    write_config_local_toml(dir.path(), "[search]\ndefault_limit = 3\n");

    // Act: config show
    let output = common::cmd()
        .args(["config", "show"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Assert: local config overrides team config
    assert!(
        stdout.contains("default_limit = 3"),
        "local config should override team config: default_limit should be 3, got: {stdout}"
    );
    // snippet_lines should still come from team config (not overridden by local)
    assert!(
        stdout.contains("snippet_lines = 4"),
        "team config snippet_lines=4 should be preserved when not overridden by local, got: {stdout}"
    );
}

// ===== Scenario 3: Config show API key masking =====

#[test]
fn e2e_config_show_api_key_masked() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: local config with api_key
    setup_test_markdown(dir.path());
    common::run_index(dir.path());
    write_config_local_toml(
        dir.path(),
        "[embedding]\napi_key = \"sk-test-secret-key-12345\"\n",
    );

    // Act: config show
    let output = common::cmd()
        .args(["config", "show"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Assert: API key is masked, not shown in plaintext
    assert!(
        !stdout.contains("sk-test-secret-key-12345"),
        "API key should NOT appear in plaintext in config show output"
    );
    assert!(
        stdout.contains("***"),
        "Masked API key (***) should appear in config show output, got: {stdout}"
    );
}

// ===== Scenario 4: Export/Import with search verification =====

#[test]
fn e2e_export_import_search_flow() {
    let source_dir = tempfile::tempdir().expect("create source dir");

    // Setup: create content with team config
    write_commandindex_toml(source_dir.path(), "[search]\ndefault_limit = 10\n");
    setup_test_markdown(source_dir.path());

    // Index and verify search
    common::run_index(source_dir.path());
    let pre_results = common::run_search_jsonl(source_dir.path(), "team onboarding");
    assert!(
        !pre_results.is_empty(),
        "search should find results before export"
    );

    // Export via CLI
    let archive_path = source_dir.path().join("team-index.tar.gz");
    common::cmd()
        .args(["export", archive_path.to_str().unwrap()])
        .current_dir(source_dir.path())
        .assert()
        .success();

    // Clean the index
    common::run_clean(source_dir.path());

    // Import into the same directory
    common::cmd()
        .args(["import", archive_path.to_str().unwrap(), "--force"])
        .current_dir(source_dir.path())
        .assert()
        .success();

    // Verify search still works after import
    let post_results = common::run_search_jsonl(source_dir.path(), "team onboarding");
    assert!(
        !post_results.is_empty(),
        "search should find results after import"
    );

    // Verify status is healthy
    let status = common::run_status_json(source_dir.path());
    assert!(status.get("version").is_some());
    assert!(status.get("total_files").is_some());
}

// ===== Scenario 5: status --verify with team config =====

#[test]
fn e2e_status_verify_with_team_config() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: team config + index
    write_commandindex_toml(dir.path(), "[search]\ndefault_limit = 10\n");
    setup_test_markdown(dir.path());
    common::run_index(dir.path());

    // Act: status --verify
    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap(), "--verify"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verify: OK"));
}

// ===== Scenario 6: status --detail =====

#[test]
fn e2e_status_detail() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: index with test files
    setup_test_markdown(dir.path());
    common::run_index(dir.path());

    // Act: status --detail
    let output = common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap(), "--detail"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Assert: detail sections are present
    assert!(
        stdout.contains("CommandIndex Status"),
        "should contain basic status header"
    );
    assert!(
        stdout.contains("Coverage"),
        "should contain Coverage section with --detail"
    );
    assert!(
        stdout.contains("Discoverable files:"),
        "should show discoverable files count"
    );
    assert!(
        stdout.contains("Storage"),
        "should contain Storage section with --detail"
    );
    assert!(
        stdout.contains("Tantivy index:"),
        "should show tantivy index size"
    );
    assert!(stdout.contains("Total:"), "should show total storage size");
}

// ===== Scenario 7: status --format json --detail =====

#[test]
fn e2e_status_json_detail() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Setup: index with test files
    setup_test_markdown(dir.path());
    common::run_index(dir.path());

    // Act: status --format json --detail
    let output = common::cmd()
        .args([
            "status",
            "--path",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
            "--detail",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");

    // Assert: basic fields
    assert!(parsed.get("version").is_some(), "should have version");
    assert!(
        parsed.get("total_files").is_some(),
        "should have total_files"
    );
    assert!(
        parsed.get("total_sections").is_some(),
        "should have total_sections"
    );
    assert!(
        parsed.get("index_size_bytes").is_some(),
        "should have index_size_bytes"
    );

    // Assert: detail fields (coverage and storage)
    let coverage = parsed
        .get("coverage")
        .expect("should have coverage with --detail");
    assert!(
        coverage.get("discoverable_files").is_some(),
        "coverage should have discoverable_files"
    );
    assert!(
        coverage.get("indexed_files").is_some(),
        "coverage should have indexed_files"
    );

    let storage = parsed
        .get("storage")
        .expect("should have storage with --detail");
    assert!(
        storage.get("tantivy_bytes").is_some(),
        "storage should have tantivy_bytes"
    );
    assert!(
        storage.get("total_bytes").is_some(),
        "storage should have total_bytes"
    );
}

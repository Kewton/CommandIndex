mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create a repository directory with a Markdown file and build the index.
fn setup_repo(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(format!("{name}.md")), content).unwrap();
    common::cmd()
        .args(["index", "--path", dir.to_str().unwrap()])
        .assert()
        .success();
}

/// Write a workspace TOML config file.
fn create_workspace_toml(path: &std::path::Path, repos: &[(&str, &str)]) {
    let mut content = String::from("[workspace]\nname = \"test-workspace\"\n\n");
    for (alias, repo_path) in repos {
        content.push_str(&format!(
            "[[workspace.repositories]]\npath = \"{repo_path}\"\nalias = \"{alias}\"\n\n"
        ));
    }
    fs::write(path, content).unwrap();
}

/// Helper: set up 3 repos with distinct content and return (workspace_dir, repo_dirs, ws_toml_path).
fn setup_three_repos() -> (TempDir, Vec<TempDir>, std::path::PathBuf) {
    let ws_dir = tempfile::tempdir().expect("create workspace dir");

    let repo_a = tempfile::tempdir().expect("create repo_a dir");
    setup_repo(
        repo_a.path(),
        "rust-guide",
        "# Rust Guide\n\nRustプロジェクトのセットアップガイドです。\n",
    );

    let repo_b = tempfile::tempdir().expect("create repo_b dir");
    setup_repo(
        repo_b.path(),
        "api-reference",
        "# API Reference\n\nThis document describes the REST API endpoints.\n",
    );

    let repo_c = tempfile::tempdir().expect("create repo_c dir");
    setup_repo(
        repo_c.path(),
        "deployment",
        "# Deployment Guide\n\nHow to deploy the application to production.\n",
    );

    let ws_toml = ws_dir.path().join("workspace.toml");
    create_workspace_toml(
        &ws_toml,
        &[
            ("repo-a", repo_a.path().to_str().unwrap()),
            ("repo-b", repo_b.path().to_str().unwrap()),
            ("repo-c", repo_c.path().to_str().unwrap()),
        ],
    );

    (ws_dir, vec![repo_a, repo_b, repo_c], ws_toml)
}

/// Parse JSONL output into a Vec of serde_json::Value.
fn parse_jsonl(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("each line should be valid JSON"))
        .collect()
}

// ============================================================================
// 横断検索テスト
// ============================================================================

#[test]
fn test_workspace_search_cross_repo() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    // Search for "Guide" which appears in repo-a (Rust Guide) and repo-c (Deployment Guide)
    let output = common::cmd()
        .args([
            "search",
            "Guide",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);

    assert!(
        !results.is_empty(),
        "cross-repo search should return results"
    );

    // Collect repository names from results
    let repos_found: Vec<String> = results
        .iter()
        .filter_map(|r| {
            r.get("repository")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();

    assert!(
        repos_found.contains(&"repo-a".to_string()),
        "results should include repo-a, got: {:?}",
        repos_found
    );
    assert!(
        repos_found.contains(&"repo-c".to_string()),
        "results should include repo-c, got: {:?}",
        repos_found
    );
}

#[test]
fn test_workspace_search_with_repo_filter() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    // Search with --repo repo-b to filter to only repo-b
    let output = common::cmd()
        .args([
            "search",
            "API",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--repo",
            "repo-b",
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);

    assert!(
        !results.is_empty(),
        "filtered search should return results from repo-b"
    );

    // All results should be from repo-b
    for result in &results {
        let repo = result
            .get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        assert_eq!(
            repo, "repo-b",
            "all results should be from repo-b, got: {}",
            repo
        );
    }
}

#[test]
fn test_workspace_search_json_format() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    let output = common::cmd()
        .args([
            "search",
            "Guide",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(!results.is_empty(), "should have results");

    // Verify all expected fields including "repository"
    let first = &results[0];
    assert!(
        first.get("repository").is_some(),
        "JSON output should include 'repository' field"
    );
    assert!(
        first.get("path").is_some(),
        "JSON output should include 'path' field"
    );
    assert!(
        first.get("heading").is_some(),
        "JSON output should include 'heading' field"
    );
    assert!(
        first.get("body").is_some(),
        "JSON output should include 'body' field"
    );
    assert!(
        first.get("score").is_some(),
        "JSON output should include 'score' field"
    );
}

#[test]
fn test_workspace_search_path_format() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    let output = common::cmd()
        .args([
            "search",
            "Guide",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "path",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(!lines.is_empty(), "path format should output lines");

    // Path format for workspace uses "[repo-alias] path" format
    for line in &lines {
        assert!(
            line.starts_with('['),
            "workspace path format should start with '[repo-alias]', got: {}",
            line
        );
        assert!(
            line.contains(']'),
            "workspace path format should contain ']', got: {}",
            line
        );
    }
}

// ============================================================================
// エラーハンドリングテスト
// ============================================================================

#[test]
fn test_workspace_search_nonexistent_config() {
    common::cmd()
        .args([
            "search",
            "test",
            "--workspace",
            "/nonexistent/path/workspace.toml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read workspace config"));
}

#[test]
fn test_workspace_search_repo_filter_not_found() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    common::cmd()
        .args([
            "search",
            "test",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--repo",
            "nonexistent-repo",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found in workspace"));
}

#[test]
fn test_workspace_search_partial_repo_failure() {
    let ws_dir = tempfile::tempdir().expect("create workspace dir");

    // repo-a: properly indexed
    let repo_a = tempfile::tempdir().expect("create repo_a dir");
    setup_repo(
        repo_a.path(),
        "guide",
        "# Guide\n\nThis is a guide document.\n",
    );

    // repo-b: exists but has no index (just an empty directory)
    let repo_b = tempfile::tempdir().expect("create repo_b dir");
    // Do NOT index repo_b

    let ws_toml = ws_dir.path().join("workspace.toml");
    create_workspace_toml(
        &ws_toml,
        &[
            ("repo-a", repo_a.path().to_str().unwrap()),
            ("repo-b", repo_b.path().to_str().unwrap()),
        ],
    );

    // Search should succeed with results from repo-a, even though repo-b has no index
    let output = common::cmd()
        .args([
            "search",
            "Guide",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "should return results from the working repo even when another repo fails"
    );

    // Results should only come from repo-a
    for result in &results {
        let repo = result
            .get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        assert_eq!(
            repo, "repo-a",
            "results should only come from repo-a (the indexed repo)"
        );
    }
}

// ============================================================================
// statusテスト
// ============================================================================

#[test]
fn test_workspace_status_human() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    let output = common::cmd()
        .args([
            "status",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "human",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Should display workspace name and repository list
    assert!(
        stdout.contains("test-workspace"),
        "status should show workspace name"
    );
    assert!(stdout.contains("repo-a"), "status should list repo-a");
    assert!(stdout.contains("repo-b"), "status should list repo-b");
    assert!(stdout.contains("repo-c"), "status should list repo-c");
}

#[test]
fn test_workspace_status_json() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    let output = common::cmd()
        .args([
            "status",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let status: serde_json::Value =
        serde_json::from_str(&stdout).expect("status JSON should be valid");

    // Verify structure
    assert_eq!(
        status["workspace"], "test-workspace",
        "JSON should contain workspace name"
    );

    let repositories = status["repositories"]
        .as_array()
        .expect("repositories should be an array");
    assert_eq!(repositories.len(), 3, "should have 3 repositories");

    // Each repo should have alias, path, and status fields
    for repo in repositories {
        assert!(repo.get("alias").is_some(), "repo should have alias");
        assert!(repo.get("path").is_some(), "repo should have path");
        assert!(repo.get("status").is_some(), "repo should have status");
    }

    // All repos should be "ok" since they were indexed
    let aliases: Vec<&str> = repositories
        .iter()
        .filter_map(|r| r.get("alias").and_then(|v| v.as_str()))
        .collect();
    assert!(aliases.contains(&"repo-a"));
    assert!(aliases.contains(&"repo-b"));
    assert!(aliases.contains(&"repo-c"));
}

// ============================================================================
// updateテスト
// ============================================================================

#[test]
fn test_workspace_update() {
    let (_ws_dir, _repos, ws_toml) = setup_three_repos();

    // Run update --workspace
    common::cmd()
        .args(["update", "--workspace", ws_toml.to_str().unwrap()])
        .assert()
        .success();

    // After update, search should still work
    let output = common::cmd()
        .args([
            "search",
            "Guide",
            "--workspace",
            ws_toml.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "search should work after workspace update"
    );
}

// ============================================================================
// 後方互換テスト
// ============================================================================

#[test]
fn test_search_without_workspace_unchanged() {
    // Set up a single repo and search without --workspace flag
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(
        dir.path().join("notes.md"),
        "# Notes\n\nSome important notes about the project.\n",
    )
    .unwrap();

    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Search without --workspace should work as before
    let output = common::cmd()
        .args(["search", "Notes", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "search without --workspace should return results"
    );

    // Results should NOT have a "repository" field (standard search)
    let first = &results[0];
    assert!(
        first.get("repository").is_none(),
        "standard search should not include 'repository' field"
    );
    assert!(first.get("path").is_some(), "should have 'path' field");
    assert!(
        first.get("heading").is_some(),
        "should have 'heading' field"
    );
}

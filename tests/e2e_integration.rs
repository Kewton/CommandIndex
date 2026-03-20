mod common;

use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create the shared E2E test directory structure.
///
/// ```text
/// ├── guide.md           (Japanese, tags: [rust, tutorial])
/// ├── api.md             (English, tags: [api, http])
/// ├── no_frontmatter.md  (no frontmatter)
/// ├── docs/
/// │   └── nested.md
/// ├── ignored/
/// │   └── secret.md
/// └── .cmindexignore     (contains: ignored/)
/// ```
fn setup_e2e_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    // guide.md — Japanese content with frontmatter
    fs::write(
        dir.path().join("guide.md"),
        "\
---
tags:
  - rust
  - tutorial
---
# ガイド

Rustプロジェクトのセットアップガイドです。

## セットアップ

環境構築の手順を説明します。
",
    )
    .unwrap();

    // api.md — English content with frontmatter
    fs::write(
        dir.path().join("api.md"),
        "\
---
tags:
  - api
  - http
---
# API Reference

This document describes the API.

## Authentication

Use Bearer tokens for authentication.

## Endpoints

GET /api/v1/search returns search results.
",
    )
    .unwrap();

    // no_frontmatter.md — no YAML frontmatter
    fs::write(
        dir.path().join("no_frontmatter.md"),
        "\
# Notes

Some plain notes without frontmatter.
",
    )
    .unwrap();

    // docs/nested.md
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join("docs/nested.md"),
        "\
# Nested Doc

Documentation inside a subdirectory.
",
    )
    .unwrap();

    // ignored/secret.md
    fs::create_dir_all(dir.path().join("ignored")).unwrap();
    fs::write(
        dir.path().join("ignored/secret.md"),
        "\
# Secret

This file should be ignored by the indexer.
",
    )
    .unwrap();

    // .cmindexignore
    fs::write(dir.path().join(".cmindexignore"), "ignored/\n").unwrap();

    dir
}

/// Run the index command on the given temp directory.
fn run_index(dir: &TempDir) {
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

/// Parse JSONL output (one JSON object per line) into a Vec of serde_json::Value.
fn parse_jsonl(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("each line should be valid JSON"))
        .collect()
}

#[test]
fn e2e_full_flow() {
    let dir = setup_e2e_dir();

    // 1. Index
    run_index(&dir);

    // 2. Search "API" with JSON format (uses current_dir)
    let search_output = common::cmd()
        .args(["search", "API", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&search_output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "search should return results for 'API'"
    );

    // Verify result structure
    let first = &results[0];
    assert!(first.get("path").is_some());
    assert!(first.get("heading").is_some());
    assert!(first.get("body").is_some());
    assert!(first.get("tags").is_some());
    assert!(first.get("score").is_some());

    // 3. Status with JSON format
    let status_output = common::cmd()
        .args([
            "status",
            "--path",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let status_stdout = String::from_utf8_lossy(&status_output.get_output().stdout);
    let status: serde_json::Value =
        serde_json::from_str(&status_stdout).expect("valid status JSON");
    assert_eq!(status["total_files"], 4);
    assert_eq!(status["total_sections"], 7);

    // 4. Clean
    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(
        !dir.path().join(".commandindex").exists(),
        ".commandindex/ should be removed after clean"
    );
}

#[test]
fn e2e_japanese_search() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    let output = common::cmd()
        .args(["search", "ガイド", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("guide.md"),
        "Japanese search should find guide.md"
    );
}

#[test]
fn e2e_filter_combination() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    // --tag rust → should find guide.md content
    let output = common::cmd()
        .args([
            "search",
            "セットアップ",
            "--tag",
            "rust",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("guide.md"),
        "--tag rust should find guide.md"
    );

    // --path docs → should find nested.md content
    let output = common::cmd()
        .args(["search", "Doc", "--path", "docs", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("nested.md"),
        "--path docs should find nested.md"
    );

    // --type markdown → should return results
    let output = common::cmd()
        .args(["search", "API", "--type", "markdown", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(!results.is_empty(), "--type markdown should return results");

    // --heading Authentication → should find api.md content
    let output = common::cmd()
        .args([
            "search",
            "Bearer",
            "--heading",
            "Authentication",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("api.md"),
        "--heading Authentication should find api.md"
    );
}

#[test]
fn e2e_output_formats() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    // --format human: stdout contains file path and heading-like content
    common::cmd()
        .args(["search", "API", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("api.md"))
        .stdout(predicate::str::contains("API"));

    // --format json: stdout is valid JSONL with expected keys
    let output = common::cmd()
        .args(["search", "API", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = parse_jsonl(&stdout);
    assert!(!results.is_empty());
    let first = &results[0];
    assert!(first.get("path").is_some());
    assert!(first.get("heading").is_some());
    assert!(first.get("body").is_some());
    assert!(first.get("tags").is_some());
    assert!(first.get("score").is_some());

    // --format path: stdout has one path per line
    let output = common::cmd()
        .args(["search", "API", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(
        !lines.is_empty(),
        "path format should output at least one line"
    );
    for line in &lines {
        assert!(
            line.ends_with(".md"),
            "each line in path format should be a file path"
        );
    }
}

#[test]
fn e2e_search_without_index() {
    let dir = tempfile::tempdir().expect("create temp dir");

    common::cmd()
        .args(["search", "anything"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn e2e_ignore_excludes_from_search() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    // Search for "Secret" which only exists in ignored/secret.md
    let output = common::cmd()
        .args(["search", "Secret", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);

    // Either no results (stderr says "No results found.") or results don't contain secret.md
    if stdout.trim().is_empty() {
        assert!(
            stderr.contains("No results found."),
            "empty stdout should mean no results"
        );
    } else {
        assert!(
            !stdout.contains("secret.md"),
            "ignored/secret.md should not appear in search results"
        );
    }
}

#[test]
fn e2e_status_after_index() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    let output = common::cmd()
        .args([
            "status",
            "--path",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let status: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(status["total_files"], 4, "should have 4 indexed files");
    assert_eq!(status["total_sections"], 7, "should have 7 total sections");
}

#[test]
fn e2e_search_no_results() {
    let dir = setup_e2e_dir();
    run_index(&dir);

    common::cmd()
        .args(["search", "xyznonexistentquery12345"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No results found."));
}

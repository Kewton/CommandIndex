mod common;

use predicates::prelude::*;

/// Create a temp directory with markdown files that link to each other.
fn setup_linked_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();

    // a.md links to b.md and c.md
    std::fs::write(
        docs.join("a.md"),
        "---\ntags: auth security\n---\n# Page A\nSee [Page B](b.md) and [Page C](c.md)\n",
    )
    .unwrap();

    // b.md links to a.md
    std::fs::write(
        docs.join("b.md"),
        "---\ntags: auth\n---\n# Page B\nBack to [Page A](a.md)\n",
    )
    .unwrap();

    // c.md has no links
    std::fs::write(
        docs.join("c.md"),
        "---\ntags: security\n---\n# Page C\nStandalone page\n",
    )
    .unwrap();

    // d.md is unrelated
    std::fs::write(
        docs.join("d.md"),
        "---\ntags: unrelated\n---\n# Page D\nNo connections\n",
    )
    .unwrap();

    common::run_index(dir.path());
    dir
}

#[test]
fn related_search_finds_linked_files() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "should find related files for docs/a.md"
    );

    // b.md and c.md should appear (linked from a.md)
    let paths: Vec<&str> = results.iter().filter_map(|r| r["path"].as_str()).collect();
    assert!(
        paths.iter().any(|p| p.contains("b.md")),
        "b.md should be related to a.md, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.contains("c.md")),
        "c.md should be related to a.md, got: {paths:?}"
    );
}

#[test]
fn related_search_json_format_has_score_and_relations() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(!results.is_empty());

    for result in &results {
        assert!(
            result.get("score").is_some(),
            "each result should have a score"
        );
        assert!(
            result.get("relations").is_some(),
            "each result should have relations"
        );
        assert!(
            result.get("path").is_some(),
            "each result should have a path"
        );
    }
}

#[test]
fn related_search_human_format() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("score:"), "human format should show score");
}

#[test]
fn related_search_path_format() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(!lines.is_empty(), "path format should output file paths");
    for line in &lines {
        assert!(
            !line.contains("score"),
            "path format should only show paths"
        );
    }
}

#[test]
fn related_search_no_results_for_unlinked_file() {
    let dir = setup_linked_docs();
    common::cmd()
        .args(["search", "--related", "docs/d.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No related files found"));
}

#[test]
fn related_search_nonexistent_file() {
    let dir = setup_linked_docs();
    // A file that doesn't exist in the index should still succeed (just no results)
    common::cmd()
        .args(["search", "--related", "nonexistent.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No related files found"));
}

#[test]
fn related_search_conflicts_with_query() {
    let dir = setup_linked_docs();
    common::cmd()
        .args([
            "search",
            "some query",
            "--related",
            "docs/a.md",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn related_search_conflicts_with_symbol() {
    let dir = setup_linked_docs();
    common::cmd()
        .args([
            "search",
            "--symbol",
            "foo",
            "--related",
            "docs/a.md",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn related_search_respects_limit() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args([
            "search",
            "--related",
            "docs/a.md",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(results.len() <= 1, "should respect limit=1");
}

#[test]
fn related_search_no_index() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["search", "--related", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

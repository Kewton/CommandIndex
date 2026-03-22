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
fn diff_json_format_correct() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["diff", "docs/a.md", "docs/b.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("should be valid JSON object");

    // Verify structure
    assert!(json.get("file_a").is_some(), "should have file_a");
    assert!(json.get("file_b").is_some(), "should have file_b");
    assert!(json.get("only_a").is_some(), "should have only_a");
    assert!(json.get("only_b").is_some(), "should have only_b");
    assert!(json.get("overlap").is_some(), "should have overlap");
    assert!(
        json.get("overlap_count").is_some(),
        "should have overlap_count"
    );

    // Verify types
    assert!(json["only_a"].is_array(), "only_a should be array");
    assert!(json["only_b"].is_array(), "only_b should be array");
    assert!(json["overlap"].is_array(), "overlap should be array");
    assert!(
        json["overlap_count"].is_number(),
        "overlap_count should be number"
    );

    // overlap_count should match overlap array length
    let overlap_count = json["overlap_count"].as_u64().unwrap();
    let overlap_len = json["overlap"].as_array().unwrap().len() as u64;
    assert_eq!(
        overlap_count, overlap_len,
        "overlap_count should match overlap length"
    );
}

#[test]
fn diff_human_format_correct() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["diff", "docs/a.md", "docs/b.md", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    assert!(
        stdout.contains("=== Diff:"),
        "should have header, got: {stdout}"
    );
    assert!(
        stdout.contains("Only in"),
        "should have Only in section, got: {stdout}"
    );
    assert!(
        stdout.contains("Overlap"),
        "should have Overlap section, got: {stdout}"
    );
}

#[test]
fn diff_path_format_outputs_overlap_only() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["diff", "docs/a.md", "docs/b.md", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Path format should only output overlap paths (or be empty if no overlap)
    // It should NOT contain diff headers or section labels
    assert!(
        !stdout.contains("=== Diff:"),
        "path format should not contain headers"
    );
    assert!(
        !stdout.contains("Only in"),
        "path format should not contain section labels"
    );
    assert!(
        !stdout.contains("Overlap"),
        "path format should not contain section labels"
    );
}

#[test]
fn diff_same_file_error() {
    let dir = setup_linked_docs();
    common::cmd()
        .args(["diff", "docs/a.md", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot diff a file with itself"));
}

#[test]
fn diff_no_index_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["diff", "docs/a.md", "docs/b.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

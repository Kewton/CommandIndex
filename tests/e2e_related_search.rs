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

/// Run `search --related <file> --format json` and return parsed JSONL results.
fn run_related_search(dir: &std::path::Path, file: &str) -> Vec<serde_json::Value> {
    run_related_search_with_args(dir, file, &[])
}

/// Run `search --related <file> --format json` with extra args and return parsed JSONL results.
fn run_related_search_with_args(
    dir: &std::path::Path,
    file: &str,
    extra_args: &[&str],
) -> Vec<serde_json::Value> {
    let mut args = vec!["search", "--related", file, "--format", "json"];
    args.extend_from_slice(extra_args);
    let output = common::cmd()
        .args(&args)
        .current_dir(dir)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    common::parse_jsonl(&stdout)
}

/// Extract all paths from JSONL results.
fn result_paths(results: &[serde_json::Value]) -> Vec<&str> {
    results.iter().filter_map(|r| r["path"].as_str()).collect()
}

/// Find a result whose path contains the given substring.
fn find_result_by_path<'a>(
    results: &'a [serde_json::Value],
    path_substr: &str,
) -> Option<&'a serde_json::Value> {
    results
        .iter()
        .find(|r| r["path"].as_str().is_some_and(|p| p.contains(path_substr)))
}

/// Check if any result has a specific string relation.
fn has_relation(results: &[serde_json::Value], relation: &str) -> bool {
    results.iter().any(|r| {
        r["relations"]
            .as_array()
            .is_some_and(|rels| rels.iter().any(|rel| rel.as_str() == Some(relation)))
    })
}

#[test]
fn related_search_finds_linked_files() {
    let dir = setup_linked_docs();
    let results = run_related_search(dir.path(), "docs/a.md");
    assert!(
        !results.is_empty(),
        "should find related files for docs/a.md"
    );

    // b.md and c.md should appear (linked from a.md)
    let paths = result_paths(&results);
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
    let results = run_related_search(dir.path(), "docs/a.md");
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
    let results = run_related_search_with_args(dir.path(), "docs/a.md", &["--limit", "1"]);
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

/// Create a temp directory with TypeScript files that have import chains.
fn setup_import_chain() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();

    // main.ts imports from './helper'
    std::fs::write(
        src.join("main.ts"),
        "import { helperFunc } from './helper';\n\nexport function main() {\n  helperFunc();\n}\n",
    )
    .unwrap();

    // helper.ts imports from './utils'
    std::fs::write(
        src.join("helper.ts"),
        "import { utilFunc } from './utils';\n\nexport function helperFunc() {\n  utilFunc();\n}\n",
    )
    .unwrap();

    // utils.ts: no imports
    std::fs::write(
        src.join("utils.ts"),
        "export function utilFunc() {\n  return 42;\n}\n",
    )
    .unwrap();

    common::run_index(dir.path());
    dir
}

#[test]
fn related_full_flow_verifies_relation_types() {
    let dir = setup_linked_docs();
    let results = run_related_search(dir.path(), "docs/a.md");
    assert!(!results.is_empty(), "should find related files");

    // b.md and c.md should both have markdown_link relation (linked from a.md)
    for filename in &["b.md", "c.md"] {
        let result = find_result_by_path(&results, filename);
        assert!(result.is_some(), "{filename} should be in results");
        let relations = result.unwrap()["relations"]
            .as_array()
            .expect("relations array");
        assert!(
            relations
                .iter()
                .any(|r| r.as_str() == Some("markdown_link")),
            "{filename} should have markdown_link relation, got: {relations:?}"
        );
    }

    // All results should have score > 0
    for result in &results {
        let score = result["score"].as_f64().expect("score should be f64");
        assert!(score > 0.0, "score should be positive, got: {score}");
    }
}

#[test]
fn related_tag_match_detects_shared_tags() {
    let dir = setup_linked_docs();
    let results = run_related_search(dir.path(), "docs/a.md");
    assert!(!results.is_empty(), "should find related files");

    // a.md has tags "auth security". Files sharing these tags should appear
    // in the results with a tag_match relation.
    // Note: tag matching returns paths as indexed (e.g. "docs/b.md"),
    // which may differ from link-resolved paths (e.g. "b.md").
    let has_any_tag_match = results.iter().any(|r| {
        let relations = r["relations"].as_array();
        relations.is_some_and(|rels| {
            rels.iter()
                .any(|rel| rel.is_object() && rel.get("tag_match").is_some())
        })
    });
    assert!(
        has_any_tag_match,
        "at least one result should have tag_match relation (shared tags), got: {results:?}"
    );

    // Verify that the tag_match contains a relevant tag (auth or security)
    let tag_match_result = results
        .iter()
        .find(|r| {
            r["relations"].as_array().is_some_and(|rels| {
                rels.iter()
                    .any(|rel| rel.is_object() && rel.get("tag_match").is_some())
            })
        })
        .unwrap();
    let relations = tag_match_result["relations"].as_array().unwrap();
    let tag_match = relations
        .iter()
        .find(|r| r.is_object() && r.get("tag_match").is_some())
        .unwrap();
    let matched_tags = tag_match["tag_match"].as_array().expect("tag_match array");
    assert!(
        matched_tags
            .iter()
            .any(|t| t.as_str() == Some("auth") || t.as_str() == Some("security")),
        "tag_match should contain 'auth' or 'security', got: {matched_tags:?}"
    );
}

#[test]
fn related_directory_proximity_boosts_score() {
    let dir = setup_linked_docs();
    let results = run_related_search(dir.path(), "docs/a.md");
    assert!(!results.is_empty(), "should find related files");

    // Directory proximity applies to files in the same directory.
    assert!(
        has_relation(&results, "directory_proximity"),
        "at least one result should have directory_proximity relation, got: {results:?}"
    );

    // Files with directory_proximity should have a boosted score
    let dir_prox_result = results
        .iter()
        .find(|r| {
            r["relations"].as_array().is_some_and(|rels| {
                rels.iter()
                    .any(|rel| rel.as_str() == Some("directory_proximity"))
            })
        })
        .unwrap();
    let score = dir_prox_result["score"].as_f64().expect("score");
    assert!(
        score > 0.2,
        "directory_proximity result score should be > 0.2 (boosted), got: {score}"
    );
}

#[test]
fn related_import_dependency_detects_ts_imports() {
    let dir = setup_import_chain();
    let results = run_related_search(dir.path(), "src/main.ts");
    assert!(
        !results.is_empty(),
        "should find related files for src/main.ts"
    );

    // helper.ts should be related via import_dependency
    let paths = result_paths(&results);
    let helper_result = find_result_by_path(&results, "helper");
    assert!(
        helper_result.is_some(),
        "helper.ts should be in results, got paths: {paths:?}"
    );
    let helper_relations = helper_result.unwrap()["relations"]
        .as_array()
        .expect("relations array");
    assert!(
        helper_relations
            .iter()
            .any(|r| r.as_str() == Some("import_dependency")),
        "helper.ts should have import_dependency relation, got: {helper_relations:?}"
    );
}

#[test]
fn related_conflicts_with_tag() {
    let dir = setup_linked_docs();
    common::cmd()
        .args([
            "search",
            "--related",
            "docs/a.md",
            "--tag",
            "auth",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .failure();
}

// --- Multiple files tests ---

/// Run `search --related <files...> --format json` with multiple files and return parsed JSONL.
fn run_related_search_multi(
    dir: &std::path::Path,
    files: &[&str],
    extra_args: &[&str],
) -> Vec<serde_json::Value> {
    let mut args = vec!["search", "--related"];
    args.extend_from_slice(files);
    args.push("--format");
    args.push("json");
    args.extend_from_slice(extra_args);
    let output = common::cmd()
        .args(&args)
        .current_dir(dir)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    common::parse_jsonl(&stdout)
}

#[test]
fn related_search_multiple_files_merged() {
    let dir = setup_linked_docs();
    let results = run_related_search_multi(dir.path(), &["docs/a.md", "docs/b.md"], &[]);
    assert!(
        !results.is_empty(),
        "should find related files for multiple input files"
    );

    let paths = result_paths(&results);
    assert!(
        paths.iter().any(|p| p.contains("c.md")),
        "c.md should be in merged results, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"docs/a.md"),
        "docs/a.md (target) should be excluded from results, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"docs/b.md"),
        "docs/b.md (target) should be excluded from results, got: {paths:?}"
    );
}

#[test]
fn related_search_single_file_backward_compat() {
    let dir = setup_linked_docs();
    let results = run_related_search_multi(dir.path(), &["docs/a.md"], &[]);
    assert!(!results.is_empty(), "single file search should still work");
    let paths = result_paths(&results);
    assert!(
        paths.iter().any(|p| p.contains("b.md")),
        "b.md should be related to a.md, got: {paths:?}"
    );
}

#[test]
fn related_search_partial_missing_graceful() {
    let dir = setup_linked_docs();
    let results = run_related_search_multi(dir.path(), &["docs/a.md", "nonexistent.md"], &[]);
    assert!(
        !results.is_empty(),
        "should return results even when one file is missing"
    );
    let paths = result_paths(&results);
    assert!(
        paths.iter().any(|p| p.contains("b.md")),
        "b.md should be in results from a.md, got: {paths:?}"
    );
}

#[test]
fn related_search_all_missing_multiple() {
    let dir = setup_linked_docs();
    common::cmd()
        .args([
            "search",
            "--related",
            "nonexistent1.md",
            "nonexistent2.md",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No related files found"));
}

#[test]
fn related_search_multiple_files_json_format() {
    let dir = setup_linked_docs();
    let results = run_related_search_multi(dir.path(), &["docs/a.md", "docs/b.md"], &[]);
    for result in &results {
        assert!(result.get("score").is_some(), "should have score");
        assert!(result.get("relations").is_some(), "should have relations");
        assert!(result.get("path").is_some(), "should have path");
    }
}

#[test]
fn related_search_multiple_files_path_format() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args([
            "search",
            "--related",
            "docs/a.md",
            "docs/b.md",
            "--format",
            "path",
        ])
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

// --- --related-stdin tests ---

#[test]
fn related_stdin_finds_related_files() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related-stdin", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "should find related files via --related-stdin"
    );
    let paths = result_paths(&results);
    assert!(
        paths.iter().any(|p| p.contains("b.md")),
        "b.md should be related to a.md via stdin, got: {paths:?}"
    );
}

#[test]
fn related_stdin_multiple_files_merges() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args(["search", "--related-stdin", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\ndocs/b.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(
        !results.is_empty(),
        "should find related files from multiple stdin inputs"
    );
}

#[test]
fn related_stdin_no_index_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n").unwrap();
    common::cmd()
        .args(["search", "--related-stdin", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("test.md\n")
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains("not found"));
}

#[test]
fn related_stdin_respects_limit() {
    let dir = setup_linked_docs();
    let output = common::cmd()
        .args([
            "search",
            "--related-stdin",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);
    assert!(results.len() <= 1, "should respect limit=1");
}

// --- Issue #123: --with-snippet bug fix tests ---

#[test]
fn related_import_with_snippet_returns_nonempty_snippet() {
    let dir = setup_import_chain();
    let results = run_related_search_with_args(dir.path(), "src/main.ts", &["--with-snippet"]);
    assert!(
        !results.is_empty(),
        "should find related files for src/main.ts"
    );

    // helper.ts should be in results with a non-empty snippet
    let helper = find_result_by_path(&results, "helper");
    assert!(
        helper.is_some(),
        "helper.ts should be in results, got: {:?}",
        result_paths(&results)
    );
    let helper = helper.unwrap();
    let snippet = helper.get("snippet").and_then(|s| s.as_str());
    assert!(
        snippet.is_some_and(|s| !s.is_empty()),
        "snippet should be non-empty for helper.ts (issue #123), got: {snippet:?}"
    );
}

#[test]
fn related_import_file_path_is_real_indexed_path() {
    let dir = setup_import_chain();
    let results = run_related_search(dir.path(), "src/main.ts");
    assert!(
        !results.is_empty(),
        "should find related files for src/main.ts"
    );

    // All result paths should be real file paths (not import paths like './helper')
    for result in &results {
        let path = result["path"].as_str().unwrap();
        assert!(
            !path.starts_with("./") && !path.starts_with("@/") && !path.starts_with("~/"),
            "result path should be a real file path, not an import path: {path}"
        );
        // Path should end with a known extension
        assert!(
            path.ends_with(".ts")
                || path.ends_with(".tsx")
                || path.ends_with(".js")
                || path.ends_with(".jsx")
                || path.ends_with(".md")
                || path.ends_with(".py"),
            "result path should have a file extension: {path}"
        );
    }

    // Specifically check helper.ts is returned as "src/helper.ts"
    let helper = find_result_by_path(&results, "helper");
    assert!(helper.is_some(), "helper.ts should be in results");
    let helper_path = helper.unwrap()["path"].as_str().unwrap();
    assert_eq!(
        helper_path, "src/helper.ts",
        "helper should be returned as real file path"
    );
}

#[test]
fn related_reverse_import_resolves_correctly() {
    let dir = setup_import_chain();
    // helper.ts is imported by main.ts, so searching from helper should find main
    let results = run_related_search(dir.path(), "src/helper.ts");
    assert!(
        !results.is_empty(),
        "should find related files for src/helper.ts"
    );

    let paths = result_paths(&results);
    assert!(
        paths.iter().any(|p| p.contains("main.ts")),
        "main.ts should be related to helper.ts (reverse import), got: {paths:?}"
    );
    // Also utils.ts (forward import from helper)
    assert!(
        paths.iter().any(|p| p.contains("utils.ts")),
        "utils.ts should be related to helper.ts (forward import), got: {paths:?}"
    );
}

// --- Issue #123 UAT fix: markdown link paths should resolve to indexed paths ---

#[test]
fn related_markdown_link_with_snippet_returns_nonempty() {
    let dir = setup_linked_docs();
    let results = run_related_search_with_args(dir.path(), "docs/a.md", &["--with-snippet"]);
    assert!(
        !results.is_empty(),
        "should find related files for docs/a.md"
    );

    // b.md should be in results with a real indexed path (not bare "b.md")
    let b_result = results
        .iter()
        .find(|r| r["path"].as_str().is_some_and(|p| p.contains("b.md")));
    assert!(
        b_result.is_some(),
        "b.md should be in results, got: {:?}",
        result_paths(&results)
    );
    let b = b_result.unwrap();
    let path = b["path"].as_str().unwrap();
    assert_eq!(
        path, "docs/b.md",
        "markdown link target should resolve to indexed path"
    );
    let snippet = b.get("snippet").and_then(|s| s.as_str());
    assert!(
        snippet.is_some_and(|s| !s.is_empty()),
        "snippet should be non-empty for markdown-linked file docs/b.md, got: {snippet:?}"
    );
}

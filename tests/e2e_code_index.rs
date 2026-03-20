mod common;

use std::fs;
use tempfile::TempDir;

/// Create a test directory with TypeScript, Python, and Markdown files.
///
/// ```text
/// ├── guide.md           (Markdown with tags)
/// ├── src/
/// │   ├── main.ts        (TypeScript: function greet, class UserService)
/// │   ├── utils.py       (Python: function calculate_sum, class DataProcessor)
/// │   └── empty.ts       (empty file — should be skipped)
/// └── .cmindexignore     (excludes nothing)
/// ```
fn setup_code_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Markdown file
    fs::write(
        dir.path().join("guide.md"),
        "\
---
tags:
  - guide
---
# Guide

This is a guide document.
",
    )
    .unwrap();

    // TypeScript file
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/main.ts"),
        "\
export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export class UserService {
    getUser(id: number) {
        return { id, name: 'test' };
    }
}
",
    )
    .unwrap();

    // Python file
    fs::write(
        dir.path().join("src/utils.py"),
        "\
def calculate_sum(a, b):
    return a + b

class DataProcessor:
    def process(self, data):
        return data
",
    )
    .unwrap();

    // Empty TypeScript file (should be skipped)
    fs::write(dir.path().join("src/empty.ts"), "").unwrap();

    dir
}

#[test]
fn e2e_code_index_creates_symbols_db() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    assert!(
        dir.path().join(".commandindex/symbols.db").exists(),
        "symbols.db should be created after indexing"
    );
}

#[test]
fn e2e_code_index_manifest_contains_code_files() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    let manifest_path = dir.path().join(".commandindex/manifest.json");
    let content = fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

    let files = manifest["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();

    assert!(
        paths.contains(&"guide.md"),
        "manifest should contain guide.md"
    );
    assert!(
        paths.contains(&"src/main.ts"),
        "manifest should contain src/main.ts"
    );
    assert!(
        paths.contains(&"src/utils.py"),
        "manifest should contain src/utils.py"
    );

    // Verify file_type fields
    for file in files {
        let path = file["path"].as_str().unwrap();
        let ft = file["file_type"].as_str().unwrap();
        match path {
            "guide.md" => assert_eq!(ft, "markdown"),
            "src/main.ts" => assert_eq!(ft, "type_script"),
            "src/utils.py" => assert_eq!(ft, "python"),
            _ => {} // empty.ts may or may not be present (it's empty, so skipped by code indexer but 0 sections)
        }
    }
}

#[test]
fn e2e_code_search_typescript() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Search for TypeScript content
    let results = common::run_search_jsonl(dir.path(), "greet");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("main.ts")));
    assert!(
        found,
        "search for 'greet' should find main.ts, got: {results:?}"
    );
}

#[test]
fn e2e_code_search_python() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Search for Python content
    let results = common::run_search_jsonl(dir.path(), "calculate_sum");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("utils.py")));
    assert!(
        found,
        "search for 'calculate_sum' should find utils.py, got: {results:?}"
    );
}

#[test]
fn e2e_code_search_markdown_still_works() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Search for Markdown content
    let results = common::run_search_jsonl(dir.path(), "guide");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("guide.md")));
    assert!(
        found,
        "search for 'guide' should find guide.md, got: {results:?}"
    );
}

#[test]
fn e2e_code_update_add_code_file() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Add a new Python file
    fs::write(
        dir.path().join("src/new_module.py"),
        "\
def new_function_xyz():
    pass
",
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update
    common::run_update(dir.path()).success();

    // Verify new content is searchable
    let results = common::run_search_jsonl(dir.path(), "new_function_xyz");
    let found = results.iter().any(|r| {
        r["path"]
            .as_str()
            .is_some_and(|p| p.contains("new_module.py"))
    });
    assert!(
        found,
        "search for 'new_function_xyz' should find new_module.py after update, got: {results:?}"
    );
}

#[test]
fn e2e_code_update_modify_code_file() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Modify existing TypeScript file
    fs::write(
        dir.path().join("src/main.ts"),
        "\
export function modified_unique_function(): void {
    console.log('modified');
}
",
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update
    common::run_update(dir.path()).success();

    // Verify new content is searchable
    let results = common::run_search_jsonl(dir.path(), "modified_unique_function");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("main.ts")));
    assert!(
        found,
        "search for 'modified_unique_function' should find main.ts after update, got: {results:?}"
    );

    // Verify old content (greet) from main.ts is no longer found
    let results = common::run_search_jsonl(dir.path(), "greet");
    let old_found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("main.ts")));
    assert!(
        !old_found,
        "search for 'greet' should NOT find main.ts after modification"
    );
}

#[test]
fn e2e_code_update_delete_code_file() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Delete the Python file
    fs::remove_file(dir.path().join("src/utils.py")).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run update
    common::run_update(dir.path()).success();

    // Verify deleted content is not found
    let results = common::run_search_jsonl(dir.path(), "calculate_sum");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("utils.py")));
    assert!(
        !found,
        "search for 'calculate_sum' should NOT find utils.py after deletion"
    );
}

#[test]
fn e2e_code_clean_and_reindex() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Clean
    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(
        !dir.path().join(".commandindex").exists(),
        ".commandindex should be removed after clean"
    );

    // Reindex
    common::run_index(dir.path());

    // Verify code files are still searchable after reindex
    let results = common::run_search_jsonl(dir.path(), "greet");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.contains("main.ts")));
    assert!(
        found,
        "search for 'greet' should find main.ts after clean + reindex"
    );
}

#[test]
fn e2e_code_empty_file_skipped() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // Empty file should not produce any search results
    let results = common::run_search_jsonl(dir.path(), "empty");
    let found = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p == "src/empty.ts"));
    assert!(!found, "empty.ts should not appear in search results");
}

#[test]
fn e2e_code_status_shows_file_types() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    let status = common::run_status_json(dir.path());

    // Verify file_type_counts
    let counts = &status["file_type_counts"];
    assert_eq!(counts["markdown"], 1, "should have 1 markdown file");
    assert_eq!(
        counts["typescript"], 1,
        "should have 1 typescript file (empty.ts produces 0 sections, not in manifest)"
    );
    assert_eq!(counts["python"], 1, "should have 1 python file");

    // Verify symbol_count > 0
    let symbol_count = status["symbol_count"].as_u64().unwrap();
    assert!(
        symbol_count > 0,
        "symbol_count should be > 0, got: {symbol_count}"
    );
}

#[test]
fn e2e_code_search_type_filter() {
    let dir = setup_code_dir();
    common::run_index(dir.path());

    // --type typescript should only return .ts files
    let output = common::cmd()
        .args([
            "search",
            "greet",
            "--type",
            "typescript",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    for r in &results {
        let path = r["path"].as_str().unwrap();
        assert!(
            path.ends_with(".ts") || path.ends_with(".tsx"),
            "all results should be TypeScript files, got: {path}"
        );
    }

    // --type code should return .ts and .py files
    let output = common::cmd()
        .args(["search", "function", "--type", "code", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let results = common::parse_jsonl(&stdout);

    for r in &results {
        let path = r["path"].as_str().unwrap();
        assert!(
            !path.ends_with(".md"),
            "--type code results should not contain .md files, got: {path}"
        );
    }
}

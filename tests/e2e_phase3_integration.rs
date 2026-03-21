mod common;

use std::fs;
use std::path::Path;

use predicates::prelude::*;

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

// -- Shared test file content (single source of truth) --

const TS_GREET: &str = r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }
}

import { Logger } from './logger';
"#;

const TSX_APP: &str = r#"import React from 'react';

export function AppComponent(): JSX.Element {
    return <div>Hello</div>;
}

export const ArrowComponent = () => {
    return <span>World</span>;
};
"#;

const PY_UTILS: &str = r#"class DataProcessor:
    def process(self, data):
        return data

def helper_function():
    pass
"#;

const MD_GUIDE: &str = "# ガイド\n\nこれは日本語のドキュメントです。\n";

// -- Setup helpers --

/// Create only TypeScript test files.
fn setup_phase3_ts_only(dir: &Path) {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/greet.ts"), TS_GREET).unwrap();
}

/// Create only Python test files.
fn setup_phase3_py_only(dir: &Path) {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/utils.py"), PY_UTILS).unwrap();
}

/// Create all test files: TypeScript, TSX, Python, and Markdown.
fn setup_phase3_full(dir: &Path) {
    setup_phase3_ts_only(dir);
    setup_phase3_py_only(dir);
    fs::write(dir.join("src/App.tsx"), TSX_APP).unwrap();
    fs::write(dir.join("guide.md"), MD_GUIDE).unwrap();
}

// ---------------------------------------------------------------------------
// Test 1: Full flow — index, symbol search, status, clean
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_full_flow_ts_symbol_status_clean() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_ts_only(dir.path());

    // Index
    common::run_index(dir.path());

    // --symbol search should find greet
    let results = common::run_symbol_search_jsonl(dir.path(), "greet");
    assert!(!results.is_empty(), "should find greet symbol");

    // status JSON should have symbol_count > 0
    let status = common::run_status_json(dir.path());
    let symbol_count = status["symbol_count"].as_u64().unwrap();
    assert!(
        symbol_count > 0,
        "symbol_count should be > 0, got: {symbol_count}"
    );

    // clean should remove .commandindex
    common::run_clean(dir.path());
    assert!(
        !dir.path().join(".commandindex").exists(),
        ".commandindex should be removed after clean"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Python symbol search
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_python_symbol_search() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_py_only(dir.path());

    common::run_index(dir.path());

    // Search for DataProcessor class
    let results = common::run_symbol_search_jsonl(dir.path(), "DataProcessor");
    assert!(
        results.iter().any(|r| r["name"] == "DataProcessor"),
        "should find DataProcessor class, got: {results:?}"
    );

    // Search for helper_function
    let results = common::run_symbol_search_jsonl(dir.path(), "helper_function");
    assert!(
        results.iter().any(|r| r["name"] == "helper_function"),
        "should find helper_function, got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Mixed content with type filter
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_mixed_content_type_filter() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_full(dir.path());

    common::run_index(dir.path());

    // --type typescript should only return TS/TSX files
    let results = common::run_typed_search_jsonl(dir.path(), "function", "typescript");
    for r in &results {
        let path = r["path"].as_str().unwrap();
        assert!(
            path.ends_with(".ts") || path.ends_with(".tsx"),
            "--type typescript should only return TS files, got: {path}"
        );
    }

    // --type markdown should only return MD files
    let results = common::run_typed_search_jsonl(dir.path(), "ガイド", "markdown");
    for r in &results {
        let path = r["path"].as_str().unwrap();
        assert!(
            path.ends_with(".md"),
            "--type markdown should only return MD files, got: {path}"
        );
    }

    // Full-text search for Japanese content in MD
    let results = common::run_search_jsonl(dir.path(), "日本語");
    let found_md = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.ends_with(".md")));
    assert!(
        found_md,
        "full-text search for '日本語' should find guide.md"
    );

    // Full-text search for English function name in code
    let results = common::run_search_jsonl(dir.path(), "greet");
    let found_ts = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.ends_with(".ts")));
    assert!(
        found_ts,
        "full-text search for 'greet' should find .ts file"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Update reflects new symbols
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_update_symbol_reflect() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_ts_only(dir.path());

    common::run_index(dir.path());

    // Record initial symbol_count
    let status_before = common::run_status_json(dir.path());
    let count_before = status_before["symbol_count"].as_u64().unwrap();

    // Add a new TypeScript file with additional symbols
    fs::write(
        dir.path().join("src/extra.ts"),
        r#"export function extraHelper(): void {}

export class ExtraService {
    run(): void {}
}
"#,
    )
    .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));

    common::run_update(dir.path()).success();

    // symbol_count should increase
    let status_after = common::run_status_json(dir.path());
    let count_after = status_after["symbol_count"].as_u64().unwrap();
    assert!(
        count_after > count_before,
        "symbol_count should increase after update: before={count_before}, after={count_after}"
    );

    // New symbol should be searchable
    let results = common::run_symbol_search_jsonl(dir.path(), "extraHelper");
    assert!(!results.is_empty(), "should find extraHelper after update");
}

// ---------------------------------------------------------------------------
// Test 5: --type python filter in mixed project
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_type_python_filter() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_full(dir.path());

    common::run_index(dir.path());

    // --type python should only return .py files
    let results = common::run_typed_search_jsonl(dir.path(), "class", "python");
    assert!(
        !results.is_empty(),
        "--type python search for 'class' should return results"
    );
    for r in &results {
        let path = r["path"].as_str().unwrap();
        assert!(
            path.ends_with(".py"),
            "--type python should only return .py files, got: {path}"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 6: Import dependency whitebox test via SymbolStore public API
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_import_dependency_whitebox() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_ts_only(dir.path());

    common::run_index(dir.path());

    // Open SymbolStore directly via the public API
    let db_path = commandindex::indexer::symbol_db_path(dir.path());
    let store =
        commandindex::indexer::symbol_store::SymbolStore::open(&db_path).expect("open symbol db");

    // Insert a dependency manually via the public API to verify the
    // insert_dependencies / find_imports_by_target round-trip works
    // when used against the same DB that the CLI created.
    use commandindex::indexer::symbol_store::ImportInfo;
    let dep = ImportInfo {
        id: None,
        source_file: "src/greet.ts".to_string(),
        target_module: "./logger".to_string(),
        imported_names: Some("Logger".to_string()),
        file_hash: "test_hash".to_string(),
    };
    store
        .insert_dependencies(&[dep])
        .expect("insert dependency");

    // Query back via find_imports_by_target
    let imports = store
        .find_imports_by_target("./logger")
        .expect("query imports");

    assert!(
        !imports.is_empty(),
        "should find import dependency for './logger'"
    );
    assert!(
        imports
            .iter()
            .any(|imp| imp.source_file.contains("greet.ts")),
        "import source should be greet.ts, got: {imports:?}"
    );

    // Verify imported_names contains Logger
    let logger_import = imports
        .iter()
        .find(|imp| imp.source_file.contains("greet.ts"))
        .unwrap();
    assert!(
        logger_import
            .imported_names
            .as_ref()
            .is_some_and(|names| names.contains("Logger")),
        "imported_names should contain 'Logger', got: {:?}",
        logger_import.imported_names
    );

    // Also verify that the symbol store has symbols from the CLI indexing
    let symbols = store.find_by_name("greet").expect("query symbols");
    assert!(
        !symbols.is_empty(),
        "should find greet symbol in the store created by CLI"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Symbol search without index should show error
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_symbol_search_without_index() {
    let dir = tempfile::tempdir().unwrap();
    // Do NOT run index — empty directory

    common::run_symbol_search(dir.path(), "anything")
        .failure()
        .stderr(predicate::str::contains("Symbol database not found"));
}

// ---------------------------------------------------------------------------
// Test 8: TSX file E2E
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_tsx_file_e2e() {
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_full(dir.path());

    common::run_index(dir.path());

    // --type typescript should include .tsx files
    let results = common::run_typed_search_jsonl(dir.path(), "AppComponent", "typescript");
    let found_tsx = results
        .iter()
        .any(|r| r["path"].as_str().is_some_and(|p| p.ends_with(".tsx")));
    assert!(found_tsx, "--type typescript should find .tsx file");

    // --symbol search should find AppComponent (function declaration in TSX)
    let results = common::run_symbol_search_jsonl(dir.path(), "AppComponent");
    assert!(
        results.iter().any(|r| r["name"] == "AppComponent"),
        "should find AppComponent via --symbol, got: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 9: --symbol with --type simultaneous
// ---------------------------------------------------------------------------

#[test]
fn e2e_phase3_symbol_type_simultaneous() {
    // NOTE: --symbol search operates on symbols.db (SQLite), independent of
    // tantivy full-text search. The --type flag currently does NOT filter
    // --symbol results. Therefore --symbol X --type typescript should return
    // the same results as --symbol X alone.
    let dir = tempfile::tempdir().unwrap();
    setup_phase3_ts_only(dir.path());

    common::run_index(dir.path());

    // --symbol greet --type typescript
    let output_with_type = common::cmd()
        .args([
            "search",
            "--symbol",
            "greet",
            "--type",
            "typescript",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout_with = String::from_utf8_lossy(&output_with_type.get_output().stdout);
    let results_with = common::parse_jsonl(&stdout_with);

    // --symbol greet (no --type)
    let results_without = common::run_symbol_search_jsonl(dir.path(), "greet");

    assert_eq!(
        results_with.len(),
        results_without.len(),
        "--symbol with --type should return same count as --symbol alone: with={}, without={}",
        results_with.len(),
        results_without.len()
    );
}

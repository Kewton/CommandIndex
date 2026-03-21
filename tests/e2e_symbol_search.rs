mod common;

use predicates::prelude::*;

fn setup_ts_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        dir.path().join("app.ts"),
        r#"
export class UserService {
    getUser(id: number): string {
        return "user";
    }
    updateUser(id: number, name: string): void {
        // update
    }
}

export function handleAuth(token: string): boolean {
    return true;
}

function helperFunc(): void {
    // private helper
}
"#,
    )
    .unwrap();
    common::run_index(dir.path());
    dir
}

#[test]
fn symbol_search_finds_function_by_name() {
    let dir = setup_ts_project();
    let results = common::run_symbol_search_jsonl(dir.path(), "handleAuth");
    assert!(!results.is_empty(), "should find handleAuth");
    assert_eq!(results[0]["name"], "handleAuth");
    assert_eq!(results[0]["kind"], "function");
}

#[test]
fn symbol_search_finds_class_by_name() {
    let dir = setup_ts_project();
    let results = common::run_symbol_search_jsonl(dir.path(), "UserService");
    assert!(!results.is_empty(), "should find UserService");
    assert_eq!(results[0]["name"], "UserService");
    assert_eq!(results[0]["kind"], "class");
}

#[test]
fn symbol_search_partial_match() {
    let dir = setup_ts_project();
    let results = common::run_symbol_search_jsonl(dir.path(), "handle");
    assert!(
        results.iter().any(|r| r["name"] == "handleAuth"),
        "partial match should find handleAuth"
    );
}

#[test]
fn symbol_search_case_insensitive() {
    let dir = setup_ts_project();
    let results = common::run_symbol_search_jsonl(dir.path(), "userservice");
    assert!(
        results.iter().any(|r| r["name"] == "UserService"),
        "case-insensitive match should find UserService"
    );
}

#[test]
fn symbol_search_not_found() {
    let dir = setup_ts_project();
    common::run_symbol_search(dir.path(), "nonexistent_symbol")
        .success()
        .stderr(predicate::str::contains("No symbols found"));
}

#[test]
fn symbol_search_human_format() {
    let dir = setup_ts_project();
    let output = common::cmd()
        .args(["search", "--symbol", "handleAuth", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("function"), "human output should show kind");
    assert!(
        stdout.contains("handleAuth"),
        "human output should show name"
    );
}

#[test]
fn symbol_search_path_format() {
    let dir = setup_ts_project();
    let output = common::cmd()
        .args(["search", "--symbol", "handleAuth", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(
        stdout.contains("app.ts:"),
        "path output should contain file:line"
    );
}

#[test]
fn symbol_search_class_shows_children() {
    let dir = setup_ts_project();
    let results = common::run_symbol_search_jsonl(dir.path(), "UserService");
    // The class itself plus its methods should appear
    let method_results: Vec<_> = results.iter().filter(|r| r["kind"] == "method").collect();
    assert!(
        !method_results.is_empty(),
        "class search should include child methods"
    );
}

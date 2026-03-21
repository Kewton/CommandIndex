mod common;

/// Create a temp directory with markdown and code files for context pack testing.
fn setup_context_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();

    // a.md: tags rust, search; links to b.md
    std::fs::write(
        docs.join("a.md"),
        "---\ntags: rust search\n---\n# Page A\nSee [Page B](b.md)\nSome content about searching.\n",
    )
    .unwrap();

    // b.md: tags rust; links to ../src/c.ts
    std::fs::write(
        docs.join("b.md"),
        "---\ntags: rust\n---\n# Page B\nSee [Code C](../src/c.ts)\nDocumentation for module B.\n",
    )
    .unwrap();

    // c.ts: imports from './d'
    std::fs::write(
        src.join("c.ts"),
        "import { func } from './d';\n\nexport function main() {\n  func();\n}\n",
    )
    .unwrap();

    // d.ts: exports func
    std::fs::write(
        src.join("d.ts"),
        "export function func() {\n  return 42;\n}\n",
    )
    .unwrap();

    common::run_index(dir.path());
    dir
}

/// Parse a single JSON object from stdout (not JSONL).
fn parse_context_pack(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout).expect("should be valid JSON")
}

#[test]
fn context_pack_outputs_valid_json() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);
    assert!(pack.is_object(), "output should be a JSON object");
    assert!(
        pack.get("target_files").is_some(),
        "should have target_files"
    );
    assert!(pack.get("context").is_some(), "should have context");
    assert!(pack.get("summary").is_some(), "should have summary");
}

#[test]
fn context_pack_includes_target_files() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let targets = pack["target_files"].as_array().expect("target_files array");
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].as_str().unwrap(), "docs/a.md");
}

#[test]
fn context_pack_includes_related_context() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let context = pack["context"].as_array().expect("context array");
    assert!(!context.is_empty(), "context should not be empty");

    // b.md should be in context (linked from a.md)
    let paths: Vec<&str> = context.iter().filter_map(|e| e["path"].as_str()).collect();
    assert!(
        paths.iter().any(|p| p.contains("b.md")),
        "b.md should be in context, got: {paths:?}"
    );
}

#[test]
fn context_pack_max_files_limits_output() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md", "--max-files", "1"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let context = pack["context"].as_array().expect("context array");
    assert!(
        context.len() <= 1,
        "context should have at most 1 entry with --max-files 1, got {}",
        context.len()
    );
}

#[test]
fn context_pack_multiple_files() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md", "docs/b.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let targets = pack["target_files"].as_array().expect("target_files array");
    assert_eq!(targets.len(), 2);

    let context = pack["context"].as_array().expect("context array");
    // Should have merged results from both files
    assert!(
        !context.is_empty(),
        "context should not be empty for multiple files"
    );
}

#[test]
fn context_pack_no_self_reference() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let context = pack["context"].as_array().expect("context array");
    let paths: Vec<&str> = context.iter().filter_map(|e| e["path"].as_str()).collect();
    assert!(
        !paths.contains(&"docs/a.md"),
        "target file docs/a.md should not appear in context, got: {paths:?}"
    );
}

#[test]
fn context_pack_summary_fields() {
    let dir = setup_context_docs();
    let output = common::cmd()
        .args(["context", "docs/a.md"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let pack = parse_context_pack(&stdout);

    let summary = pack.get("summary").expect("should have summary");
    assert!(
        summary.get("total_related").is_some(),
        "summary should have total_related"
    );
    assert!(
        summary.get("included").is_some(),
        "summary should have included"
    );
    assert!(
        summary.get("estimated_tokens").is_some(),
        "summary should have estimated_tokens"
    );

    let total = summary["total_related"].as_u64().unwrap();
    let included = summary["included"].as_u64().unwrap();
    assert!(
        included <= total,
        "included ({included}) should be <= total_related ({total})"
    );
}

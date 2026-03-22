mod common;

use predicates::prelude::*;

/// Create a temp directory with linked markdown files and build index.
fn setup_impact_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();

    // a.md links to b.md
    std::fs::write(
        docs.join("a.md"),
        "---\ntags: auth security\n---\n# Page A\nSee [Page B](b.md)\n",
    )
    .unwrap();

    // b.md links to c.md
    std::fs::write(
        docs.join("b.md"),
        "---\ntags: auth\n---\n# Page B\nSee [Page C](c.md)\n",
    )
    .unwrap();

    // c.md standalone
    std::fs::write(
        docs.join("c.md"),
        "---\ntags: security\n---\n# Page C\nStandalone page\n",
    )
    .unwrap();

    common::run_index(dir.path());
    dir
}

#[test]
fn impact_stdin_json_output() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["total_input_files"], 1);
    assert!(parsed["impacted_files"].as_array().is_some());
    assert!(
        parsed["input_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("docs/a.md"))
    );
}

#[test]
fn impact_args_json_output() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["total_input_files"], 1);
    assert!(parsed["impacted_files"].as_array().is_some());
}

#[test]
fn impact_human_output() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "human"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Impact analysis"));
    assert!(stdout.contains("input file"));
}

#[test]
fn impact_path_output() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "path"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    // Should output at least one path (b.md or c.md)
    assert!(!lines.is_empty(), "path format should output file paths");
    for line in &lines {
        assert!(
            !line.contains("score"),
            "path format should only show paths"
        );
    }
}

#[test]
fn impact_stdin_multiple_files() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\ndocs/b.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["total_input_files"], 2);
    // impacted_files should have impacted_by array
    if let Some(files) = parsed["impacted_files"].as_array() {
        for f in files {
            assert!(
                f["impacted_by"].is_array(),
                "each file should have impacted_by"
            );
        }
    }
}

#[test]
fn impact_stdin_dedup() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("docs/a.md\ndocs/a.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // Deduplication: only 1 input file
    assert_eq!(parsed["total_input_files"], 1);
}

#[test]
fn impact_stdin_with_dot_slash_prefix() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("./docs/a.md\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // Normalized to docs/a.md
    assert!(
        parsed["input_files"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("docs/a.md"))
    );
}

#[test]
fn impact_no_index_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n").unwrap();
    common::cmd()
        .args(["impact", "test.md"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn impact_nonexistent_file_warning() {
    let dir = setup_impact_docs();
    common::cmd()
        .args(["impact", "nonexistent.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no valid file paths"));
}

#[test]
fn impact_limit_option() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "json", "--limit", "1"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed["impacted_files"].as_array().unwrap().len() <= 1);
}

#[test]
fn impact_stdin_invalid_path_skipped() {
    let dir = setup_impact_docs();
    // ../etc/passwd should be skipped with a warning
    let output = common::cmd()
        .args(["impact", "--format", "json"])
        .current_dir(dir.path())
        .write_stdin("../etc/passwd\ndocs/a.md\n")
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(stderr.contains("Warning"));
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["total_input_files"], 1);
}

#[test]
fn impact_excludes_input_files_from_results() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let impacted_paths: Vec<&str> = parsed["impacted_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert!(
        !impacted_paths.contains(&"docs/a.md"),
        "input file should be excluded from impacted files"
    );
}

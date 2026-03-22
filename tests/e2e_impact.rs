mod common;

use predicates::prelude::*;

/// Create a temp directory with linked markdown files and build index.
fn setup_impact_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();

    // a.md links to b.md and c.md
    std::fs::write(
        docs.join("a.md"),
        "---\ntags: auth security\n---\n# Page A\nSee [Page B](b.md) and [Page C](c.md)\n",
    )
    .unwrap();

    // b.md links to a.md and c.md
    std::fs::write(
        docs.join("b.md"),
        "---\ntags: auth\n---\n# Page B\nSee [Page A](a.md) and [Page C](c.md)\n",
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
    assert_eq!(parsed["summary"]["changed"], 1);
    assert!(parsed["impact"].as_array().is_some());
    assert!(
        parsed["changed_files"]
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
    assert_eq!(parsed["summary"]["changed"], 1);
    assert!(parsed["impact"].as_array().is_some());
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
    assert!(stdout.contains("changed file"));
    assert!(stdout.contains("Summary:"));
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
    assert_eq!(parsed["summary"]["changed"], 2);
    // impact array should have per-file entries
    let impact = parsed["impact"].as_array().unwrap();
    assert_eq!(impact.len(), 2);
    for entry in impact {
        assert!(entry["file"].is_string(), "each entry should have 'file'");
        assert!(
            entry["related"].is_array(),
            "each entry should have 'related'"
        );
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
    assert_eq!(parsed["summary"]["changed"], 1);
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
        parsed["changed_files"]
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
    // --limit applies per-file to related[]
    let impact = parsed["impact"].as_array().unwrap();
    for entry in impact {
        assert!(
            entry["related"].as_array().unwrap().len() <= 1,
            "per-file related should be limited to 1"
        );
    }
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
    assert_eq!(parsed["summary"]["changed"], 1);
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
    // Check that input file is not in any related list
    let impact = parsed["impact"].as_array().unwrap();
    for entry in impact {
        let related = entry["related"].as_array().unwrap();
        let related_paths: Vec<&str> = related.iter().filter_map(|r| r["path"].as_str()).collect();
        assert!(
            !related_paths.contains(&"docs/a.md"),
            "input file should be excluded from related files"
        );
    }
}

#[test]
fn impact_overlap_detection() {
    let dir = setup_impact_docs();
    // a.md and b.md both link to c.md, so c.md should be in overlap
    let output = common::cmd()
        .args(["impact", "docs/a.md", "docs/b.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let overlap = parsed["overlap"].as_array().unwrap();
    // c.md should appear in overlap (both a.md and b.md relate to it)
    let overlap_paths: Vec<&str> = overlap.iter().filter_map(|v| v.as_str()).collect();
    let has_c_md = overlap_paths.iter().any(|p| p.ends_with("c.md"));
    assert!(
        has_c_md,
        "c.md should be in overlap, got: {:?}",
        overlap_paths
    );
    assert!(
        parsed["summary"]["overlap_count"].as_u64().unwrap() >= 1,
        "overlap_count should be at least 1"
    );
}

#[test]
fn impact_summary_statistics() {
    let dir = setup_impact_docs();
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // Summary should have all required fields
    assert!(parsed["summary"]["changed"].is_number());
    assert!(parsed["summary"]["total_impacted"].is_number());
    assert!(parsed["summary"]["overlap_count"].is_number());
    assert_eq!(parsed["summary"]["changed"], 1);
    // total_impacted should be positive (a.md links to b.md and c.md)
    assert!(
        parsed["summary"]["total_impacted"].as_u64().unwrap() > 0,
        "total_impacted should be positive"
    );
}

#[test]
fn impact_limit_per_file_summary_uses_full_count() {
    let dir = setup_impact_docs();
    // With --limit 1, related[] is truncated but summary.total_impacted uses full count
    let output = common::cmd()
        .args(["impact", "docs/a.md", "--format", "json", "--limit", "1"])
        .current_dir(dir.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let impact = parsed["impact"].as_array().unwrap();
    let related_count = impact[0]["related"].as_array().unwrap().len();
    let total_impacted = parsed["summary"]["total_impacted"].as_u64().unwrap();
    // related is truncated to 1, but total_impacted counts all
    assert!(related_count <= 1, "related should be limited to 1");
    assert!(
        total_impacted >= related_count as u64,
        "total_impacted ({total_impacted}) should be >= related shown ({related_count})"
    );
}

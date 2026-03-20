mod common;

use std::io::Cursor;
use std::path::PathBuf;

use commandindex::cli::status::{StatusFormat, compute_dir_size, format_size, run};

// ===== format_size tests =====

#[test]
fn format_size_bytes() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1023), "1023 B");
}

#[test]
fn format_size_kilobytes() {
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(1024 * 1023), "1023.0 KB");
}

#[test]
fn format_size_megabytes() {
    assert_eq!(format_size(1024 * 1024), "1.0 MB");
    assert_eq!(format_size(1024 * 1024 * 500), "500.0 MB");
}

#[test]
fn format_size_gigabytes() {
    assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(format_size(1024 * 1024 * 1024 * 2), "2.0 GB");
}

// ===== compute_dir_size tests =====

#[test]
fn compute_dir_size_empty_dir() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let size = compute_dir_size(dir.path());
    assert_eq!(size, 0);
}

#[test]
fn compute_dir_size_with_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("a.txt"), "hello").expect("write file");
    std::fs::write(dir.path().join("b.txt"), "world!").expect("write file");
    let size = compute_dir_size(dir.path());
    assert_eq!(size, 11); // 5 + 6
}

#[test]
fn compute_dir_size_nested() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).expect("create subdir");
    std::fs::write(sub.join("file.txt"), "abc").expect("write file");
    let size = compute_dir_size(dir.path());
    assert_eq!(size, 3);
}

// ===== run() error cases =====

#[test]
fn run_directory_not_found() {
    let mut buf = Cursor::new(Vec::new());
    let path = PathBuf::from("/nonexistent/path/that/does/not/exist");
    let result = run(&path, StatusFormat::Human, &mut buf);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Directory not found"));
}

#[test]
fn run_not_initialized() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mut buf = Cursor::new(Vec::new());
    let result = run(dir.path(), StatusFormat::Human, &mut buf);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not initialized"));
}

// ===== run() success cases =====

fn setup_commandindex_dir(base: &std::path::Path) {
    let ci_dir = base.join(".commandindex");
    std::fs::create_dir_all(&ci_dir).expect("create .commandindex");

    let state = commandindex::indexer::state::IndexState::new(base.to_path_buf());
    state.save(&ci_dir).expect("save state");
}

#[test]
fn run_human_format() {
    let dir = tempfile::tempdir().expect("create temp dir");
    setup_commandindex_dir(dir.path());

    let mut buf = Cursor::new(Vec::new());
    run(dir.path(), StatusFormat::Human, &mut buf).expect("run should succeed");

    let output = String::from_utf8(buf.into_inner()).expect("valid utf8");
    assert!(output.contains("CommandIndex Status"));
    assert!(output.contains("Index root:"));
    assert!(output.contains("Version:"));
    assert!(output.contains("Created:"));
    assert!(output.contains("Last updated:"));
    assert!(output.contains("Total files:"));
    assert!(output.contains("Total sections:"));
    assert!(output.contains("Index size:"));
}

#[test]
fn run_json_format() {
    let dir = tempfile::tempdir().expect("create temp dir");
    setup_commandindex_dir(dir.path());

    let mut buf = Cursor::new(Vec::new());
    run(dir.path(), StatusFormat::Json, &mut buf).expect("run should succeed");

    let output = String::from_utf8(buf.into_inner()).expect("valid utf8");
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid json");
    assert!(parsed.get("version").is_some());
    assert!(parsed.get("total_files").is_some());
    assert!(parsed.get("total_sections").is_some());
    assert!(parsed.get("index_size_bytes").is_some());
    assert!(parsed.get("index_root").is_some());
    assert!(parsed.get("created_at").is_some());
    assert!(parsed.get("last_updated_at").is_some());
}

// ===== E2E tests via CLI binary =====

#[test]
fn status_cli_not_initialized() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains(
            "not initialized",
        ));
}

#[test]
fn status_cli_human_format() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // First, run index to create .commandindex
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    common::cmd()
        .args(["status", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicates::prelude::predicate::str::contains(
            "CommandIndex Status",
        ));
}

#[test]
fn status_cli_json_format() {
    let dir = tempfile::tempdir().expect("create temp dir");
    // First, run index to create .commandindex
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    common::cmd()
        .args([
            "status",
            "--path",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::prelude::predicate::str::contains("\"version\""));
}

#[test]
fn status_cli_directory_not_found() {
    common::cmd()
        .args(["status", "--path", "/nonexistent/dir"])
        .assert()
        .failure()
        .stderr(predicates::prelude::predicate::str::contains(
            "Directory not found",
        ));
}

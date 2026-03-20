#![allow(dead_code)]

use assert_cmd::Command;

/// Create a Command for the commandindex binary.
pub fn cmd() -> Command {
    Command::cargo_bin("commandindex").expect("binary should exist")
}

/// Runs `commandindex index --path <path>` and asserts success.
pub fn run_index(path: impl AsRef<std::path::Path>) {
    cmd()
        .args(["index", "--path", path.as_ref().to_str().unwrap()])
        .assert()
        .success();
}

/// Runs `commandindex update --path <path>` and returns Assert for further checks.
pub fn run_update(path: impl AsRef<std::path::Path>) -> assert_cmd::assert::Assert {
    cmd()
        .args(["update", "--path", path.as_ref().to_str().unwrap()])
        .assert()
}

/// Runs `commandindex search --format json <query>` with current_dir set to path.
pub fn run_search(path: impl AsRef<std::path::Path>, query: &str) -> assert_cmd::assert::Assert {
    cmd()
        .args(["search", query, "--format", "json"])
        .current_dir(path.as_ref())
        .assert()
}

/// Runs `commandindex status --path <path> --format json` and returns parsed JSON.
pub fn run_status_json(path: impl AsRef<std::path::Path>) -> serde_json::Value {
    let output = cmd()
        .args([
            "status",
            "--path",
            path.as_ref().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    serde_json::from_str(&stdout).expect("valid status JSON")
}

/// Parses JSONL output (one JSON object per line) into a Vec.
pub fn parse_jsonl(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("each line should be valid JSON"))
        .collect()
}

/// Composite: run search and parse JSONL results.
pub fn run_search_jsonl(path: impl AsRef<std::path::Path>, query: &str) -> Vec<serde_json::Value> {
    let output = run_search(path, query).success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    parse_jsonl(&stdout)
}

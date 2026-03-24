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

/// JSONL出力からメタデータ行を除外して検索結果のみ返す
pub fn parse_search_jsonl(output: &str) -> Vec<serde_json::Value> {
    parse_jsonl(output)
        .into_iter()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("metadata"))
        .collect()
}

/// JSONL出力からメタデータ行を取得
pub fn parse_jsonl_metadata(output: &str) -> Option<serde_json::Value> {
    parse_jsonl(output)
        .into_iter()
        .find(|v| v.get("type").and_then(|t| t.as_str()) == Some("metadata"))
}

/// Composite: run search and parse JSONL results.
pub fn run_search_jsonl(path: impl AsRef<std::path::Path>, query: &str) -> Vec<serde_json::Value> {
    let output = run_search(path, query).success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    parse_jsonl(&stdout)
}

/// Runs `commandindex search --symbol <name> --format json` with current_dir set to path.
pub fn run_symbol_search(
    path: impl AsRef<std::path::Path>,
    name: &str,
) -> assert_cmd::assert::Assert {
    cmd()
        .args(["search", "--symbol", name, "--format", "json"])
        .current_dir(path.as_ref())
        .assert()
}

/// Composite: run symbol search and parse JSONL results.
pub fn run_symbol_search_jsonl(
    path: impl AsRef<std::path::Path>,
    name: &str,
) -> Vec<serde_json::Value> {
    let output = run_symbol_search(path, name).success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    parse_jsonl(&stdout)
}

/// Runs `commandindex search <query> --type <type_filter> --format json` and parses JSONL.
pub fn run_typed_search_jsonl(
    path: impl AsRef<std::path::Path>,
    query: &str,
    type_filter: &str,
) -> Vec<serde_json::Value> {
    let output = cmd()
        .args(["search", query, "--type", type_filter, "--format", "json"])
        .current_dir(path.as_ref())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    parse_jsonl(&stdout)
}

/// Runs `commandindex clean --path <path>` and asserts success.
pub fn run_clean(path: impl AsRef<std::path::Path>) {
    cmd()
        .args(["clean", "--path", path.as_ref().to_str().unwrap()])
        .assert()
        .success();
}

/// Initialize a git repo, create a file, commit, and return the commit hash.
/// CI-safe: uses `-c user.name` / `-c user.email` to avoid requiring global config.
pub fn git_init_with_commit(dir: &std::path::Path) -> String {
    use std::process::Command;

    // git init
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");

    // Create a file
    let docs = dir.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("a.md"),
        "---\ntags: auth\n---\n# Page A\nSee [Page B](b.md)\n",
    )
    .unwrap();
    std::fs::write(
        docs.join("b.md"),
        "---\ntags: auth\n---\n# Page B\nSee [Page A](a.md)\n",
    )
    .unwrap();

    // git add + commit
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add");

    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@test.com",
            "commit",
            "-m",
            "initial commit",
        ])
        .current_dir(dir)
        .output()
        .expect("git commit");
    assert!(output.status.success(), "git commit failed");

    // Get commit hash
    let hash_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("git rev-parse HEAD");
    String::from_utf8_lossy(&hash_output.stdout)
        .trim()
        .to_string()
}

mod common;

use predicates::prelude::*;

#[test]
fn help_flag_shows_usage() {
    common::cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: commandindex <COMMAND>"))
        .stdout(predicate::str::contains("index"))
        .stdout(predicate::str::contains("search"))
        .stdout(predicate::str::contains("update"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("clean"));
}

#[test]
fn version_flag_shows_version() {
    common::cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("commandindex"));
}

#[test]
fn no_args_shows_error() {
    common::cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn index_subcommand_accepts_path_option() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn search_without_index_shows_error() {
    // Run search from a temp directory where no index exists
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn search_with_all_options_accepted() {
    // Verify all options are accepted by clap (even if search fails due to no index)
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args([
            "search",
            "test query",
            "--format",
            "json",
            "--tag",
            "rust",
            "--path",
            "docs/",
            "--type",
            "markdown",
            "--heading",
            "Setup",
            "--limit",
            "5",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn update_without_index_shows_error() {
    // update without existing index should error
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Test\n\nContent\n").unwrap();
    common::cmd()
        .args(["update", "--path", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No index found"));
}

#[test]
fn search_requires_query_argument() {
    common::cmd()
        .arg("search")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn unknown_subcommand_shows_error() {
    common::cmd()
        .arg("unknown")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

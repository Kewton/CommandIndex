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
fn search_subcommand_exits_with_not_implemented() {
    common::cmd()
        .args(["search", "test query"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}

#[test]
fn update_subcommand_exits_with_not_implemented() {
    common::cmd()
        .arg("update")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}

#[test]
fn status_subcommand_exits_with_not_implemented() {
    common::cmd()
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
}

#[test]
fn clean_subcommand_exits_with_not_implemented() {
    common::cmd()
        .arg("clean")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not yet implemented"));
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

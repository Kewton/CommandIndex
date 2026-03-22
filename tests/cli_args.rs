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
        .stdout(predicate::str::contains("clean"))
        .stdout(predicate::str::contains("context"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("import"));
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
fn search_requires_query_or_symbol() {
    common::cmd()
        .arg("search")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Either").or(predicate::str::contains("required")));
}

#[test]
fn search_symbol_option_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "--symbol", "my_func"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Symbol database not found"));
}

#[test]
fn search_query_and_symbol_conflict() {
    common::cmd()
        .args(["search", "query", "--symbol", "name"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn unknown_subcommand_shows_error() {
    common::cmd()
        .arg("unknown")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn search_type_invalid_value_rejected() {
    common::cmd()
        .args(["search", "test query", "--type", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'invalid'"));
}

#[test]
fn search_type_valid_values_accepted() {
    // Each valid type should be accepted by clap (fails only because no index exists)
    for valid_type in &["markdown", "typescript", "python", "code"] {
        let tmp = tempfile::tempdir().expect("create temp dir");
        common::cmd()
            .current_dir(tmp.path())
            .args(["search", "test query", "--type", valid_type])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Index not found"));
    }
}

#[test]
fn search_semantic_and_symbol_conflict() {
    common::cmd()
        .args(["search", "--semantic", "query", "--symbol", "name"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_semantic_and_related_conflict() {
    common::cmd()
        .args(["search", "--semantic", "query", "--related", "file.rs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_semantic_and_query_conflict() {
    common::cmd()
        .args(["search", "query", "--semantic", "semantic query"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_semantic_and_heading_conflict() {
    common::cmd()
        .args(["search", "--semantic", "query", "--heading", "intro"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_semantic_option_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "--semantic", "how to use"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_no_semantic_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test", "--no-semantic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn test_no_semantic_conflicts_with_semantic() {
    common::cmd()
        .args(["search", "--semantic", "query", "--no-semantic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_no_semantic_conflicts_with_symbol() {
    common::cmd()
        .args(["search", "--symbol", "name", "--no-semantic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_no_semantic_conflicts_with_related() {
    common::cmd()
        .args(["search", "--related", "file.rs", "--no-semantic"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_with_rerank_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query", "--rerank"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn search_with_rerank_and_rerank_top_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query", "--rerank", "--rerank-top", "30"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn search_rerank_conflicts_with_symbol() {
    common::cmd()
        .args(["search", "--symbol", "name", "--rerank"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_rerank_conflicts_with_related() {
    common::cmd()
        .args(["search", "--related", "file.rs", "--rerank"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_rerank_conflicts_with_semantic() {
    common::cmd()
        .args(["search", "--semantic", "query", "--rerank"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_rerank_top_requires_rerank() {
    common::cmd()
        .args(["search", "test query", "--rerank-top", "20"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("required").or(predicate::str::contains("can only be used")),
        );
}

#[test]
fn config_show_runs_successfully() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["config", "show"])
        .current_dir(tmp.path())
        .assert()
        .success();
}

#[test]
fn config_path_runs_successfully() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["config", "path"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No config files loaded"));
}

#[test]
fn config_show_with_team_config() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        tmp.path().join("commandindex.toml"),
        "[embedding]\nprovider = \"ollama\"\nmodel = \"custom-model\"\n",
    )
    .unwrap();
    common::cmd()
        .args(["config", "show"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("custom-model"))
        .stdout(predicate::str::contains("api_key"));
}

#[test]
fn config_path_with_team_config() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        tmp.path().join("commandindex.toml"),
        "[embedding]\nprovider = \"ollama\"\n",
    )
    .unwrap();
    common::cmd()
        .args(["config", "path"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[team]"))
        .stdout(predicate::str::contains("commandindex.toml"));
}

#[test]
fn config_path_legacy_shows_deprecated() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ci_dir = tmp.path().join(".commandindex");
    std::fs::create_dir_all(&ci_dir).unwrap();
    std::fs::write(
        ci_dir.join("config.toml"),
        "[embedding]\nprovider = \"ollama\"\n",
    )
    .unwrap();
    common::cmd()
        .args(["config", "path"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[deprecated]"));
}

#[test]
fn config_help_shows_subcommands() {
    common::cmd()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("path"));
}

#[test]
fn search_limit_is_optional() {
    // Verify that search works without explicit --limit (it's now Option)
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}

#[test]
fn search_with_explicit_limit_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query", "--limit", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Index not found"));
}


// --- Workspace CLI option tests ---

#[test]
fn search_workspace_option_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["search", "test query", "--workspace", "workspace.toml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Workspace error"));
}

#[test]
fn search_workspace_with_repo_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args([
            "search",
            "test query",
            "--workspace",
            "workspace.toml",
            "--repo",
            "backend",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Workspace error"));
}

#[test]
fn search_repo_without_workspace_fails() {
    common::cmd()
        .args(["search", "test query", "--repo", "backend"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("required").or(predicate::str::contains("can only be used")),
        );
}

#[test]
fn search_workspace_conflicts_with_symbol() {
    common::cmd()
        .args([
            "search",
            "--symbol",
            "my_func",
            "--workspace",
            "workspace.toml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_workspace_conflicts_with_related() {
    common::cmd()
        .args([
            "search",
            "--related",
            "file.rs",
            "--workspace",
            "workspace.toml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn search_workspace_conflicts_with_semantic() {
    common::cmd()
        .args([
            "search",
            "--semantic",
            "query",
            "--workspace",
            "workspace.toml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn status_workspace_option_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["status", "--workspace", "workspace.toml"])
        .assert()
        .failure();
}

#[test]
fn update_workspace_option_accepted() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .current_dir(tmp.path())
        .args(["update", "--workspace", "workspace.toml"])
        .assert()
        .failure();
}

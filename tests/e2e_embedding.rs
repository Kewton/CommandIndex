mod common;
use predicates::prelude::*;

#[test]
fn embed_help_shows_usage() {
    common::cmd()
        .args(["embed", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generate embeddings"))
        .stdout(predicate::str::contains("--path"));
}

#[test]
fn embed_without_index_shows_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    common::cmd()
        .args(["embed", "--path", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No index found"));
}

#[test]
fn clean_keep_embeddings_preserves_embeddings_db() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Create a markdown file and index it
    std::fs::write(dir.path().join("test.md"), "# Test\nContent").unwrap();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Create a dummy embeddings.db and config.toml in .commandindex/
    let commandindex_dir = dir.path().join(".commandindex");
    std::fs::write(commandindex_dir.join("embeddings.db"), "dummy").unwrap();
    std::fs::write(
        commandindex_dir.join("config.toml"),
        "[embedding]\nprovider = \"ollama\"\n",
    )
    .unwrap();

    // Clean with --keep-embeddings
    common::cmd()
        .args([
            "clean",
            "--path",
            dir.path().to_str().unwrap(),
            "--keep-embeddings",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("embeddings preserved"));

    // Verify embeddings.db and config.toml are preserved
    assert!(commandindex_dir.join("embeddings.db").exists());
    assert!(commandindex_dir.join("config.toml").exists());

    // Verify tantivy, manifest.json, state.json are removed
    assert!(!commandindex_dir.join("tantivy").exists());
    assert!(!commandindex_dir.join("manifest.json").exists());
    assert!(!commandindex_dir.join("state.json").exists());
}

#[test]
fn clean_without_keep_embeddings_removes_everything() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Create a markdown file and index it
    std::fs::write(dir.path().join("test.md"), "# Test\nContent").unwrap();
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Create a dummy embeddings.db in .commandindex/
    let commandindex_dir = dir.path().join(".commandindex");
    std::fs::write(commandindex_dir.join("embeddings.db"), "dummy").unwrap();

    // Clean without --keep-embeddings
    common::cmd()
        .args(["clean", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed index at .commandindex/"));

    // Verify entire .commandindex/ is removed
    assert!(!commandindex_dir.exists());
}

#[test]
fn index_with_embedding_help_shows_option() {
    common::cmd()
        .args(["index", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--with-embedding"));
}

#[test]
fn update_with_embedding_help_shows_option() {
    common::cmd()
        .args(["update", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--with-embedding"));
}

#[test]
fn clean_keep_embeddings_help_shows_option() {
    common::cmd()
        .args(["clean", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--keep-embeddings"));
}

#[test]
fn help_shows_embed_subcommand() {
    common::cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("embed"));
}

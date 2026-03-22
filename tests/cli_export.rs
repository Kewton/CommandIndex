mod common;

use std::collections::HashSet;
use std::io::Read;

use flate2::read::GzDecoder;
use tar::Archive;

/// Helper: create an index and return the temp dir
fn setup_indexed_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Hello\n\nWorld\n").unwrap();
    common::run_index(dir.path());
    dir
}

/// Helper: list file names in a tar.gz archive
fn list_archive_entries(archive_path: &std::path::Path) -> HashSet<String> {
    let file = std::fs::File::open(archive_path).unwrap();
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut names = HashSet::new();
    for entry in archive.entries().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().to_string();
        names.insert(path);
    }
    names
}

#[test]
fn export_basic() {
    let dir = setup_indexed_dir();
    let output = dir.path().join("snapshot.tar.gz");

    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    let result = commandindex::cli::export::run(dir.path(), &output, &options);
    assert!(result.is_ok(), "export should succeed: {:?}", result.err());

    let result = result.unwrap();
    assert!(result.output_path.exists());
    assert!(result.archive_size > 0);

    // Verify archive contents
    let entries = list_archive_entries(&output);
    assert!(
        entries.contains("export_meta.json"),
        "should contain export_meta.json"
    );
    assert!(entries.contains("state.json"), "should contain state.json");
}

#[test]
fn export_not_initialized() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let output = dir.path().join("snapshot.tar.gz");

    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    let result = commandindex::cli::export::run(dir.path(), &output, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not initialized"));
}

#[test]
fn export_excludes_config_local_toml() {
    let dir = setup_indexed_dir();

    // Create a config.local.toml inside .commandindex
    let ci_dir = dir.path().join(".commandindex");
    std::fs::write(ci_dir.join("config.local.toml"), "secret = 'value'\n").unwrap();

    let output = dir.path().join("snapshot.tar.gz");
    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    commandindex::cli::export::run(dir.path(), &output, &options).unwrap();

    let entries = list_archive_entries(&output);
    assert!(
        !entries.contains("config.local.toml"),
        "config.local.toml should be excluded"
    );
}

#[test]
fn export_excludes_embeddings_by_default() {
    let dir = setup_indexed_dir();

    // Create a fake embeddings.db inside .commandindex
    let ci_dir = dir.path().join(".commandindex");
    std::fs::write(ci_dir.join("embeddings.db"), "fake-embeddings").unwrap();

    let output = dir.path().join("snapshot.tar.gz");
    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    commandindex::cli::export::run(dir.path(), &output, &options).unwrap();

    let entries = list_archive_entries(&output);
    assert!(
        !entries.contains("embeddings.db"),
        "embeddings.db should be excluded by default"
    );
}

#[test]
fn export_includes_embeddings_when_requested() {
    let dir = setup_indexed_dir();

    // Create a fake embeddings.db inside .commandindex
    let ci_dir = dir.path().join(".commandindex");
    std::fs::write(ci_dir.join("embeddings.db"), "fake-embeddings").unwrap();

    let output = dir.path().join("snapshot.tar.gz");
    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: true,
    };
    commandindex::cli::export::run(dir.path(), &output, &options).unwrap();

    let entries = list_archive_entries(&output);
    assert!(
        entries.contains("embeddings.db"),
        "embeddings.db should be included with --with-embeddings"
    );
}

#[test]
fn export_sanitizes_index_root() {
    let dir = setup_indexed_dir();
    let output = dir.path().join("snapshot.tar.gz");

    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    commandindex::cli::export::run(dir.path(), &output, &options).unwrap();

    // Read state.json from the archive
    let file = std::fs::File::open(&output).unwrap();
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().to_string();
        if path == "state.json" {
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            assert!(
                content.contains("__COMMANDINDEX_EXPORT_PLACEHOLDER__"),
                "state.json should have sanitized index_root"
            );
            assert!(
                !content.contains(dir.path().to_str().unwrap()),
                "state.json should not contain original path"
            );
            return;
        }
    }
    panic!("state.json not found in archive");
}

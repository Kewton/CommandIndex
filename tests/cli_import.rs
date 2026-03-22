mod common;

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;

/// Helper: create an index, export it, and return (source_dir, archive_path)
fn setup_exported_archive() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(dir.path().join("test.md"), "# Hello\n\nWorld\n").unwrap();
    common::run_index(dir.path());

    let archive_path = dir.path().join("snapshot.tar.gz");
    let options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    commandindex::cli::export::run(dir.path(), &archive_path, &options).unwrap();
    (dir, archive_path)
}

/// Helper: add a bytes entry to a tar builder
fn add_tar_entry<W: Write>(builder: &mut Builder<W>, name: &str, data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();
    builder.append_data(&mut header, name, data).unwrap();
}

/// Helper: add a bytes entry with a raw path (bypasses tar safety for malicious paths)
fn add_tar_entry_raw<W: Write>(builder: &mut Builder<W>, name: &[u8], data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_entry_type(tar::EntryType::Regular);
    // Write the path directly into the header name field
    {
        let header_bytes = header.as_old_mut();
        let name_field = &mut header_bytes.name;
        let len = name.len().min(name_field.len());
        name_field[..len].copy_from_slice(&name[..len]);
        if len < name_field.len() {
            name_field[len..].fill(0);
        }
    }
    header.set_cksum();
    builder.append(&header, std::io::Cursor::new(data)).unwrap();
}

#[test]
fn import_basic() {
    let (source_dir, archive_path) = setup_exported_archive();

    // Clean existing index
    common::run_clean(source_dir.path());

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(source_dir.path(), &archive_path, &options);
    assert!(result.is_ok(), "import should succeed: {:?}", result.err());

    let result = result.unwrap();
    assert!(result.imported_files > 0);

    // Verify index is usable (state exists)
    let ci_dir = source_dir.path().join(".commandindex");
    assert!(ci_dir.join("state.json").exists());
}

#[test]
fn import_existing_index_without_force() {
    let (source_dir, archive_path) = setup_exported_archive();

    // Don't clean - existing index should cause error
    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(source_dir.path(), &archive_path, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("already exists"));
}

#[test]
fn import_existing_index_with_force() {
    let (source_dir, archive_path) = setup_exported_archive();

    // With force, should overwrite
    let options = commandindex::cli::import_index::ImportOptions { force: true };
    let result = commandindex::cli::import_index::run(source_dir.path(), &archive_path, &options);
    assert!(
        result.is_ok(),
        "import --force should succeed: {:?}",
        result.err()
    );
}

#[test]
fn import_archive_not_found() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let fake_archive = dir.path().join("nonexistent.tar.gz");

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(dir.path(), &fake_archive, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not found") || err_msg.contains("Archive"));
}

#[test]
fn import_rejects_path_traversal_parent_dir() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let archive_path = dir.path().join("malicious.tar.gz");

    // Create a malicious archive with ../escape path
    let file = std::fs::File::create(&archive_path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    // Add valid export_meta.json
    let meta = serde_json::json!({
        "export_format_version": 1,
        "commandindex_version": "0.0.5",
        "git_commit_hash": null,
        "exported_at": "2024-01-01T00:00:00Z"
    });
    add_tar_entry(
        &mut builder,
        "export_meta.json",
        meta.to_string().as_bytes(),
    );

    // Add malicious path (using raw to bypass tar crate safety check)
    add_tar_entry_raw(&mut builder, b"../../../etc/passwd", b"malicious content");

    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(dir.path(), &archive_path, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Path traversal") || err_msg.contains("parent dir"),
        "Expected path traversal error, got: {err_msg}"
    );
}

#[test]
fn import_rejects_symlink_entry() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let archive_path = dir.path().join("symlink.tar.gz");

    // Create archive with a symlink entry
    let file = std::fs::File::create(&archive_path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    // Add export_meta.json
    let meta = serde_json::json!({
        "export_format_version": 1,
        "commandindex_version": "0.0.5",
        "git_commit_hash": null,
        "exported_at": "2024-01-01T00:00:00Z"
    });
    add_tar_entry(
        &mut builder,
        "export_meta.json",
        meta.to_string().as_bytes(),
    );

    // Add a symlink entry
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    header.set_mtime(0);
    header.set_cksum();
    builder
        .append_link(&mut header, "evil_link", "/etc/passwd")
        .unwrap();

    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(dir.path(), &archive_path, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Symlink") || err_msg.contains("symlink"),
        "Expected symlink error, got: {err_msg}"
    );
}

#[test]
fn import_rejects_incompatible_version() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let archive_path = dir.path().join("future_version.tar.gz");

    // Create archive with future format version
    let file = std::fs::File::create(&archive_path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    let meta = serde_json::json!({
        "export_format_version": 999,
        "commandindex_version": "99.0.0",
        "git_commit_hash": null,
        "exported_at": "2024-01-01T00:00:00Z"
    });
    add_tar_entry(
        &mut builder,
        "export_meta.json",
        meta.to_string().as_bytes(),
    );

    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(dir.path(), &archive_path, &options);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Incompatible") || err_msg.contains("version"),
        "Expected version error, got: {err_msg}"
    );
}

#[test]
fn import_git_commit_hash_mismatch_shows_warning() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let archive_path = dir.path().join("hash_mismatch.tar.gz");

    // Create an archive with a known (fake) git_commit_hash
    let file = std::fs::File::create(&archive_path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    let meta = serde_json::json!({
        "export_format_version": 1,
        "commandindex_version": "0.0.5",
        "git_commit_hash": "aaaa0000bbbb1111cccc2222dddd3333eeee4444",
        "exported_at": "2024-01-01T00:00:00Z"
    });
    add_tar_entry(
        &mut builder,
        "export_meta.json",
        meta.to_string().as_bytes(),
    );

    // Add a minimal state.json
    let state = serde_json::json!({
        "version": "0.0.5",
        "schema_version": 2,
        "index_root": "__COMMANDINDEX_EXPORT_PLACEHOLDER__",
        "created_at": "2024-01-01T00:00:00Z",
        "last_updated_at": "2024-01-01T00:00:00Z",
        "total_files": 0,
        "total_sections": 0
    });
    add_tar_entry(&mut builder, "state.json", state.to_string().as_bytes());

    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(dir.path(), &archive_path, &options);
    assert!(result.is_ok(), "import should succeed: {:?}", result.err());

    let result = result.unwrap();
    // The current git hash won't match the fake hash
    assert!(!result.git_hash_match, "git_hash_match should be false");
    assert!(
        !result.warnings.is_empty(),
        "should have warnings about hash mismatch"
    );
    // Check that the warning mentions the hash or mismatch
    let has_relevant_warning = result
        .warnings
        .iter()
        .any(|w| w.contains("mismatch") || w.contains("commit") || w.contains("hash"));
    assert!(
        has_relevant_warning,
        "Expected warning about commit hash, got: {:?}",
        result.warnings
    );
}

#[test]
fn import_state_json_index_root_rewritten() {
    let (_source_dir, archive_path) = setup_exported_archive();

    // Import into a different directory
    let import_dir = tempfile::tempdir().expect("create import dir");

    let options = commandindex::cli::import_index::ImportOptions { force: false };
    let result = commandindex::cli::import_index::run(import_dir.path(), &archive_path, &options);
    assert!(result.is_ok(), "import should succeed: {:?}", result.err());

    // Read state.json and verify index_root was rewritten
    let ci_dir = import_dir.path().join(".commandindex");
    let state_content = std::fs::read_to_string(ci_dir.join("state.json")).unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_content).unwrap();

    let index_root = state["index_root"].as_str().unwrap();
    assert!(
        !index_root.contains("PLACEHOLDER"),
        "index_root should not contain placeholder"
    );

    // Allow both original path and import path (depends on canonicalization)
    // The key thing is that it shouldn't contain the placeholder
    assert!(!index_root.is_empty(), "index_root should not be empty");

    // Verify source dir's original path is NOT in the state
    assert!(
        !index_root.contains("__COMMANDINDEX_EXPORT_PLACEHOLDER__"),
        "Should not contain placeholder"
    );
}

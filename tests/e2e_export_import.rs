mod common;

#[test]
fn e2e_export_import_search() {
    // 1. Create source with markdown content
    let source_dir = tempfile::tempdir().expect("create source dir");
    std::fs::write(
        source_dir.path().join("readme.md"),
        "# Project Overview\n\nThis is a test project for export import flow.\n",
    )
    .unwrap();

    // 2. Index the source
    common::run_index(source_dir.path());

    // 3. Verify search works on source
    let results = common::run_search_jsonl(source_dir.path(), "test project");
    assert!(!results.is_empty(), "search should find results in source");

    // 4. Export
    let archive_path = source_dir.path().join("index-snapshot.tar.gz");
    let export_options = commandindex::cli::export::ExportOptions {
        with_embeddings: false,
    };
    let export_result =
        commandindex::cli::export::run(source_dir.path(), &archive_path, &export_options).unwrap();
    assert!(export_result.archive_size > 0);

    // 5. Import into a new directory
    let import_dir = tempfile::tempdir().expect("create import dir");
    // Copy the markdown file to the import dir (so relative paths work)
    std::fs::write(
        import_dir.path().join("readme.md"),
        "# Project Overview\n\nThis is a test project for export import flow.\n",
    )
    .unwrap();

    let import_options = commandindex::cli::import_index::ImportOptions { force: false };
    let import_result =
        commandindex::cli::import_index::run(import_dir.path(), &archive_path, &import_options)
            .unwrap();
    assert!(import_result.imported_files > 0);

    // 6. Verify search works on imported index
    let results = common::run_search_jsonl(import_dir.path(), "test project");
    assert!(
        !results.is_empty(),
        "search should find results after import"
    );

    // 7. Verify status works on imported index
    let status = common::run_status_json(import_dir.path());
    assert!(
        status.get("version").is_some(),
        "status should show version after import"
    );
}

#[test]
fn e2e_export_import_cli() {
    // Test via CLI binary
    let source_dir = tempfile::tempdir().expect("create source dir");
    std::fs::write(
        source_dir.path().join("doc.md"),
        "# Documentation\n\nSome content here.\n",
    )
    .unwrap();

    // Index
    common::run_index(source_dir.path());

    // Export via CLI
    let archive_path = source_dir.path().join("export.tar.gz");
    common::cmd()
        .args(["export", archive_path.to_str().unwrap()])
        .current_dir(source_dir.path())
        .assert()
        .success();

    assert!(archive_path.exists(), "archive should be created");

    // Import via CLI into new dir
    let import_dir = tempfile::tempdir().expect("create import dir");
    std::fs::write(
        import_dir.path().join("doc.md"),
        "# Documentation\n\nSome content here.\n",
    )
    .unwrap();

    common::cmd()
        .args(["import", archive_path.to_str().unwrap()])
        .current_dir(import_dir.path())
        .assert()
        .success();

    // Verify status works
    common::cmd()
        .args([
            "status",
            "--path",
            import_dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();
}

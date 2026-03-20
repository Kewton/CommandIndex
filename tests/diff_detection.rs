mod common;

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use commandindex::indexer::diff::{DiffResult, detect_changes, scan_files};
use commandindex::indexer::manifest::{
    FileEntry, Manifest, compute_file_hash, to_relative_path_string,
};
use commandindex::parser::ignore::IgnoreFilter;

// FileType is used in make_entry() via commandindex::indexer::manifest::FileType

fn make_entry(path: &str, hash: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        hash: hash.to_string(),
        last_modified: chrono::Utc::now(),
        sections: 1,
        file_type: commandindex::indexer::manifest::FileType::Markdown,
    }
}

fn setup_dir_with_md(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    for (name, content) in files {
        let p = dir.path().join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
    }
    dir
}

// --- Test 1: detect_changes_detects_added_files ---
#[test]
fn detect_changes_detects_added_files() {
    let dir = setup_dir_with_md(&[("new.md", "# New")]);
    let manifest = Manifest::new(); // empty

    let files = vec![dir.path().join("new.md")];
    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert_eq!(result.added.len(), 1);
    assert!(result.added[0].ends_with("new.md"));
    assert!(result.modified.is_empty());
    assert!(result.deleted.is_empty());
}

// --- Test 2: detect_changes_detects_modified_files ---
#[test]
fn detect_changes_detects_modified_files() {
    let dir = setup_dir_with_md(&[("doc.md", "# Original")]);
    let hash = compute_file_hash(&dir.path().join("doc.md")).unwrap();

    // Manifest has old hash
    let manifest = Manifest {
        files: vec![make_entry("doc.md", "sha256:0000old")],
    };

    let files = vec![dir.path().join("doc.md")];
    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert!(result.added.is_empty());
    assert_eq!(result.modified.len(), 1);
    assert!(result.modified[0].ends_with("doc.md"));
    assert!(result.deleted.is_empty());
    // Verify hash actually differs
    assert_ne!(hash, "sha256:0000old");
}

// --- Test 3: detect_changes_detects_deleted_files ---
#[test]
fn detect_changes_detects_deleted_files() {
    let dir = setup_dir_with_md(&[]); // empty dir, no files on disk
    let manifest = Manifest {
        files: vec![make_entry("gone.md", "sha256:abc")],
    };

    let files: Vec<std::path::PathBuf> = vec![]; // no current files
    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert!(result.added.is_empty());
    assert!(result.modified.is_empty());
    assert_eq!(result.deleted.len(), 1);
    assert!(result.deleted[0].ends_with("gone.md"));
}

// --- Test 4: detect_changes_skips_unchanged_files ---
#[test]
fn detect_changes_skips_unchanged_files() {
    let dir = setup_dir_with_md(&[("stable.md", "# Stable content")]);
    let hash = compute_file_hash(&dir.path().join("stable.md")).unwrap();

    let manifest = Manifest {
        files: vec![make_entry("stable.md", &hash)],
    };

    let files = vec![dir.path().join("stable.md")];
    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert!(result.added.is_empty());
    assert!(result.modified.is_empty());
    assert!(result.deleted.is_empty());
    assert_eq!(result.unchanged, 1);
}

// --- Test 5: scan_files_applies_ignore_filter ---
#[test]
fn scan_files_applies_ignore_filter() {
    let dir = setup_dir_with_md(&[("included.md", "# Yes"), ("node_modules/dep.md", "# No")]);

    let ignore = IgnoreFilter::default(); // ignores node_modules/**
    let result = scan_files(dir.path(), &ignore, &["md"]).unwrap();

    assert_eq!(result.files.len(), 1);
    assert!(result.files[0].ends_with("included.md"));
    assert_eq!(result.ignored_count, 1);
}

// --- Test 6: detect_changes_with_empty_manifest ---
#[test]
fn detect_changes_with_empty_manifest() {
    let dir = setup_dir_with_md(&[("a.md", "# A"), ("b.md", "# B")]);
    let manifest = Manifest::new();

    let files = vec![dir.path().join("a.md"), dir.path().join("b.md")];
    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert_eq!(result.added.len(), 2);
    assert!(result.modified.is_empty());
    assert!(result.deleted.is_empty());
    assert_eq!(result.unchanged, 0);
}

// --- Test 7: diff_result_is_empty ---
#[test]
fn diff_result_is_empty() {
    let empty = DiffResult {
        added: vec![],
        modified: vec![],
        deleted: vec![],
        unchanged: 5,
    };
    assert!(empty.is_empty());

    let not_empty = DiffResult {
        added: vec![std::path::PathBuf::from("x.md")],
        modified: vec![],
        deleted: vec![],
        unchanged: 0,
    };
    assert!(!not_empty.is_empty());
}

// --- Test 8: roundtrip_index_then_detect_changes ---
#[test]
fn roundtrip_index_then_detect_changes() {
    let dir = setup_dir_with_md(&[("hello.md", "# Hello\n\nWorld\n")]);

    // Run actual index command
    common::cmd()
        .args(["index", "--path", dir.path().to_str().unwrap()])
        .assert()
        .success();

    // Load manifest
    let ci_dir = dir.path().join(".commandindex");
    let manifest = Manifest::load(&ci_dir).expect("load manifest");

    // Scan files
    let ignore = IgnoreFilter::default();
    let scan = scan_files(dir.path(), &ignore, &["md"]).unwrap();

    // Detect changes - should all be unchanged
    let result = detect_changes(dir.path(), &manifest, &scan.files).unwrap();

    assert!(result.added.is_empty());
    assert!(result.modified.is_empty());
    assert!(result.deleted.is_empty());
    assert!(result.unchanged > 0);
}

// --- Test 9a: load_or_default_returns_empty_on_not_found ---
#[test]
fn load_or_default_returns_empty_on_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let non_existent = dir.path().join("does_not_exist");
    let manifest = Manifest::load_or_default(&non_existent).unwrap();
    assert_eq!(manifest.file_count(), 0);
}

// --- Test 9b: load_or_default_propagates_other_errors ---
#[test]
fn load_or_default_propagates_other_errors() {
    let dir = tempfile::tempdir().unwrap();
    let ci_dir = dir.path().join(".commandindex");
    fs::create_dir_all(&ci_dir).unwrap();
    // Write invalid JSON so it triggers a Json error, not NotFound
    fs::write(ci_dir.join("manifest.json"), "NOT VALID JSON").unwrap();
    let result = Manifest::load_or_default(&ci_dir);
    assert!(result.is_err());
}

// --- Test 10: scan_files_filters_by_extension ---
#[test]
fn scan_files_filters_by_extension() {
    let dir = setup_dir_with_md(&[
        ("readme.md", "# Readme"),
        ("code.rs", "fn main() {}"),
        ("data.txt", "hello"),
    ]);

    let ignore = IgnoreFilter::from_content(""); // no ignore rules
    let result = scan_files(dir.path(), &ignore, &["md"]).unwrap();

    assert_eq!(result.files.len(), 1);
    assert!(result.files[0].ends_with("readme.md"));
}

// --- Test 11: detect_changes_mixed_added_modified_deleted ---
#[test]
fn detect_changes_mixed_added_modified_deleted() {
    let dir = setup_dir_with_md(&[
        ("kept.md", "# Kept same"),
        ("changed.md", "# Changed content"),
        ("brand_new.md", "# Brand new"),
    ]);

    let kept_hash = compute_file_hash(&dir.path().join("kept.md")).unwrap();

    let manifest = Manifest {
        files: vec![
            make_entry("kept.md", &kept_hash),           // unchanged
            make_entry("changed.md", "sha256:oldhash"),  // modified
            make_entry("removed.md", "sha256:whatever"), // deleted
        ],
    };

    let files = vec![
        dir.path().join("kept.md"),
        dir.path().join("changed.md"),
        dir.path().join("brand_new.md"),
    ];

    let result = detect_changes(dir.path(), &manifest, &files).unwrap();

    assert_eq!(result.added.len(), 1, "added");
    assert_eq!(result.modified.len(), 1, "modified");
    assert_eq!(result.deleted.len(), 1, "deleted");
    assert_eq!(result.unchanged, 1, "unchanged");

    assert!(result.added[0].ends_with("brand_new.md"));
    assert!(result.modified[0].ends_with("changed.md"));
    assert!(result.deleted[0].ends_with("removed.md"));
}

// --- Bonus: to_relative_path_string ---
#[test]
fn to_relative_path_string_works() {
    let base = Path::new("/home/user/project");
    let abs = Path::new("/home/user/project/docs/readme.md");
    assert_eq!(to_relative_path_string(abs, base), "docs/readme.md");

    // When not a prefix, returns the absolute path as-is
    let other = Path::new("/other/path/file.md");
    assert_eq!(to_relative_path_string(other, base), "/other/path/file.md");
}

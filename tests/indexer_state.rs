use chrono::Utc;
use commandindex::indexer::manifest::{self, FileEntry, Manifest};
use commandindex::indexer::state::IndexState;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// === IndexState tests ===

#[test]
fn test_state_new() {
    let state = IndexState::new(PathBuf::from("/test/repo"));
    assert_eq!(state.schema_version, 1);
    assert_eq!(state.total_files, 0);
    assert_eq!(state.total_sections, 0);
    assert_eq!(state.index_root, PathBuf::from("/test/repo"));
}

#[test]
fn test_state_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");

    let state = IndexState::new(PathBuf::from("/test/repo"));
    state.save(&ci_dir).unwrap();

    let loaded = IndexState::load(&ci_dir).unwrap();
    assert_eq!(state.version, loaded.version);
    assert_eq!(state.schema_version, loaded.schema_version);
    assert_eq!(state.total_files, loaded.total_files);
    assert_eq!(state.index_root, loaded.index_root);
}

#[test]
fn test_state_check_schema_version_ok() {
    let state = IndexState::new(PathBuf::from("/test/repo"));
    assert!(state.check_schema_version().is_ok());
}

#[test]
fn test_state_check_schema_version_mismatch() {
    let mut state = IndexState::new(PathBuf::from("/test/repo"));
    state.schema_version = 999;
    assert!(state.check_schema_version().is_err());
}

#[test]
fn test_state_exists_false() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");
    assert!(!IndexState::exists(&ci_dir));
}

#[test]
fn test_state_exists_true() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");

    let state = IndexState::new(PathBuf::from("/test/repo"));
    state.save(&ci_dir).unwrap();

    assert!(IndexState::exists(&ci_dir));
}

#[test]
fn test_state_touch_updates_timestamp() {
    let mut state = IndexState::new(PathBuf::from("/test/repo"));
    let original = state.last_updated_at;

    // Small delay to ensure timestamp differs
    std::thread::sleep(std::time::Duration::from_millis(10));
    state.touch();

    assert!(state.last_updated_at >= original);
}

#[test]
fn test_state_load_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");
    assert!(IndexState::load(&ci_dir).is_err());
}

// === Manifest tests ===

#[test]
fn test_manifest_new() {
    let manifest = Manifest::new();
    assert!(manifest.files.is_empty());
    assert_eq!(manifest.file_count(), 0);
}

#[test]
fn test_manifest_add_entry() {
    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/auth.md".to_string(),
        hash: "sha256:abc123".to_string(),
        last_modified: Utc::now(),
        sections: 5,
    });
    assert_eq!(manifest.file_count(), 1);
}

#[test]
fn test_manifest_find_by_path() {
    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/auth.md".to_string(),
        hash: "sha256:abc123".to_string(),
        last_modified: Utc::now(),
        sections: 5,
    });
    manifest.add_entry(FileEntry {
        path: "docs/api.md".to_string(),
        hash: "sha256:def456".to_string(),
        last_modified: Utc::now(),
        sections: 3,
    });

    let entry = manifest.find_by_path("docs/auth.md");
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().sections, 5);

    assert!(manifest.find_by_path("nonexistent.md").is_none());
}

#[test]
fn test_manifest_save_and_load() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");

    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/auth.md".to_string(),
        hash: "sha256:abc123".to_string(),
        last_modified: Utc::now(),
        sections: 5,
    });

    manifest.save(&ci_dir).unwrap();

    let loaded = Manifest::load(&ci_dir).unwrap();
    assert_eq!(loaded.file_count(), 1);
    assert_eq!(loaded.files[0].path, "docs/auth.md");
    assert_eq!(loaded.files[0].hash, "sha256:abc123");
}

#[test]
fn test_manifest_load_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let ci_dir = tmp.path().join(".commandindex");
    assert!(Manifest::load(&ci_dir).is_err());
}

// === File hash tests ===

#[test]
fn test_compute_file_hash() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.md");
    fs::write(&file_path, "Hello, World!").unwrap();

    let hash = manifest::compute_file_hash(&file_path).unwrap();
    assert!(hash.starts_with("sha256:"));
    assert!(hash.len() > 10);
}

#[test]
fn test_compute_file_hash_deterministic() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.md");
    fs::write(&file_path, "Same content").unwrap();

    let hash1 = manifest::compute_file_hash(&file_path).unwrap();
    let hash2 = manifest::compute_file_hash(&file_path).unwrap();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_compute_file_hash_different_content() {
    let tmp = TempDir::new().unwrap();

    let file1 = tmp.path().join("file1.md");
    let file2 = tmp.path().join("file2.md");
    fs::write(&file1, "Content A").unwrap();
    fs::write(&file2, "Content B").unwrap();

    let hash1 = manifest::compute_file_hash(&file1).unwrap();
    let hash2 = manifest::compute_file_hash(&file2).unwrap();
    assert_ne!(hash1, hash2);
}

#[test]
fn test_compute_file_hash_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nonexistent.md");
    assert!(manifest::compute_file_hash(&path).is_err());
}

// === Manifest remove_by_path / upsert_entry tests ===

#[test]
fn manifest_remove_by_path_removes_entry() {
    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/target.md".to_string(),
        hash: "sha256:aaa".to_string(),
        last_modified: Utc::now(),
        sections: 2,
    });
    assert_eq!(manifest.file_count(), 1);

    manifest.remove_by_path("docs/target.md");
    assert!(manifest.find_by_path("docs/target.md").is_none());
    assert_eq!(manifest.file_count(), 0);
}

#[test]
fn manifest_remove_by_path_nonexistent_is_noop() {
    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/keep.md".to_string(),
        hash: "sha256:bbb".to_string(),
        last_modified: Utc::now(),
        sections: 1,
    });
    let count_before = manifest.file_count();

    manifest.remove_by_path("docs/nonexistent.md");
    assert_eq!(manifest.file_count(), count_before);
}

#[test]
fn manifest_upsert_entry_adds_new() {
    let mut manifest = Manifest::new();
    assert_eq!(manifest.file_count(), 0);

    manifest.upsert_entry(FileEntry {
        path: "docs/new.md".to_string(),
        hash: "sha256:ccc".to_string(),
        last_modified: Utc::now(),
        sections: 3,
    });
    assert_eq!(manifest.file_count(), 1);
    assert!(manifest.find_by_path("docs/new.md").is_some());
}

#[test]
fn manifest_upsert_entry_updates_existing() {
    let mut manifest = Manifest::new();
    manifest.add_entry(FileEntry {
        path: "docs/existing.md".to_string(),
        hash: "sha256:old".to_string(),
        last_modified: Utc::now(),
        sections: 1,
    });

    manifest.upsert_entry(FileEntry {
        path: "docs/existing.md".to_string(),
        hash: "sha256:new".to_string(),
        last_modified: Utc::now(),
        sections: 5,
    });

    assert_eq!(manifest.file_count(), 1);
    let entry = manifest.find_by_path("docs/existing.md").unwrap();
    assert_eq!(entry.hash, "sha256:new");
    assert_eq!(entry.sections, 5);
}

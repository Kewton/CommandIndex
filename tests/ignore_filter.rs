use commandindex::parser::ignore::IgnoreFilter;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// === Default rules tests ===

#[test]
fn test_default_ignores_node_modules() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new("node_modules/foo/bar.js")));
}

#[test]
fn test_default_ignores_target() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new("target/debug/build")));
}

#[test]
fn test_default_ignores_git() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new(".git/objects/abc")));
}

#[test]
fn test_default_ignores_commandindex() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new(".commandindex/tantivy/index")));
}

#[test]
fn test_default_ignores_min_js() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new("vendor/jquery.min.js")));
}

#[test]
fn test_default_ignores_lock_files() {
    let filter = IgnoreFilter::default();
    assert!(filter.is_ignored(Path::new("Cargo.lock")));
    assert!(filter.is_ignored(Path::new("yarn.lock")));
}

#[test]
fn test_default_allows_normal_files() {
    let filter = IgnoreFilter::default();
    assert!(!filter.is_ignored(Path::new("src/main.rs")));
    assert!(!filter.is_ignored(Path::new("docs/README.md")));
    assert!(!filter.is_ignored(Path::new("app.js")));
}

// === Custom rules tests ===

#[test]
fn test_custom_patterns() {
    let content = "*.log\nbuild/\nsecrets.json";
    let filter = IgnoreFilter::from_content(content);

    assert!(filter.is_ignored(Path::new("app.log")));
    assert!(filter.is_ignored(Path::new("build/output.bin")));
    assert!(filter.is_ignored(Path::new("secrets.json")));
    assert!(!filter.is_ignored(Path::new("src/main.rs")));
}

#[test]
fn test_comment_lines_ignored() {
    let content = "# This is a comment\n*.log\n# Another comment";
    let filter = IgnoreFilter::from_content(content);

    assert!(filter.is_ignored(Path::new("debug.log")));
    assert!(!filter.is_ignored(Path::new("src/main.rs")));
}

#[test]
fn test_empty_lines_ignored() {
    let content = "\n*.log\n\n*.tmp\n\n";
    let filter = IgnoreFilter::from_content(content);

    assert!(filter.is_ignored(Path::new("debug.log")));
    assert!(filter.is_ignored(Path::new("temp.tmp")));
}

#[test]
fn test_directory_pattern_with_trailing_slash() {
    let content = "vendor/";
    let filter = IgnoreFilter::from_content(content);

    assert!(filter.is_ignored(Path::new("vendor/lib/foo.js")));
    assert!(!filter.is_ignored(Path::new("src/vendor.rs")));
}

#[test]
fn test_invalid_pattern_skipped() {
    let content = "*.log\n[invalid\n*.tmp";
    let filter = IgnoreFilter::from_content(content);

    assert!(filter.is_ignored(Path::new("debug.log")));
    assert!(filter.is_ignored(Path::new("temp.tmp")));
    // Invalid pattern should be skipped, not cause an error
}

#[test]
fn test_empty_content() {
    let content = "";
    let filter = IgnoreFilter::from_content(content);
    assert!(!filter.is_ignored(Path::new("anything.rs")));
}

// === File-based tests ===

#[test]
fn test_from_file_exists() {
    let tmp = TempDir::new().unwrap();
    let ignore_path = tmp.path().join(".cmindexignore");
    fs::write(&ignore_path, "*.log\nbuild/").unwrap();

    let filter = IgnoreFilter::from_file(&ignore_path).unwrap();
    assert!(filter.is_ignored(Path::new("app.log")));
    assert!(filter.is_ignored(Path::new("build/output.bin")));
    assert!(!filter.is_ignored(Path::new("src/main.rs")));
}

#[test]
fn test_from_file_not_exists_uses_defaults() {
    let tmp = TempDir::new().unwrap();
    let ignore_path = tmp.path().join(".cmindexignore");
    // File does not exist

    let filter = IgnoreFilter::from_file(&ignore_path).unwrap();
    // Should use default rules
    assert!(filter.is_ignored(Path::new("node_modules/foo.js")));
    assert!(filter.is_ignored(Path::new("target/debug/build")));
    assert!(!filter.is_ignored(Path::new("src/main.rs")));
}

#[test]
fn test_only_comments_and_blanks() {
    let content = "# comment\n\n# another comment\n  \n";
    let filter = IgnoreFilter::from_content(content);
    assert!(!filter.is_ignored(Path::new("anything.txt")));
}

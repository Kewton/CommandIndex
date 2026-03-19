use commandindex::indexer::reader::IndexReaderWrapper;
use commandindex::indexer::writer::{IndexWriterWrapper, SectionDoc};
use tempfile::TempDir;

fn create_test_section(path: &str, heading: &str, body: &str, tags: &str) -> SectionDoc {
    SectionDoc {
        path: path.to_string(),
        heading: heading.to_string(),
        body: body.to_string(),
        tags: tags.to_string(),
        heading_level: 1,
        line_start: 1,
    }
}

// === Schema tests ===

#[test]
fn test_schema_creation() {
    let schema = commandindex::indexer::schema::IndexSchema::new();
    assert!(schema.schema.get_field("path").is_ok());
    assert!(schema.schema.get_field("heading").is_ok());
    assert!(schema.schema.get_field("body").is_ok());
    assert!(schema.schema.get_field("tags").is_ok());
    assert!(schema.schema.get_field("heading_level").is_ok());
    assert!(schema.schema.get_field("line_start").is_ok());
}

// === Writer tests ===

#[test]
fn test_create_index_in_ram() {
    let result = IndexWriterWrapper::open_in_ram();
    assert!(result.is_ok());
}

#[test]
fn test_create_index_on_disk() {
    let tmp = TempDir::new().unwrap();
    let index_dir = tmp.path().join("tantivy");
    let result = IndexWriterWrapper::open(&index_dir);
    assert!(result.is_ok());
}

#[test]
fn test_add_and_commit_section() {
    let (mut writer, _index) = IndexWriterWrapper::open_in_ram().unwrap();
    let section = create_test_section("docs/test.md", "Test Heading", "Test body content", "rust");
    assert!(writer.add_section(&section).is_ok());
    assert!(writer.commit().is_ok());
}

// === Reader tests ===

#[test]
fn test_search_english() {
    let (mut writer, index) = IndexWriterWrapper::open_in_ram().unwrap();

    writer
        .add_section(&create_test_section(
            "docs/auth.md",
            "Authentication",
            "This module handles user authentication and authorization.",
            "auth security",
        ))
        .unwrap();
    writer
        .add_section(&create_test_section(
            "docs/api.md",
            "API Reference",
            "REST API endpoints for the application.",
            "api http",
        ))
        .unwrap();
    writer.commit().unwrap();

    let reader = IndexReaderWrapper::from_index(index);
    let results = reader.search("authentication", 10).unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].path, "docs/auth.md");
}

#[test]
fn test_search_japanese() {
    let (mut writer, index) = IndexWriterWrapper::open_in_ram().unwrap();

    writer
        .add_section(&create_test_section(
            "docs/guide.md",
            "ユーザーガイド",
            "このドキュメントはユーザー向けの操作ガイドです。検索機能の使い方を説明します。",
            "ガイド 日本語",
        ))
        .unwrap();
    writer
        .add_section(&create_test_section(
            "docs/install.md",
            "インストール",
            "インストール手順について説明します。",
            "セットアップ",
        ))
        .unwrap();
    writer.commit().unwrap();

    let reader = IndexReaderWrapper::from_index(index);
    let results = reader.search("検索", 10).unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].path, "docs/guide.md");
}

#[test]
fn test_search_by_tags() {
    let (mut writer, index) = IndexWriterWrapper::open_in_ram().unwrap();

    writer
        .add_section(&create_test_section(
            "docs/a.md",
            "Doc A",
            "Content A",
            "rust cli",
        ))
        .unwrap();
    writer
        .add_section(&create_test_section(
            "docs/b.md",
            "Doc B",
            "Content B",
            "python web",
        ))
        .unwrap();
    writer.commit().unwrap();

    let reader = IndexReaderWrapper::from_index(index);
    let results = reader.search("rust", 10).unwrap();

    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.path == "docs/a.md"));
}

#[test]
fn test_search_no_results() {
    let (mut writer, index) = IndexWriterWrapper::open_in_ram().unwrap();

    writer
        .add_section(&create_test_section(
            "docs/a.md",
            "Title",
            "Some content",
            "test",
        ))
        .unwrap();
    writer.commit().unwrap();

    let reader = IndexReaderWrapper::from_index(index);
    let results = reader.search("nonexistent_query_xyz", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_search_result_fields() {
    let (mut writer, index) = IndexWriterWrapper::open_in_ram().unwrap();

    writer
        .add_section(&SectionDoc {
            path: "docs/test.md".to_string(),
            heading: "Test Heading".to_string(),
            body: "Test body content for search".to_string(),
            tags: "tag1 tag2".to_string(),
            heading_level: 2,
            line_start: 10,
        })
        .unwrap();
    writer.commit().unwrap();

    let reader = IndexReaderWrapper::from_index(index);
    let results = reader.search("search", 10).unwrap();

    assert_eq!(results.len(), 1);
    let r = &results[0];
    assert_eq!(r.path, "docs/test.md");
    assert_eq!(r.heading, "Test Heading");
    assert!(r.body.contains("search"));
    assert_eq!(r.tags, "tag1 tag2");
    assert_eq!(r.heading_level, 2);
    assert_eq!(r.line_start, 10);
    assert!(r.score > 0.0);
}

// === Disk-based index tests ===

#[test]
fn test_disk_index_write_and_read() {
    let tmp = TempDir::new().unwrap();
    let index_dir = tmp.path().join("tantivy");

    // Write
    {
        let mut writer = IndexWriterWrapper::open(&index_dir).unwrap();
        writer
            .add_section(&create_test_section(
                "docs/test.md",
                "Disk Test",
                "Content stored on disk.",
                "disk",
            ))
            .unwrap();
        writer.commit().unwrap();
    }

    // Read
    {
        let reader = IndexReaderWrapper::open(&index_dir).unwrap();
        let results = reader.search("disk", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "docs/test.md");
    }
}

#[test]
fn test_commandindex_tantivy_path() {
    let tmp = TempDir::new().unwrap();
    let index_dir = tmp.path().join(".commandindex").join("tantivy");

    let mut writer = IndexWriterWrapper::open(&index_dir).unwrap();
    writer
        .add_section(&create_test_section("test.md", "Title", "Body", ""))
        .unwrap();
    writer.commit().unwrap();

    assert!(index_dir.exists());

    let reader = IndexReaderWrapper::open(&index_dir).unwrap();
    let results = reader.search("Title", 10).unwrap();
    assert!(!results.is_empty());
}

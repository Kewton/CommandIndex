use commandindex::parser::{self, Link, LinkType};
use std::fs;
use tempfile::TempDir;

// === Section parsing tests ===

#[test]
fn test_parse_heading_levels() {
    let content = "# H1\n\nBody1\n\n## H2\n\nBody2\n\n### H3\n\nBody3";
    let doc = parser::markdown::parse_content(content);

    assert_eq!(doc.sections.len(), 3);
    assert_eq!(doc.sections[0].heading, "H1");
    assert_eq!(doc.sections[0].level, 1);
    assert_eq!(doc.sections[0].body, "Body1");
    assert_eq!(doc.sections[1].heading, "H2");
    assert_eq!(doc.sections[1].level, 2);
    assert_eq!(doc.sections[2].heading, "H3");
    assert_eq!(doc.sections[2].level, 3);
}

#[test]
fn test_parse_all_heading_levels() {
    let content = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
    let doc = parser::markdown::parse_content(content);

    assert_eq!(doc.sections.len(), 6);
    for (i, section) in doc.sections.iter().enumerate() {
        assert_eq!(section.level, (i + 1) as u8);
    }
}

#[test]
fn test_empty_file() {
    let doc = parser::markdown::parse_content("");
    assert!(doc.sections.is_empty());
    assert!(doc.frontmatter.is_none());
    assert!(doc.links.is_empty());
}

#[test]
fn test_no_headings() {
    let content = "Just some text\nwithout any headings.";
    let doc = parser::markdown::parse_content(content);
    assert!(doc.sections.is_empty());
}

#[test]
fn test_heading_without_space_is_not_heading() {
    let content = "#not-a-heading\n# Real heading";
    let doc = parser::markdown::parse_content(content);
    assert_eq!(doc.sections.len(), 1);
    assert_eq!(doc.sections[0].heading, "Real heading");
}

#[test]
fn test_section_multiline_body() {
    let content = "# Title\n\nLine 1\nLine 2\nLine 3";
    let doc = parser::markdown::parse_content(content);

    assert_eq!(doc.sections.len(), 1);
    assert!(doc.sections[0].body.contains("Line 1"));
    assert!(doc.sections[0].body.contains("Line 3"));
}

// === Frontmatter tests ===

#[test]
fn test_frontmatter_with_tags() {
    let content = "---\ntags:\n  - rust\n  - cli\ntitle: Test\n---\n# Heading\n\nBody";
    let doc = parser::markdown::parse_content(content);

    assert!(doc.frontmatter.is_some());
    let fm = doc.frontmatter.unwrap();
    assert_eq!(fm.tags, vec!["rust", "cli"]);
    assert!(fm.raw.contains_key("title"));
}

#[test]
fn test_frontmatter_without_tags() {
    let content = "---\ntitle: Test\nauthor: Someone\n---\n# Heading";
    let doc = parser::markdown::parse_content(content);

    assert!(doc.frontmatter.is_some());
    let fm = doc.frontmatter.unwrap();
    assert!(fm.tags.is_empty());
}

#[test]
fn test_no_frontmatter() {
    let content = "# Just a heading\n\nSome body text.";
    let doc = parser::markdown::parse_content(content);
    assert!(doc.frontmatter.is_none());
    assert_eq!(doc.sections.len(), 1);
}

#[test]
fn test_empty_frontmatter() {
    let content = "---\n---\n# Heading";
    let doc = parser::markdown::parse_content(content);
    // Empty YAML may parse as None or empty frontmatter
    assert_eq!(doc.sections.len(), 1);
}

// === Link extraction tests ===

#[test]
fn test_wiki_links() {
    let content = "# Links\n\nSee [[target-page]] and [[another page]].";
    let doc = parser::markdown::parse_content(content);

    let wiki_links: Vec<&Link> = doc
        .links
        .iter()
        .filter(|l| l.link_type == LinkType::WikiLink)
        .collect();
    assert_eq!(wiki_links.len(), 2);
    assert_eq!(wiki_links[0].target, "target-page");
    assert_eq!(wiki_links[1].target, "another page");
}

#[test]
fn test_markdown_links() {
    let content = "# Links\n\n[Click here](https://example.com) and [docs](./docs.md).";
    let doc = parser::markdown::parse_content(content);

    let md_links: Vec<&Link> = doc
        .links
        .iter()
        .filter(|l| l.link_type == LinkType::MarkdownLink)
        .collect();
    assert_eq!(md_links.len(), 2);
    assert_eq!(md_links[0].target, "https://example.com");
    assert_eq!(md_links[1].target, "./docs.md");
}

#[test]
fn test_mixed_links() {
    let content = "# Mixed\n\n[[wiki]] and [md](target.md)";
    let doc = parser::markdown::parse_content(content);
    assert_eq!(doc.links.len(), 2);
}

// === Directory traversal tests ===

#[test]
fn test_parse_directory() {
    let tmp = TempDir::new().unwrap();

    // Create some .md files
    fs::write(tmp.path().join("file1.md"), "# Title 1\n\nBody 1").unwrap();
    fs::write(
        tmp.path().join("file2.md"),
        "---\ntags:\n  - test\n---\n# Title 2\n\nBody 2",
    )
    .unwrap();

    // Create a subdirectory with another .md file
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("file3.md"), "# Nested\n\nNested body").unwrap();

    // Create a non-.md file (should be ignored)
    fs::write(tmp.path().join("readme.txt"), "Not markdown").unwrap();

    let docs = parser::parse_directory(tmp.path()).unwrap();
    assert_eq!(docs.len(), 3);
}

#[test]
fn test_parse_empty_directory() {
    let tmp = TempDir::new().unwrap();
    let docs = parser::parse_directory(tmp.path()).unwrap();
    assert!(docs.is_empty());
}

#[test]
fn test_line_start_with_frontmatter() {
    let content = "---\ntitle: Test\n---\n# Heading\n\nBody";
    let doc = parser::markdown::parse_content(content);

    assert_eq!(doc.sections.len(), 1);
    // heading should be after frontmatter lines
    assert!(doc.sections[0].line_start > 1);
}

#[test]
fn test_line_start_without_frontmatter() {
    let content = "# First\n\nBody\n\n## Second\n\nMore body";
    let doc = parser::markdown::parse_content(content);

    assert_eq!(doc.sections.len(), 2);
    assert_eq!(doc.sections[0].line_start, 1);
    assert!(doc.sections[1].line_start > 1);
}

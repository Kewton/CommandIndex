use commandindex::indexer::reader::SearchResult;
use commandindex::output::{OutputFormat, format_results};

fn make_result(path: &str, heading: &str, body: &str, tags: &str) -> SearchResult {
    SearchResult {
        path: path.to_string(),
        heading: heading.to_string(),
        body: body.to_string(),
        tags: tags.to_string(),
        heading_level: 2,
        line_start: 10,
        score: 1.5,
    }
}

fn format_to_string(results: &[SearchResult], format: OutputFormat) -> String {
    // テスト時はANSIカラーを無効化
    colored::control::set_override(false);
    let mut buf = Vec::new();
    format_results(results, format, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

// --- Human format tests ---

#[test]
fn test_human_format_basic() {
    let results = vec![make_result(
        "docs/auth.md",
        "認証フロー",
        "認証はJWTベースで行う",
        "",
    )];
    let output = format_to_string(&results, OutputFormat::Human);
    assert!(output.contains("docs/auth.md:10"));
    assert!(output.contains("[## 認証フロー]"));
    assert!(output.contains("認証はJWTベースで行う"));
}

#[test]
fn test_human_format_with_tags() {
    let results = vec![make_result("test.md", "Title", "Body", "auth security")];
    let output = format_to_string(&results, OutputFormat::Human);
    assert!(output.contains("Tags: auth, security"));
}

#[test]
fn test_human_format_no_tags() {
    let results = vec![make_result("test.md", "Title", "Body", "")];
    let output = format_to_string(&results, OutputFormat::Human);
    assert!(!output.contains("Tags:"));
}

#[test]
fn test_human_format_snippet_truncation() {
    let body = "line1\nline2\nline3\nline4";
    let results = vec![make_result("test.md", "Title", body, "")];
    let output = format_to_string(&results, OutputFormat::Human);
    assert!(output.contains("line1"));
    assert!(output.contains("line2"));
    assert!(output.contains("..."));
    assert!(!output.contains("line3"));
}

#[test]
fn test_human_format_long_single_line() {
    let body = "あ".repeat(150);
    let results = vec![make_result("test.md", "Title", &body, "")];
    let output = format_to_string(&results, OutputFormat::Human);
    // 120文字 + "..." で切り詰め
    assert!(output.contains("..."));
    // 元の150文字がそのまま含まれていないことを確認
    assert!(!output.contains(&body));
}

// --- JSON format tests ---

#[test]
fn test_json_format_basic() {
    let results = vec![make_result("test.md", "Title", "Body text", "tag1 tag2")];
    let output = format_to_string(&results, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["path"], "test.md");
    assert_eq!(parsed["heading"], "Title");
    assert_eq!(parsed["heading_level"], 2);
    assert_eq!(parsed["body"], "Body text");
    assert_eq!(parsed["line_start"], 10);
}

#[test]
fn test_json_format_tags_array() {
    let results = vec![make_result("test.md", "Title", "Body", "auth security")];
    let output = format_to_string(&results, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["tags"], serde_json::json!(["auth", "security"]));
}

#[test]
fn test_json_format_empty_tags() {
    let results = vec![make_result("test.md", "Title", "Body", "")];
    let output = format_to_string(&results, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["tags"], serde_json::json!([]));
}

#[test]
fn test_json_format_score() {
    let results = vec![make_result("test.md", "Title", "Body", "")];
    let output = format_to_string(&results, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed["score"].is_number());
    assert_eq!(parsed["score"].as_f64().unwrap(), 1.5);
}

// --- Path format tests ---

#[test]
fn test_path_format_basic() {
    let results = vec![
        make_result("docs/auth.md", "Title1", "Body1", ""),
        make_result("docs/api.md", "Title2", "Body2", ""),
    ];
    let output = format_to_string(&results, OutputFormat::Path);
    assert_eq!(output.trim(), "docs/auth.md\ndocs/api.md");
}

#[test]
fn test_path_format_dedup() {
    let results = vec![
        make_result("docs/auth.md", "Title1", "Body1", ""),
        make_result("docs/auth.md", "Title2", "Body2", ""),
        make_result("docs/api.md", "Title3", "Body3", ""),
    ];
    let output = format_to_string(&results, OutputFormat::Path);
    assert_eq!(output.trim(), "docs/auth.md\ndocs/api.md");
}

// --- Empty results test ---

#[test]
fn test_format_empty_results() {
    for format in [OutputFormat::Human, OutputFormat::Json, OutputFormat::Path] {
        let output = format_to_string(&[], format);
        assert!(
            output.is_empty(),
            "Empty results should produce no output for {:?}",
            format
        );
    }
}

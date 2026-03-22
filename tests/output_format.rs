use commandindex::indexer::reader::SearchResult;
use commandindex::output::{
    OutputFormat, SnippetConfig, WorkspaceSearchResult, format_results, format_workspace_results,
};

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

// --- Snippet config tests ---

fn format_human_to_string(results: &[SearchResult], snippet_config: SnippetConfig) -> String {
    colored::control::set_override(false);
    let mut buf = Vec::new();
    commandindex::output::human::format_human(results, &mut buf, snippet_config).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn test_snippet_custom_lines() {
    let body = "line1\nline2\nline3\nline4\nline5\nline6";
    let results = vec![make_result("test.md", "Title", body, "")];
    let config = SnippetConfig {
        lines: 5,
        chars: 120,
    };
    let output = format_human_to_string(&results, config);
    assert!(output.contains("line1"));
    assert!(output.contains("line5"));
    assert!(output.contains("..."));
    assert!(!output.contains("line6"));
}

#[test]
fn test_snippet_custom_chars() {
    let body = "あ".repeat(100);
    let results = vec![make_result("test.md", "Title", &body, "")];
    let config = SnippetConfig {
        lines: 2,
        chars: 50,
    };
    let output = format_human_to_string(&results, config);
    assert!(output.contains("..."));
    // 50文字分の「あ」が含まれていること
    assert!(output.contains(&"あ".repeat(50)));
    // 51文字分は含まれていないこと
    assert!(!output.contains(&"あ".repeat(51)));
}

#[test]
fn test_snippet_lines_zero_unlimited() {
    let body = "line1\nline2\nline3\nline4\nline5\nline6";
    let results = vec![make_result("test.md", "Title", body, "")];
    let config = SnippetConfig { lines: 0, chars: 0 };
    let output = format_human_to_string(&results, config);
    assert!(output.contains("line1"));
    assert!(output.contains("line6"));
    assert!(!output.contains("..."));
}

#[test]
fn test_snippet_chars_zero_unlimited() {
    let body = "あ".repeat(200);
    let results = vec![make_result("test.md", "Title", &body, "")];
    let config = SnippetConfig { lines: 2, chars: 0 };
    let output = format_human_to_string(&results, config);
    // 単一行なのでchars=0(無制限)で全文表示される
    assert!(output.contains(&body));
    assert!(!output.contains("..."));
}

#[test]
fn test_snippet_default_unchanged() {
    let body = "line1\nline2\nline3\nline4";
    let results = vec![make_result("test.md", "Title", body, "")];
    let default_output = format_human_to_string(&results, SnippetConfig::default());
    let format_results_output = format_to_string(&results, OutputFormat::Human);
    assert_eq!(default_output, format_results_output);
}

// --- Workspace format tests ---

fn make_workspace_result(
    repo: &str,
    path: &str,
    heading: &str,
    body: &str,
    tags: &str,
) -> WorkspaceSearchResult {
    WorkspaceSearchResult {
        repository: repo.to_string(),
        result: make_result(path, heading, body, tags),
    }
}

fn format_workspace_to_string(results: &[WorkspaceSearchResult], format: OutputFormat) -> String {
    colored::control::set_override(false);
    let mut buf = Vec::new();
    format_workspace_results(results, format, &mut buf, SnippetConfig::default()).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn test_workspace_human_contains_repo_prefix() {
    let results = vec![make_workspace_result(
        "backend",
        "docs/auth.md",
        "認証フロー",
        "認証はJWTベースで行う",
        "",
    )];
    let output = format_workspace_to_string(&results, OutputFormat::Human);
    assert!(output.contains("[backend]"));
    assert!(output.contains("docs/auth.md:10"));
    assert!(output.contains("[## 認証フロー]"));
}

#[test]
fn test_workspace_json_contains_repository_field() {
    let results = vec![make_workspace_result(
        "backend",
        "docs/auth.md",
        "Title",
        "Body",
        "tag1",
    )];
    let output = format_workspace_to_string(&results, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["repository"], "backend");
    assert_eq!(parsed["path"], "docs/auth.md");
    assert_eq!(parsed["heading"], "Title");
}

#[test]
fn test_workspace_path_different_repos_same_path_not_deduped() {
    let results = vec![
        make_workspace_result("backend", "docs/README.md", "Title1", "Body1", ""),
        make_workspace_result("frontend", "docs/README.md", "Title2", "Body2", ""),
    ];
    let output = format_workspace_to_string(&results, OutputFormat::Path);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("[backend]"));
    assert!(lines[0].contains("docs/README.md"));
    assert!(lines[1].contains("[frontend]"));
    assert!(lines[1].contains("docs/README.md"));
}

#[test]
fn test_workspace_path_same_repo_same_path_deduped() {
    let results = vec![
        make_workspace_result("backend", "docs/README.md", "Title1", "Body1", ""),
        make_workspace_result("backend", "docs/README.md", "Title2", "Body2", ""),
    ];
    let output = format_workspace_to_string(&results, OutputFormat::Path);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("[backend]"));
}

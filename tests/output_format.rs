use commandindex::indexer::reader::SearchResult;
use commandindex::output::{
    ImpactPerFile, ImpactRelatedFile, ImpactResult, ImpactSummary, OutputFormat, SnippetConfig,
    WorkspaceSearchResult, format_impact_results, format_results, format_workspace_results,
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

// --- Impact format tests ---

fn make_impact_result() -> ImpactResult {
    ImpactResult {
        changed_files: vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
        impact: vec![
            ImpactPerFile {
                file: "src/a.rs".to_string(),
                related: vec![
                    ImpactRelatedFile {
                        path: "docs/a.md".to_string(),
                        score: 0.95,
                        relations: vec!["markdown_link".to_string()],
                    },
                    ImpactRelatedFile {
                        path: "tests/a_test.rs".to_string(),
                        score: 0.80,
                        relations: vec!["import_dependency".to_string()],
                    },
                    ImpactRelatedFile {
                        path: "tests/common.rs".to_string(),
                        score: 0.60,
                        relations: vec!["directory_proximity".to_string()],
                    },
                ],
            },
            ImpactPerFile {
                file: "src/b.rs".to_string(),
                related: vec![
                    ImpactRelatedFile {
                        path: "tests/b_test.rs".to_string(),
                        score: 0.70,
                        relations: vec!["import_dependency".to_string()],
                    },
                    ImpactRelatedFile {
                        path: "tests/common.rs".to_string(),
                        score: 0.50,
                        relations: vec!["directory_proximity".to_string()],
                    },
                ],
            },
        ],
        overlap: vec!["tests/common.rs".to_string()],
        summary: ImpactSummary {
            changed: 2,
            total_impacted: 4,
            overlap_count: 1,
        },
    }
}

fn format_impact_to_string(result: &ImpactResult, format: OutputFormat) -> String {
    colored::control::set_override(false);
    let mut buf = Vec::new();
    format_impact_results(result, format, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn test_impact_json_format() {
    let result = make_impact_result();
    let output = format_impact_to_string(&result, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
    // changed_files
    assert_eq!(
        parsed["changed_files"],
        serde_json::json!(["src/a.rs", "src/b.rs"])
    );
    // impact array
    let impact = parsed["impact"].as_array().unwrap();
    assert_eq!(impact.len(), 2);
    assert_eq!(impact[0]["file"], "src/a.rs");
    assert_eq!(impact[0]["related"][0]["path"], "docs/a.md");
    assert!(impact[0]["related"][0]["score"].as_f64().unwrap() > 0.9);
    assert_eq!(
        impact[0]["related"][0]["relations"],
        serde_json::json!(["markdown_link"])
    );
    // overlap
    assert_eq!(parsed["overlap"], serde_json::json!(["tests/common.rs"]));
    // summary
    assert_eq!(parsed["summary"]["changed"], 2);
    assert_eq!(parsed["summary"]["total_impacted"], 4);
    assert_eq!(parsed["summary"]["overlap_count"], 1);
}

#[test]
fn test_impact_human_format() {
    let result = make_impact_result();
    let output = format_impact_to_string(&result, OutputFormat::Human);
    assert!(output.contains("Impact analysis"));
    assert!(output.contains("2 changed file(s)"));
    assert!(output.contains("4 impacted file(s)"));
    // Per-file sections
    assert!(output.contains("src/a.rs:"));
    assert!(output.contains("docs/a.md"));
    assert!(output.contains("src/b.rs:"));
    // Overlap section
    assert!(output.contains("Overlap"));
    assert!(output.contains("tests/common.rs"));
    // Summary line
    assert!(output.contains("Summary: 2 changed, 4 impacted, 1 overlap"));
}

#[test]
fn test_impact_path_format() {
    let result = make_impact_result();
    let output = format_impact_to_string(&result, OutputFormat::Path);
    let lines: Vec<&str> = output.trim().lines().collect();
    // Union of all impacted paths, dedup, sorted by max score desc
    assert!(
        lines.len() == 4,
        "expected 4 unique paths, got {}",
        lines.len()
    );
    // docs/a.md (0.95) should be first
    assert_eq!(lines[0], "docs/a.md");
    // tests/a_test.rs (0.80) second
    assert_eq!(lines[1], "tests/a_test.rs");
    // tests/b_test.rs (0.70) third
    assert_eq!(lines[2], "tests/b_test.rs");
    // tests/common.rs (max(0.60, 0.50) = 0.60) fourth
    assert_eq!(lines[3], "tests/common.rs");
}

#[test]
fn test_impact_empty_results() {
    let result = ImpactResult {
        changed_files: vec!["src/main.rs".to_string()],
        impact: vec![ImpactPerFile {
            file: "src/main.rs".to_string(),
            related: vec![],
        }],
        overlap: vec![],
        summary: ImpactSummary {
            changed: 1,
            total_impacted: 0,
            overlap_count: 0,
        },
    };
    let output = format_impact_to_string(&result, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
    assert_eq!(parsed["summary"]["total_impacted"], 0);
    assert!(parsed["overlap"].as_array().unwrap().is_empty());
}

#[test]
fn test_impact_human_no_overlap() {
    let result = ImpactResult {
        changed_files: vec!["src/main.rs".to_string()],
        impact: vec![ImpactPerFile {
            file: "src/main.rs".to_string(),
            related: vec![ImpactRelatedFile {
                path: "src/lib.rs".to_string(),
                score: 0.9,
                relations: vec!["import_dependency".to_string()],
            }],
        }],
        overlap: vec![],
        summary: ImpactSummary {
            changed: 1,
            total_impacted: 1,
            overlap_count: 0,
        },
    };
    let output = format_impact_to_string(&result, OutputFormat::Human);
    // No overlap section when empty
    assert!(!output.contains("Overlap ("));
    assert!(output.contains("Summary: 1 changed, 1 impacted, 0 overlap"));
}

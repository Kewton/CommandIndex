mod common;

use predicates::prelude::*;

/// Helper: create a temporary .commandindex directory with knowledge graph data.
fn setup_issue_test_data(tmp: &std::path::Path) -> std::path::PathBuf {
    let ci_dir = tmp.join(".commandindex");
    std::fs::create_dir_all(&ci_dir).unwrap();

    let db_path = ci_dir.join("symbols.db");
    let store = commandindex::indexer::symbol_store::SymbolStore::open(&db_path).unwrap();
    store.create_tables().unwrap();

    use commandindex::indexer::knowledge::{DocSubtype, KnowledgeEntry, KnowledgeRelation};

    let entries = vec![
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/design/issue-140-issue-cmd-design-policy.md".to_string(),
            relation: KnowledgeRelation::HasDesign,
            doc_subtype: DocSubtype::DesignPolicy,
            date: None,
        },
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/issue/140/work-plan.md".to_string(),
            relation: KnowledgeRelation::HasWorkplan,
            doc_subtype: DocSubtype::WorkPlan,
            date: None,
        },
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/issue/140/issue-review/summary-report.md".to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::IssueReview,
            date: None,
        },
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/issue/140/multi-stage-design-review/summary-report.md"
                .to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::DesignReview,
            date: None,
        },
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/issue/140/pm-auto-dev/iteration-1/progress-report.md"
                .to_string(),
            relation: KnowledgeRelation::HasProgress,
            doc_subtype: DocSubtype::ProgressReport,
            date: None,
        },
        KnowledgeEntry {
            issue_number: "140".to_string(),
            file_path: "dev-reports/review/2026-03-20-issue140-consistency-review-stage2.md"
                .to_string(),
            relation: KnowledgeRelation::HasReview,
            doc_subtype: DocSubtype::StageReview,
            date: None,
        },
    ];
    store.insert_knowledge_entries(&entries).unwrap();

    ci_dir
}

#[test]
fn issue_human_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "140"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Issue #140 関連ドキュメント:"));
    assert!(stdout.contains("設計:"));
    assert!(stdout.contains("issue-140-issue-cmd-design-policy.md"));
    assert!(stdout.contains("レビュー:"));
    assert!(stdout.contains("作業計画:"));
    assert!(stdout.contains("進捗レポート:"));
}

#[test]
fn issue_json_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "140", "--format", "json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["issue_number"], "140");
    assert!(parsed["documents"]["設計"].is_array());
    assert!(parsed["documents"]["レビュー"].is_array());
    assert!(parsed["documents"]["作業計画"].is_array());
    assert!(parsed["documents"]["進捗レポート"].is_array());
}

#[test]
fn issue_llm_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "140", "--format", "llm"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("# Issue #140 関連ドキュメント"));
    assert!(stdout.contains("## 設計"));
    assert!(stdout.contains("## レビュー"));
    assert!(stdout.contains("## 作業計画"));
    assert!(stdout.contains("## 進捗レポート"));
    assert!(stdout.contains("- dev-reports/design/issue-140-issue-cmd-design-policy.md"));
}

#[test]
fn issue_path_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "140", "--format", "path"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 6);
    // Verify all paths are present
    assert!(lines.contains(&"dev-reports/design/issue-140-issue-cmd-design-policy.md"));
    assert!(lines.contains(&"dev-reports/issue/140/work-plan.md"));
}

#[test]
fn issue_not_found() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "No documents found for issue #999",
        ));
}

#[test]
fn issue_progress_report_categorized() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "show", "140", "--format", "json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    // progress_report should be in 進捗レポート category
    let progress = parsed["documents"]["進捗レポート"]
        .as_array()
        .expect("進捗レポート should be array");
    assert_eq!(progress.len(), 1);
    assert!(
        progress[0]["file_path"]
            .as_str()
            .unwrap()
            .contains("progress-report.md")
    );
}

// --- issue list tests ---

#[test]
fn issue_list_human_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "list"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("Issue #140"));
    assert!(stdout.contains("Total: 1 issues"));
}

#[test]
fn issue_list_json_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "list", "--format", "json"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["number"], 140);
    assert!(parsed[0]["has_design"].as_bool().unwrap());
}

#[test]
fn issue_list_llm_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "list", "--format", "llm"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("| Issue | Docs | Label |"));
    assert!(stdout.contains("| #140 |"));
    assert!(stdout.contains("Total: 1 issues"));
}

#[test]
fn issue_list_path_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    setup_issue_test_data(tmp.path());

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "list", "--format", "path"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["140"]);
}

#[test]
fn issue_list_empty() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    // Create empty DB
    let ci_dir = tmp.path().join(".commandindex");
    std::fs::create_dir_all(&ci_dir).unwrap();
    let db_path = ci_dir.join("symbols.db");
    let store = commandindex::indexer::symbol_store::SymbolStore::open(&db_path).unwrap();
    store.create_tables().unwrap();

    let output = common::cmd()
        .current_dir(tmp.path())
        .args(["issue", "list"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);
    assert!(stdout.contains("No issues found."));
}

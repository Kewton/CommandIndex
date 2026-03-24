use std::collections::HashMap;
use std::path::Path;

use crate::embedding::store::EmbeddingSimilarityResult;
use crate::indexer::reader::SearchResult;

/// テストファイルのスコア係数（BM25スコアに乗算、1.0未満は減衰）
pub const TEST_FILE_WEIGHT: f32 = 0.3;

/// ドキュメント/レポートファイルのスコア係数
pub const DOC_FILE_WEIGHT: f32 = 0.5;

// ---------------------------------------------------------------------------
// File type classification
// ---------------------------------------------------------------------------

/// テストファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準（セパレータ付きパターンで誤検知を防止）:
/// - ファイル名が "_test." / ".test." / "_spec." / ".spec." パターンを含む
/// - ファイル名が "test_" で始まる（test_helper等）
/// - パスに "/tests/" または "/__tests__/" ディレクトリを含む
pub fn is_test_file(lower_path: &str) -> bool {
    let file_name = Path::new(lower_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // セパレータ付きパターンで誤検知防止（contest.rs, latest.rs等を除外）
    file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains("_spec.")
        || file_name.contains(".spec.")
        || file_name.starts_with("test_")
        || lower_path.contains("/tests/")
        || lower_path.starts_with("tests/")
        || lower_path.contains("/__tests__/")
        || lower_path.starts_with("__tests__/")
}

/// ドキュメント/レポートファイルかどうかをパスベースで判定（小文字化済みパスを受け取る）
///
/// 判定基準:
/// - パスに "dev-reports/" を含む（プロジェクト固有の判定基準）
/// - プロジェクトルート直下の定型ドキュメント（readme.md, changelog.md等）
///
/// 注意: src/配下の.mdファイルはナレッジとして有用なため、一律減衰しない
pub fn is_doc_file(lower_path: &str) -> bool {
    // プロジェクト固有のレポートディレクトリ
    if lower_path.contains("dev-reports/") {
        return true;
    }

    // .mdファイルのうち、docs/配下またはルート直下の定型ドキュメントのみ
    if lower_path.ends_with(".md") {
        let file_name = Path::new(lower_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        // ルート直下の定型ドキュメント（ディレクトリ区切りがない＝ルート直下）
        let is_root_doc = !lower_path.contains('/')
            && matches!(
                file_name,
                "readme.md" | "changelog.md" | "contributing.md" | "license.md" | "claude.md"
            );
        // docs/ ディレクトリ配下
        let is_docs_dir = lower_path.contains("/docs/") || lower_path.starts_with("docs/");
        return is_root_doc || is_docs_dir;
    }

    false
}

/// ファイルパスからスコア係数を判定する
///
/// - テストファイル: TEST_FILE_WEIGHT (0.3)
/// - ドキュメント/レポート: DOC_FILE_WEIGHT (0.5)
/// - ソースコードファイル: 1.0（調整なし）
pub fn file_type_weight_factor(path: &str) -> f32 {
    // パス区切り文字を正規化（Windows `\` → `/`）してOS非依存にする
    let lower = path.to_lowercase().replace('\\', "/");
    if is_test_file(&lower) {
        TEST_FILE_WEIGHT
    } else if is_doc_file(&lower) {
        DOC_FILE_WEIGHT
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// BM25 result aggregation
// ---------------------------------------------------------------------------

/// BM25検索結果をファイル単位に正規化・重複排除（リネーム: deduplicate_by_file → aggregate_by_file）
///
/// 同一ファイルの複数セクションのスコアのうち最大値を採用し、
/// スコア降順でソートして上位 `limit` 件を返す。
pub fn aggregate_by_file(results: Vec<SearchResult>, limit: usize) -> Vec<(String, f32)> {
    let mut file_scores: HashMap<String, f32> = HashMap::new();
    for result in results {
        let entry = file_scores.entry(result.path.clone()).or_insert(0.0);
        *entry = entry.max(result.score);
    }
    let mut sorted: Vec<(String, f32)> = file_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
}

/// BM25スコアにファイル種別ごとの係数を適用し、再ソート・truncateする
pub fn apply_file_type_weight(files: Vec<(String, f32)>, limit: usize) -> Vec<(String, f32)> {
    let mut weighted: Vec<(String, f32)> = files
        .into_iter()
        .map(|(path, score)| {
            let factor = file_type_weight_factor(&path);
            (path, score * factor)
        })
        .collect();
    weighted.sort_by(|a, b| b.1.total_cmp(&a.1));
    weighted.truncate(limit);
    weighted
}

// ---------------------------------------------------------------------------
// Embedding similarity aggregation
// ---------------------------------------------------------------------------

/// EmbeddingSimilarityResult をファイル単位に集約し、スコア降順で返す。
///
/// 同一ファイルの複数セクションの similarity のうち最大値を採用する。
pub fn aggregate_similarity_by_file(
    results: Vec<EmbeddingSimilarityResult>,
    limit: usize,
) -> Vec<(String, f32)> {
    let mut file_scores: HashMap<String, f32> = HashMap::new();
    for result in results {
        let entry = file_scores.entry(result.file_path.clone()).or_insert(0.0);
        *entry = entry.max(result.similarity);
    }
    let mut sorted: Vec<(String, f32)> = file_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(limit);
    sorted
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_test_file tests ---

    #[test]
    fn test_is_test_file_separator_patterns() {
        assert!(is_test_file("foo_test.ts"));
        assert!(is_test_file("foo.test.ts"));
        assert!(is_test_file("foo_spec.py"));
        assert!(is_test_file("foo.spec.tsx"));
    }

    #[test]
    fn test_is_test_file_test_prefix() {
        assert!(is_test_file("test_helper.rs"));
        assert!(is_test_file("test_utils.ts"));
    }

    #[test]
    fn test_is_test_file_tests_directory() {
        assert!(is_test_file("tests/unit/foo.rs"));
        assert!(is_test_file("__tests__/bar.ts"));
        assert!(is_test_file("src/__tests__/component.tsx"));
    }

    #[test]
    fn test_is_test_file_non_test_files() {
        assert!(!is_test_file("src/auth.rs"));
        assert!(!is_test_file("src/contest.rs"));
        assert!(!is_test_file("src/latest.rs"));
    }

    #[test]
    fn test_is_test_file_empty_path() {
        assert!(!is_test_file(""));
    }

    // --- is_doc_file tests ---

    #[test]
    fn test_is_doc_file_dev_reports() {
        assert!(is_doc_file("dev-reports/review.json"));
        assert!(is_doc_file("dev-reports/design/policy.md"));
    }

    #[test]
    fn test_is_doc_file_docs_directory() {
        assert!(is_doc_file("docs/guide.md"));
    }

    #[test]
    fn test_is_doc_file_root_docs() {
        assert!(is_doc_file("readme.md"));
        assert!(is_doc_file("changelog.md"));
        assert!(is_doc_file("contributing.md"));
        assert!(is_doc_file("license.md"));
        assert!(is_doc_file("claude.md"));
    }

    #[test]
    fn test_is_doc_file_ignores_nested_root_doc_names() {
        assert!(!is_doc_file("src/readme.md"));
        assert!(!is_doc_file("guide/changelog.md"));
    }

    #[test]
    fn test_is_doc_file_ignores_source_markdown() {
        assert!(!is_doc_file("src/notes.md"));
    }

    #[test]
    fn test_is_doc_file_ignores_source_files() {
        assert!(!is_doc_file("src/main.rs"));
    }

    // --- file_type_weight_factor tests ---

    #[test]
    fn test_file_type_weight_factor_values() {
        assert!(
            (file_type_weight_factor("src/foo_test.ts") - TEST_FILE_WEIGHT).abs() < f32::EPSILON
        );
        assert!(
            (file_type_weight_factor("dev-reports/review.json") - DOC_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
        assert!((file_type_weight_factor("src/main.rs") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_file_type_weight_factor_normalizes_windows_paths() {
        assert!(
            (file_type_weight_factor("tests\\unit\\foo.rs") - TEST_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
        assert!(
            (file_type_weight_factor("dev-reports\\review.json") - DOC_FILE_WEIGHT).abs()
                < f32::EPSILON
        );
    }

    // --- aggregate_by_file tests (renamed from deduplicate_by_file) ---

    #[test]
    fn test_aggregate_by_file_removes_duplicates_keeps_max_score() {
        let results = vec![
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 1.0,
            },
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 10,
                score: 2.0,
            },
            SearchResult {
                path: "b.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 0.5,
            },
        ];
        let deduped = aggregate_by_file(results, 10);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].0, "a.rs");
        assert!((deduped[0].1 - 2.0).abs() < f32::EPSILON);
        assert_eq!(deduped[1].0, "b.rs");
    }

    #[test]
    fn test_aggregate_by_file_respects_limit() {
        let results = vec![
            SearchResult {
                path: "a.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 3.0,
            },
            SearchResult {
                path: "b.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 2.0,
            },
            SearchResult {
                path: "c.rs".to_string(),
                heading: String::new(),
                body: String::new(),
                tags: String::new(),
                heading_level: 0,
                line_start: 0,
                score: 1.0,
            },
        ];
        let deduped = aggregate_by_file(results, 2);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].0, "a.rs");
        assert_eq!(deduped[1].0, "b.rs");
    }

    #[test]
    fn test_aggregate_by_file_empty_input() {
        let deduped = aggregate_by_file(vec![], 10);
        assert!(deduped.is_empty());
    }

    // --- apply_file_type_weight tests ---

    #[test]
    fn test_apply_file_type_weight_reorders() {
        let input = vec![
            ("src/foo_test.ts".to_string(), 2.0),
            ("src/main.rs".to_string(), 1.5),
        ];
        let result = apply_file_type_weight(input, 10);
        assert_eq!(result[0].0, "src/main.rs");
        assert_eq!(result[1].0, "src/foo_test.ts");
    }

    #[test]
    fn test_apply_file_type_weight_truncates() {
        let input = vec![
            ("a.rs".to_string(), 3.0),
            ("b.rs".to_string(), 2.0),
            ("c.rs".to_string(), 1.0),
        ];
        let result = apply_file_type_weight(input, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_apply_file_type_weight_empty_input() {
        let result = apply_file_type_weight(vec![], 10);
        assert!(result.is_empty());
    }

    // --- aggregate_similarity_by_file tests ---

    #[test]
    fn test_aggregate_similarity_by_file_basic() {
        let results = vec![
            EmbeddingSimilarityResult {
                file_path: "src/main.rs".to_string(),
                section_heading: "fn main".to_string(),
                similarity: 0.8,
            },
            EmbeddingSimilarityResult {
                file_path: "src/main.rs".to_string(),
                section_heading: "fn helper".to_string(),
                similarity: 0.9,
            },
            EmbeddingSimilarityResult {
                file_path: "src/lib.rs".to_string(),
                section_heading: "mod tests".to_string(),
                similarity: 0.7,
            },
            EmbeddingSimilarityResult {
                file_path: "src/utils.rs".to_string(),
                section_heading: "fn util".to_string(),
                similarity: 0.6,
            },
        ];
        let aggregated = aggregate_similarity_by_file(results, 10);
        assert_eq!(aggregated.len(), 3);
        // src/main.rs should be first with max similarity 0.9
        assert_eq!(aggregated[0].0, "src/main.rs");
        assert!((aggregated[0].1 - 0.9).abs() < f32::EPSILON);
        // src/lib.rs should be second with 0.7
        assert_eq!(aggregated[1].0, "src/lib.rs");
        assert!((aggregated[1].1 - 0.7).abs() < f32::EPSILON);
        // src/utils.rs should be third with 0.6
        assert_eq!(aggregated[2].0, "src/utils.rs");
        assert!((aggregated[2].1 - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_aggregate_similarity_by_file_empty() {
        let aggregated = aggregate_similarity_by_file(vec![], 10);
        assert!(aggregated.is_empty());
    }

    #[test]
    fn test_aggregate_similarity_by_file_respects_limit() {
        let results = vec![
            EmbeddingSimilarityResult {
                file_path: "a.rs".to_string(),
                section_heading: "s1".to_string(),
                similarity: 0.9,
            },
            EmbeddingSimilarityResult {
                file_path: "b.rs".to_string(),
                section_heading: "s2".to_string(),
                similarity: 0.8,
            },
            EmbeddingSimilarityResult {
                file_path: "c.rs".to_string(),
                section_heading: "s3".to_string(),
                similarity: 0.7,
            },
        ];
        let aggregated = aggregate_similarity_by_file(results, 2);
        assert_eq!(aggregated.len(), 2);
        assert_eq!(aggregated[0].0, "a.rs");
        assert_eq!(aggregated[1].0, "b.rs");
    }
}

use std::collections::HashMap;

use crate::indexer::reader::SearchResult;

/// RRF定数（業界標準値）
const RRF_K: f32 = 60.0;

/// ハイブリッド検索用Oversampling倍率
pub const HYBRID_OVERSAMPLING_FACTOR: usize = 3;

/// 複数の検索結果リストをRRF（Reciprocal Rank Fusion）で統合する。
/// キー: (path, heading) の2タプルで同一結果を識別。
/// スコア: 各リストについて sum(1 / (K + rank)) を計算（rank は 1-based）。
/// 片側のみヒット: 未出現側の寄与=0（標準RRF準拠）。
/// 同点時は(path, heading)辞書順で安定ソート。
pub fn rrf_merge_multiple(result_lists: &[Vec<SearchResult>], limit: usize) -> Vec<SearchResult> {
    let mut scores: HashMap<(String, String), (f32, SearchResult)> = HashMap::new();

    for list in result_lists {
        for (i, result) in list.iter().enumerate() {
            let rank = (i + 1) as f32;
            let rrf_score = 1.0 / (RRF_K + rank);
            let key = (result.path.clone(), result.heading.clone());
            let entry = scores.entry(key).or_insert_with(|| (0.0, result.clone()));
            entry.0 += rrf_score;
        }
    }

    let mut merged: Vec<(f32, SearchResult)> = scores
        .into_values()
        .map(|(score, mut result)| {
            result.score = score;
            (score, result)
        })
        .collect();

    merged.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.path.cmp(&b.1.path))
            .then_with(|| a.1.heading.cmp(&b.1.heading))
    });

    merged
        .into_iter()
        .take(limit)
        .map(|(_, result)| result)
        .collect()
}

/// RRFでBM25結果とセマンティック結果を統合する（後方互換ラッパー）。
/// 両方の入力はSearchResult型。scoreフィールドにRRFスコアを格納。
/// ランクは1-based。片側のみヒット: 未出現側の寄与=0（標準RRF準拠）。
/// 同点時は(path, heading)辞書順で安定ソート。
pub fn rrf_merge(
    bm25_results: &[SearchResult],
    semantic_results: &[SearchResult],
    limit: usize,
) -> Vec<SearchResult> {
    rrf_merge_multiple(&[bm25_results.to_vec(), semantic_results.to_vec()], limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(path: &str, heading: &str, score: f32) -> SearchResult {
        SearchResult {
            path: path.to_string(),
            heading: heading.to_string(),
            body: String::new(),
            tags: String::new(),
            heading_level: 1,
            line_start: 1,
            score,
        }
    }

    #[test]
    fn test_rrf_both_rankings() {
        // Doc appears in both BM25 (rank 1) and Semantic (rank 2)
        let bm25 = vec![make_result("a.md", "Title A", 10.0)];
        let semantic = vec![
            make_result("b.md", "Title B", 0.9),
            make_result("a.md", "Title A", 0.8),
        ];
        let results = rrf_merge(&bm25, &semantic, 10);

        // a.md should have score = 1/(60+1) + 1/(60+2) = 1/61 + 1/62
        let a_result = results.iter().find(|r| r.path == "a.md").unwrap();
        let expected = 1.0 / 61.0 + 1.0 / 62.0;
        assert!(
            (a_result.score - expected).abs() < 1e-6,
            "Expected {expected}, got {}",
            a_result.score
        );
    }

    #[test]
    fn test_rrf_bm25_only() {
        // Doc only in BM25 (rank 1) - semantic contribution = 0
        let bm25 = vec![make_result("a.md", "Title A", 10.0)];
        let semantic: Vec<SearchResult> = vec![];
        let results = rrf_merge(&bm25, &semantic, 10);

        assert_eq!(results.len(), 1);
        let expected = 1.0 / 61.0;
        assert!(
            (results[0].score - expected).abs() < 1e-6,
            "Expected {expected}, got {}",
            results[0].score
        );
    }

    #[test]
    fn test_rrf_semantic_only() {
        // Doc only in Semantic (rank 1) - BM25 contribution = 0
        let bm25: Vec<SearchResult> = vec![];
        let semantic = vec![make_result("a.md", "Title A", 0.95)];
        let results = rrf_merge(&bm25, &semantic, 10);

        assert_eq!(results.len(), 1);
        let expected = 1.0 / 61.0;
        assert!(
            (results[0].score - expected).abs() < 1e-6,
            "Expected {expected}, got {}",
            results[0].score
        );
    }

    #[test]
    fn test_rrf_empty_results() {
        let bm25: Vec<SearchResult> = vec![];
        let semantic: Vec<SearchResult> = vec![];
        let results = rrf_merge(&bm25, &semantic, 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_stable_sort() {
        // Two docs with the same score (both only in BM25 at rank 1 and 2 respectively,
        // but we make them appear such that they get equal scores)
        // Use: a.md in BM25 rank 1, b.md in semantic rank 1 -> both get 1/61
        let bm25 = vec![make_result("b.md", "Title B", 10.0)];
        let semantic = vec![make_result("a.md", "Title A", 0.95)];
        let results = rrf_merge(&bm25, &semantic, 10);

        // Both have score 1/61 - should be sorted by (path, heading) ascending
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a.md");
        assert_eq!(results[1].path, "b.md");
    }

    #[test]
    fn test_rrf_limit() {
        let bm25 = vec![
            make_result("a.md", "A", 10.0),
            make_result("b.md", "B", 9.0),
            make_result("c.md", "C", 8.0),
        ];
        let semantic: Vec<SearchResult> = vec![];
        let results = rrf_merge(&bm25, &semantic, 2);
        assert_eq!(results.len(), 2);
    }

    // --- rrf_merge_multiple tests ---

    #[test]
    fn test_rrf_multiple_three_lists() {
        let list1 = vec![make_result("a.md", "A", 1.0)];
        let list2 = vec![make_result("b.md", "B", 1.0)];
        let list3 = vec![make_result("c.md", "C", 1.0)];
        let results = rrf_merge_multiple(&[list1, list2, list3], 10);

        // All three should appear with equal score 1/61
        assert_eq!(results.len(), 3);
        let expected = 1.0 / 61.0;
        for r in &results {
            assert!(
                (r.score - expected).abs() < 1e-6,
                "Expected {expected}, got {}",
                r.score
            );
        }
        // Tied scores -> sorted by (path, heading) ascending
        assert_eq!(results[0].path, "a.md");
        assert_eq!(results[1].path, "b.md");
        assert_eq!(results[2].path, "c.md");
    }

    #[test]
    fn test_rrf_multiple_empty_lists_mixed() {
        let list1 = vec![make_result("a.md", "A", 1.0)];
        let list2: Vec<SearchResult> = vec![];
        let list3 = vec![make_result("b.md", "B", 1.0)];
        let results = rrf_merge_multiple(&[list1, list2, list3], 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_rrf_multiple_all_same_result() {
        // Same doc in all 3 lists at rank 1 -> score = 3 * 1/61
        let list1 = vec![make_result("a.md", "A", 1.0)];
        let list2 = vec![make_result("a.md", "A", 2.0)];
        let list3 = vec![make_result("a.md", "A", 3.0)];
        let results = rrf_merge_multiple(&[list1, list2, list3], 10);

        assert_eq!(results.len(), 1);
        let expected = 3.0 / 61.0;
        assert!(
            (results[0].score - expected).abs() < 1e-6,
            "Expected {expected}, got {}",
            results[0].score
        );
    }

    #[test]
    fn test_rrf_multiple_single_list() {
        let list = vec![make_result("a.md", "A", 1.0), make_result("b.md", "B", 2.0)];
        let results = rrf_merge_multiple(&[list], 10);

        assert_eq!(results.len(), 2);
        // rank 1 -> 1/61, rank 2 -> 1/62
        assert_eq!(results[0].path, "a.md");
        let expected_a = 1.0 / 61.0;
        assert!(
            (results[0].score - expected_a).abs() < 1e-6,
            "Expected {expected_a}, got {}",
            results[0].score
        );
        let expected_b = 1.0 / 62.0;
        assert!(
            (results[1].score - expected_b).abs() < 1e-6,
            "Expected {expected_b}, got {}",
            results[1].score
        );
    }

    #[test]
    fn test_rrf_multiple_zero_lists() {
        let results = rrf_merge_multiple(&[], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_rrf_multiple_limit_smaller_than_results() {
        let list1 = vec![make_result("a.md", "A", 1.0), make_result("b.md", "B", 2.0)];
        let list2 = vec![make_result("c.md", "C", 1.0), make_result("d.md", "D", 2.0)];
        let results = rrf_merge_multiple(&[list1, list2], 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_rrf_multiple_score_accumulation_across_lists() {
        // a.md appears in list1 rank 1 and list2 rank 2
        // b.md appears in list1 rank 2 and list2 rank 1
        // Both should have equal scores: 1/61 + 1/62
        let list1 = vec![make_result("a.md", "A", 1.0), make_result("b.md", "B", 2.0)];
        let list2 = vec![make_result("b.md", "B", 1.0), make_result("a.md", "A", 2.0)];
        let results = rrf_merge_multiple(&[list1, list2], 10);

        assert_eq!(results.len(), 2);
        let expected = 1.0 / 61.0 + 1.0 / 62.0;
        for r in &results {
            assert!(
                (r.score - expected).abs() < 1e-6,
                "Expected {expected}, got {} for {}",
                r.score,
                r.path
            );
        }
        // Tied -> sorted by path
        assert_eq!(results[0].path, "a.md");
        assert_eq!(results[1].path, "b.md");
    }
}

use std::collections::HashMap;

use crate::indexer::reader::SearchResult;

/// RRF定数（業界標準値）
const RRF_K: f32 = 60.0;

/// ハイブリッド検索用Oversampling倍率
pub const HYBRID_OVERSAMPLING_FACTOR: usize = 3;

/// RRFでBM25結果とセマンティック結果を統合する。
/// 両方の入力はSearchResult型。scoreフィールドにRRFスコアを格納。
/// ランクは1-based。片側のみヒット: 未出現側の寄与=0（標準RRF準拠）。
/// 同点時は(path, heading)辞書順で安定ソート。
pub fn rrf_merge(
    bm25_results: &[SearchResult],
    semantic_results: &[SearchResult],
    limit: usize,
) -> Vec<SearchResult> {
    // (path, heading) をキーとして RRF スコアを蓄積
    // 最良のSearchResultも保持する
    let mut scores: HashMap<(String, String), (f32, SearchResult)> = HashMap::new();

    // BM25側: rank は 1-based
    for (i, result) in bm25_results.iter().enumerate() {
        let rank = (i + 1) as f32;
        let rrf_score = 1.0 / (RRF_K + rank);
        let key = (result.path.clone(), result.heading.clone());
        let entry = scores.entry(key).or_insert_with(|| (0.0, result.clone()));
        entry.0 += rrf_score;
    }

    // Semantic側: rank は 1-based
    for (i, result) in semantic_results.iter().enumerate() {
        let rank = (i + 1) as f32;
        let rrf_score = 1.0 / (RRF_K + rank);
        let key = (result.path.clone(), result.heading.clone());
        let entry = scores.entry(key).or_insert_with(|| (0.0, result.clone()));
        entry.0 += rrf_score;
    }

    // スコア降順、同点時は (path, heading) 辞書順でソート
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
}

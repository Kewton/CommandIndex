/// トークン数から文字数予算に変換（1トークン ≈ 4文字）
pub(crate) fn tokens_to_char_budget(tokens: usize) -> usize {
    tokens.saturating_mul(4)
}

/// snippet を先頭と末尾に比率で切り詰める際の定数
const HEAD_RATIO: usize = 3;
const TOTAL_PARTS: usize = 5;
/// 省略マーカー "..." の文字数
const ELLIPSIS_LEN: usize = 3;

/// 文字数予算に収まるようsnippetを動的に切り詰める
/// 先頭と末尾を保持し、中間を省略する戦略
/// budget_chars: 文字数ベースの予算（トークンではない）
/// 戻り値が空文字列の場合、呼び出し側で snippet = None に正規化する
pub(crate) fn truncate_snippet_for_char_budget(snippet: &str, budget_chars: usize) -> String {
    let chars: Vec<char> = snippet.chars().collect();
    if chars.len() <= budget_chars {
        return snippet.to_string();
    }
    if budget_chars == 0 {
        return String::new();
    }
    // 省略マーカー + 最低1文字ずつ = 最低5文字必要
    // それ未満なら先頭のみで切り詰め（省略マーカーなし）
    if budget_chars < ELLIPSIS_LEN + 2 {
        return chars[..budget_chars].iter().collect();
    }
    let content_budget = budget_chars - ELLIPSIS_LEN;
    let head_chars = (content_budget * HEAD_RATIO) / TOTAL_PARTS;
    let tail_chars = content_budget - head_chars;
    let head: String = chars[..head_chars].iter().collect();
    let tail: String = chars[chars.len() - tail_chars..].iter().collect();
    format!("{head}...{tail}")
}

/// 結果リストにトークン予算を適用し、予算内に収まるよう打ち切る
/// - items: スコア順にソート済みの結果リスト
/// - max_tokens: トークン予算（> 0 が前提。0の場合は空Vecを早期リターン）
/// - estimate_fn: 各アイテムのトークン推定関数（クロージャで注入、型依存を排除）
/// - 最初のアイテムは予算超過でも必ず含める
pub(crate) fn apply_token_budget<T, F>(items: Vec<T>, max_tokens: usize, estimate_fn: F) -> Vec<T>
where
    F: Fn(&T) -> usize,
{
    if max_tokens == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut used_tokens: usize = 0;
    for (i, item) in items.into_iter().enumerate() {
        let item_tokens = estimate_fn(&item);
        if i > 0 && used_tokens.saturating_add(item_tokens) > max_tokens {
            break;
        }
        used_tokens = used_tokens.saturating_add(item_tokens);
        result.push(item);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- tokens_to_char_budget tests ----

    #[test]
    fn test_tokens_to_char_budget() {
        assert_eq!(tokens_to_char_budget(0), 0);
        assert_eq!(tokens_to_char_budget(1), 4);
        assert_eq!(tokens_to_char_budget(10), 40);
        assert_eq!(tokens_to_char_budget(100), 400);
    }

    // ---- truncate_snippet_for_char_budget tests ----

    #[test]
    fn test_truncate_within_budget() {
        let s = "hello";
        assert_eq!(truncate_snippet_for_char_budget(s, 10), "hello");
        assert_eq!(truncate_snippet_for_char_budget(s, 5), "hello");
    }

    #[test]
    fn test_truncate_exceeds_budget() {
        // 10 chars, budget 8
        let s = "0123456789";
        let result = truncate_snippet_for_char_budget(s, 8);
        // content_budget = 8 - 3 = 5
        // head = 5*3/5 = 3 => "012"
        // tail = 5-3 = 2 => "89"
        assert_eq!(result, "012...89");
        assert_eq!(result.chars().count(), 8);
    }

    #[test]
    fn test_truncate_zero_budget() {
        assert_eq!(truncate_snippet_for_char_budget("hello", 0), "");
    }

    #[test]
    fn test_truncate_budget_1() {
        // budget < 5, so head-only
        assert_eq!(truncate_snippet_for_char_budget("hello", 1), "h");
    }

    #[test]
    fn test_truncate_budget_2() {
        assert_eq!(truncate_snippet_for_char_budget("hello", 2), "he");
    }

    #[test]
    fn test_truncate_budget_3() {
        assert_eq!(truncate_snippet_for_char_budget("hello", 3), "hel");
    }

    #[test]
    fn test_truncate_budget_4() {
        assert_eq!(truncate_snippet_for_char_budget("hello", 4), "hell");
    }

    #[test]
    fn test_truncate_budget_5() {
        // "hello" is exactly 5 chars = budget, fits within budget
        assert_eq!(truncate_snippet_for_char_budget("hello", 5), "hello");
        // 6 chars with budget 5: uses ellipsis mode
        // content_budget = 5 - 3 = 2
        // head = 2*3/5 = 1 => "h"
        // tail = 2-1 = 1 => "!"
        assert_eq!(truncate_snippet_for_char_budget("hello!", 5), "h...!");
    }

    #[test]
    fn test_truncate_short_text() {
        // Text shorter than budget
        assert_eq!(truncate_snippet_for_char_budget("ab", 10), "ab");
    }

    #[test]
    fn test_truncate_japanese() {
        let s = "あいうえおかきくけこ"; // 10 chars
        let result = truncate_snippet_for_char_budget(s, 8);
        assert_eq!(result.chars().count(), 8);
    }

    // ---- apply_token_budget tests ----

    #[test]
    fn test_apply_token_budget_within_budget() {
        let items = vec!["aaa", "bbb", "ccc"];
        // Each item = 1 token (3 chars / 4 = 0, max 1)
        let result = apply_token_budget(items, 100, |s| {
            let count = s.chars().count();
            if count == 0 { 0 } else { (count / 4).max(1) }
        });
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_apply_token_budget_exceeds_budget() {
        // Each item is 10 tokens
        let items = vec!["a", "b", "c", "d"];
        let result = apply_token_budget(items, 25, |_| 10);
        // First item: 10 tokens (always included)
        // Second item: 10+10=20 <= 25, included
        // Third item: 20+10=30 > 25, break
        assert_eq!(result.len(), 2);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    fn test_apply_token_budget_first_item_always_included() {
        // First item exceeds budget but should be included
        let items = vec!["huge_item", "small"];
        let result = apply_token_budget(items, 1, |_| 100);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "huge_item");
    }

    #[test]
    fn test_apply_token_budget_max_tokens_zero() {
        let items = vec!["a", "b"];
        let result = apply_token_budget(items, 0, |_| 1);
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_token_budget_empty_items() {
        let items: Vec<&str> = Vec::new();
        let result = apply_token_budget(items, 100, |_| 1);
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_token_budget_all_items_fit() {
        let items = vec!["a", "b", "c"];
        let result = apply_token_budget(items, 100, |_| 1);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_apply_token_budget_max_tokens_1() {
        // max_tokens=1, each item costs 1 token
        let items = vec!["a", "b", "c"];
        let result = apply_token_budget(items, 1, |_| 1);
        // First item: 1 token, included
        // Second item: 1+1=2 > 1, break
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "a");
    }

    #[test]
    fn test_apply_token_budget_saturating_add() {
        // Test with large token values to ensure no overflow
        let items = vec!["a", "b"];
        let result = apply_token_budget(items, usize::MAX, |_| usize::MAX / 2);
        // First item: usize::MAX/2, included
        // Second item: saturating_add would overflow, so it saturates
        // usize::MAX/2 + usize::MAX/2 = usize::MAX - 1 (if even) which <= usize::MAX
        // Actually depends on arch, but it should not panic
        assert!(!result.is_empty());
    }
}

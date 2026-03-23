pub const CONTEXT_AFTER_HELP: &str = "\
When to use:
  Prepare context for LLM consumption (code review, refactoring, etc.).
  Generates a JSON context pack with file content and relationships.

Examples:
  commandindexdev context src/main.rs
  commandindexdev context src/auth.rs --max-files 10
  commandindexdev context src/a.rs src/b.rs --max-tokens 8000
  (Limits total estimated tokens. Estimation: approx. 1 token per 4 chars)";

use std::collections::HashMap;
use std::path::Path;

use crate::cli::search::SearchError;
use crate::indexer::reader::IndexReaderWrapper;
use crate::indexer::symbol_store::SymbolStore;
use crate::output::estimate_tokens;
use crate::output::token_budget::{tokens_to_char_budget, truncate_snippet_for_char_budget};
use crate::output::{
    ContextEntry, ContextPack, ContextSummary, RelatedSearchResult, RelationType,
    strip_control_chars, truncate_body,
};
use crate::search::related::RelatedSearchEngine;

/// Context Pack 生成のメインエントリポイント
pub fn run_context(
    files: &[String],
    max_files: usize,
    max_tokens: Option<usize>,
    commandindex_dir: &Path,
) -> Result<(), SearchError> {
    // 入力検証
    super::validate_file_paths(files, 100)?;

    // インデックスオープン
    let tantivy_dir = crate::indexer::index_dir(commandindex_dir);
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(commandindex_dir);
    if !db_path.exists() {
        return Err(SearchError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    // 関連ファイル収集
    let merged = collect_related_context(files, &reader, &store)?;

    // ContextPack 構築
    let pack = build_context_pack(files, &merged, max_files, max_tokens, &reader, &store)?;

    // 出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    crate::output::context_pack::format_context_pack(&pack, &mut handle)?;

    Ok(())
}

/// 各ファイルの関連検索結果を収集・マージする。
/// Caller must validate file_paths with validate_file_paths before calling.
pub(crate) fn collect_related_context(
    files: &[String],
    reader: &IndexReaderWrapper,
    store: &SymbolStore,
) -> Result<Vec<RelatedSearchResult>, SearchError> {
    let engine = RelatedSearchEngine::new(reader, store);

    let mut results_per_file = Vec::new();
    for file in files {
        match engine.find_related(file, 1000) {
            Ok(results) => results_per_file.push(results),
            Err(crate::search::related::RelatedSearchError::FileNotFound(_))
            | Err(crate::search::related::RelatedSearchError::FileNotIndexed(_)) => {
                // ファイルが見つからない/インデックスされていない場合はスキップ
                results_per_file.push(Vec::new());
            }
            Err(e) => return Err(SearchError::RelatedSearch(e)),
        }
    }

    Ok(merge_related_results(results_per_file, files))
}

/// 外部から呼び出し可能な関連ファイル収集・マージ（search --related-stdin 用）
pub(crate) fn collect_and_merge_related(
    engine: &RelatedSearchEngine,
    files: &[String],
    limit: usize,
) -> Result<Vec<RelatedSearchResult>, SearchError> {
    let mut results_per_file = Vec::new();
    for file in files {
        match engine.find_related(file, 1000) {
            Ok(results) => results_per_file.push(results),
            Err(crate::search::related::RelatedSearchError::FileNotFound(_))
            | Err(crate::search::related::RelatedSearchError::FileNotIndexed(_)) => {
                results_per_file.push(Vec::new());
            }
            Err(e) => return Err(SearchError::RelatedSearch(e)),
        }
    }

    let mut merged = merge_related_results(results_per_file, files);
    merged.truncate(limit);
    Ok(merged)
}

/// Merges related search results from multiple files using union + max score strategy.
pub(crate) fn merge_related_results(
    results_per_file: Vec<Vec<RelatedSearchResult>>,
    target_files: &[String],
) -> Vec<RelatedSearchResult> {
    let mut merged: HashMap<String, (f32, Vec<RelationType>)> = HashMap::new();

    // target_files を正規化（比較用）
    let normalized_targets: Vec<String> = target_files
        .iter()
        .filter_map(|f| crate::search::related::normalize_path(f).ok())
        .collect();

    for results in results_per_file {
        for result in results {
            let entry = merged
                .entry(result.file_path.clone())
                .or_insert((0.0, Vec::new()));
            // スコア最大値を採用
            if result.score > entry.0 {
                entry.0 = result.score;
            }
            // relation_types をマージ（TagMatch は matched_tags を union）
            for rt in result.relation_types {
                if let RelationType::TagMatch {
                    matched_tags: new_tags,
                } = &rt
                {
                    // 既存の TagMatch があれば matched_tags を union
                    let existing_tag_match = entry
                        .1
                        .iter_mut()
                        .find(|existing| matches!(existing, RelationType::TagMatch { .. }));
                    if let Some(RelationType::TagMatch { matched_tags }) = existing_tag_match {
                        for tag in new_tags {
                            if !matched_tags.contains(tag) {
                                matched_tags.push(tag.clone());
                            }
                        }
                    } else {
                        entry.1.push(rt);
                    }
                } else {
                    let already_exists = entry.1.iter().any(|existing| {
                        std::mem::discriminant(existing) == std::mem::discriminant(&rt)
                    });
                    if !already_exists {
                        entry.1.push(rt);
                    }
                }
            }
        }
    }

    // target_files を除外
    for target in &normalized_targets {
        merged.remove(target);
    }

    // スコア降順ソート
    let mut results: Vec<RelatedSearchResult> = merged
        .into_iter()
        .map(|(path, (score, relation_types))| RelatedSearchResult {
            file_path: path,
            score,
            relation_types,
            snippet: None,
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

/// ContextPack を構築する（エンリッチ + 制限適用）
fn build_context_pack(
    target_files: &[String],
    merged: &[RelatedSearchResult],
    max_files: usize,
    max_tokens: Option<usize>,
    reader: &IndexReaderWrapper,
    store: &SymbolStore,
) -> Result<ContextPack, SearchError> {
    let total_related = merged.len();

    // max_files でトリム
    let limited = &merged[..merged.len().min(max_files)];

    // エンリッチ + max_tokens 適用
    let mut entries = Vec::new();
    let mut token_total: usize = 0;

    for result in limited {
        let mut entry = enrich_entry(
            &result.file_path,
            result.score,
            &result.relation_types,
            reader,
            store,
        );

        if let Some(max_tok) = max_tokens {
            let meta_tokens = estimate_entry_meta_tokens(&entry);

            // メタデータだけで残予算超過 → スキップ（最初のエントリのみ例外）
            if token_total + meta_tokens > max_tok && !entries.is_empty() {
                continue;
            }

            // snippet動的縮約
            let remaining = max_tok.saturating_sub(token_total + meta_tokens);
            let snippet_budget = tokens_to_char_budget(remaining);
            if let Some(s) = &entry.snippet {
                let truncated = truncate_snippet_for_char_budget(s, snippet_budget);
                // 空文字列はNoneに正規化
                entry.snippet = if truncated.is_empty() {
                    None
                } else {
                    Some(truncated)
                };
            }

            token_total += estimate_entry_tokens(&entry);
        }

        entries.push(entry);
    }

    let included = entries.len();
    let estimated_tokens = if max_tokens.is_some() {
        token_total
    } else {
        entries.iter().map(estimate_entry_tokens).sum()
    };

    Ok(ContextPack {
        target_files: target_files.to_vec(),
        context: entries,
        summary: ContextSummary {
            total_related,
            included,
            estimated_tokens,
        },
    })
}

/// エントリをエンリッチする（スニペット・見出し・シンボル付加）
fn enrich_entry(
    path: &str,
    score: f32,
    relation_types: &[RelationType],
    reader: &IndexReaderWrapper,
    store: &SymbolStore,
) -> ContextEntry {
    let relation = relation_to_string(relation_types);

    // 関連タイプに基づいてデータを取得
    let has_markdown_link = relation_types
        .iter()
        .any(|r| matches!(r, RelationType::MarkdownLink));
    let has_import = relation_types
        .iter()
        .any(|r| matches!(r, RelationType::ImportDependency));
    let has_tag_match = relation_types
        .iter()
        .any(|r| matches!(r, RelationType::TagMatch { .. }));

    let mut heading = None;
    let mut snippet = None;
    let mut symbols = None;

    // heading と snippet の取得
    if has_markdown_link || has_tag_match {
        if let Ok(docs) = reader.search_by_exact_path(path)
            && let Some(first) = docs.first()
        {
            if !first.heading.is_empty() {
                heading = Some(first.heading.clone());
            }
            if !first.body.is_empty() {
                // 事前に500文字/10行に切り詰める。これは max_tokens 未指定時に
                // 出力が巨大にならないための安全策。max_tokens 指定時は
                // build_context_pack 内の truncate_snippet_for_char_budget が
                // さらに予算内に縮約するため、機能上の問題はない。
                let truncated = truncate_body(&first.body, 10, 500);
                let cleaned = strip_control_chars(&truncated);
                if !cleaned.is_empty() {
                    snippet = Some(cleaned);
                }
            }
        }
    } else if let Ok(docs) = reader.search_by_exact_path(path)
        && let Some(first) = docs.first()
        && !first.heading.is_empty()
    {
        // PathSimilarity, DirectoryProximity: heading のみ
        heading = Some(first.heading.clone());
    }

    // ImportDependency: symbols を取得
    if has_import && let Ok(imports) = store.find_imports_by_source(path) {
        let mut all_names: Vec<String> = Vec::new();
        for imp in &imports {
            if let Some(names) = &imp.imported_names {
                for name in names.split(", ") {
                    let name = name.trim();
                    if !name.is_empty() && !all_names.contains(&name.to_string()) {
                        all_names.push(name.to_string());
                    }
                }
            }
        }
        // Also check reverse: files that import this path
        if let Ok(reverse_imports) = store.find_imports_by_target(path) {
            for imp in &reverse_imports {
                if let Some(names) = &imp.imported_names {
                    for name in names.split(", ") {
                        let name = name.trim();
                        if !name.is_empty() && !all_names.contains(&name.to_string()) {
                            all_names.push(name.to_string());
                        }
                    }
                }
            }
        }
        if !all_names.is_empty() {
            symbols = Some(all_names);
        }
    }

    ContextEntry {
        path: path.to_string(),
        relation,
        score,
        heading,
        snippet,
        symbols,
    }
}

/// RelationType を文字列に変換する（優先度順）
fn relation_to_string(relation_types: &[RelationType]) -> String {
    // 優先度: MarkdownLink > ImportDependency > TagMatch > PathSimilarity > DirectoryProximity
    for rt in relation_types {
        if matches!(rt, RelationType::MarkdownLink) {
            return "linked".to_string();
        }
    }
    for rt in relation_types {
        if matches!(rt, RelationType::ImportDependency) {
            return "import_dependency".to_string();
        }
    }
    for rt in relation_types {
        if matches!(rt, RelationType::TagMatch { .. }) {
            return "tag_match".to_string();
        }
    }
    for rt in relation_types {
        if matches!(rt, RelationType::PathSimilarity) {
            return "path_similarity".to_string();
        }
    }
    for rt in relation_types {
        if matches!(rt, RelationType::DirectoryProximity) {
            return "directory_proximity".to_string();
        }
    }
    "unknown".to_string()
}

/// ContextEntryのメタデータ部分（snippet以外）の推定トークン数を算出
fn estimate_entry_meta_tokens(entry: &ContextEntry) -> usize {
    let mut total = 0;
    total += estimate_tokens(&entry.path);
    total += estimate_tokens(&entry.relation);
    total += 1; // score は固定1トークン
    if let Some(h) = &entry.heading {
        total += estimate_tokens(h);
    }
    if let Some(syms) = &entry.symbols {
        for sym in syms {
            total += estimate_tokens(sym);
        }
    }
    total
}

/// ContextEntry全体の推定トークン数を算出（メタデータ + snippet）
fn estimate_entry_tokens(entry: &ContextEntry) -> usize {
    let meta = estimate_entry_meta_tokens(entry);
    let snippet = entry
        .snippet
        .as_ref()
        .map(|s| estimate_tokens(s))
        .unwrap_or(0);
    meta + snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- estimate_tokens tests ----

    #[test]
    fn test_estimate_tokens_ascii() {
        // "hello" = 5 chars => 5/4 = 1
        assert_eq!(estimate_tokens("hello"), 1);
        // 8 chars => 8/4 = 2
        assert_eq!(estimate_tokens("12345678"), 2);
    }

    #[test]
    fn test_estimate_tokens_japanese() {
        // "あいうえお" = 5 chars => 5/4 = 1 (previously would be 15/4=3 with bytes)
        assert_eq!(estimate_tokens("あいうえお"), 1);
        // "あいうえおかきくけこ" = 10 chars => 10/4 = 2
        assert_eq!(estimate_tokens("あいうえおかきくけこ"), 2);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        // 1 char => min 1 token
        assert_eq!(estimate_tokens("a"), 1);
        // 3 chars => 3/4 = 0, but max(1) => 1
        assert_eq!(estimate_tokens("abc"), 1);
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        // "hello世界" = 7 chars => 7/4 = 1
        assert_eq!(estimate_tokens("hello世界"), 1);
        // "hello世界abc" = 10 chars => 10/4 = 2
        assert_eq!(estimate_tokens("hello世界abc"), 2);
    }

    // ---- estimate_entry_meta_tokens tests ----

    fn make_entry(
        path: &str,
        relation: &str,
        heading: Option<&str>,
        snippet: Option<&str>,
        symbols: Option<Vec<&str>>,
    ) -> ContextEntry {
        ContextEntry {
            path: path.to_string(),
            relation: relation.to_string(),
            score: 1.0,
            heading: heading.map(|s| s.to_string()),
            snippet: snippet.map(|s| s.to_string()),
            symbols: symbols.map(|v| v.into_iter().map(|s| s.to_string()).collect()),
        }
    }

    #[test]
    fn test_estimate_entry_meta_tokens_all_fields() {
        let entry = make_entry(
            "src/main.rs",
            "import_dependency",
            Some("Main Module"),
            Some("fn main() {}"),
            Some(vec!["func_a", "func_b"]),
        );
        let meta = estimate_entry_meta_tokens(&entry);
        // path: "src/main.rs" = 11 chars => 2
        // relation: "import_dependency" = 17 chars => 4
        // score: 1
        // heading: "Main Module" = 11 chars => 2
        // symbols: "func_a"=6=>1, "func_b"=6=>1 => 2
        assert_eq!(meta, 2 + 4 + 1 + 2 + 2);
    }

    #[test]
    fn test_estimate_entry_meta_tokens_minimal() {
        let entry = make_entry("a.rs", "linked", None, None, None);
        let meta = estimate_entry_meta_tokens(&entry);
        // path: "a.rs" = 4 chars => 1
        // relation: "linked" = 6 chars => 1
        // score: 1
        assert_eq!(meta, 1 + 1 + 1);
    }

    // ---- estimate_entry_tokens tests ----

    #[test]
    fn test_estimate_entry_tokens_with_snippet() {
        let entry = make_entry("a.rs", "linked", None, Some("some snippet text here"), None);
        let meta = estimate_entry_meta_tokens(&entry);
        let snippet_tokens = estimate_tokens("some snippet text here");
        assert_eq!(estimate_entry_tokens(&entry), meta + snippet_tokens);
    }

    #[test]
    fn test_estimate_entry_tokens_without_snippet() {
        let entry = make_entry("a.rs", "linked", None, None, None);
        let meta = estimate_entry_meta_tokens(&entry);
        assert_eq!(estimate_entry_tokens(&entry), meta);
    }
}

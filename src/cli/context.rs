use std::collections::HashMap;
use std::path::Path;

use crate::cli::search::SearchError;
use crate::indexer::reader::IndexReaderWrapper;
use crate::indexer::symbol_store::SymbolStore;
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
) -> Result<(), SearchError> {
    // 入力検証
    if files.is_empty() {
        return Err(SearchError::InvalidArgument(
            "At least one file is required".to_string(),
        ));
    }
    if files.len() > 100 {
        return Err(SearchError::InvalidArgument(
            "Too many files specified (max 100)".to_string(),
        ));
    }
    for f in files {
        if f.is_empty() {
            return Err(SearchError::InvalidArgument(
                "File path cannot be empty".to_string(),
            ));
        }
        if f.len() > 1024 {
            return Err(SearchError::InvalidArgument(
                "File path too long (max 1024 characters)".to_string(),
            ));
        }
        if f.contains("..") {
            return Err(SearchError::InvalidArgument(format!(
                "File path must not contain '..': {f}"
            )));
        }
        if f.starts_with('/') || f.starts_with('\\') {
            return Err(SearchError::InvalidArgument(format!(
                "File path must be relative: {f}"
            )));
        }
        if f.contains('\\') {
            return Err(SearchError::InvalidArgument(format!(
                "File path must not contain backslashes: {f}"
            )));
        }
    }

    // インデックスオープン
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(SearchError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(Path::new("."));
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

/// 各ファイルの関連検索結果を収集・マージする
fn collect_related_context(
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

/// 複数ファイルの関連検索結果をunionマージする
fn merge_related_results(
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
        let entry = enrich_entry(
            &result.file_path,
            result.score,
            &result.relation_types,
            reader,
            store,
        );

        if let Some(max_tok) = max_tokens {
            let entry_tokens = entry
                .snippet
                .as_ref()
                .map(|s| estimate_tokens(s))
                .unwrap_or(0);
            if token_total + entry_tokens > max_tok && !entries.is_empty() {
                break;
            }
            token_total += entry_tokens;
        }

        entries.push(entry);
    }

    let estimated_tokens = if max_tokens.is_some() {
        token_total
    } else {
        entries
            .iter()
            .map(|e| e.snippet.as_ref().map(|s| estimate_tokens(s)).unwrap_or(0))
            .sum()
    };

    Ok(ContextPack {
        target_files: target_files.to_vec(),
        context: entries,
        summary: ContextSummary {
            total_related,
            included: 0, // 一時的に0、後で更新
            estimated_tokens,
        },
    })
    .map(|mut pack| {
        pack.summary.included = pack.context.len();
        pack
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

/// トークン数を概算する（バイト数 / 4）
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

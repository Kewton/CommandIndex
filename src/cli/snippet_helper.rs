use crate::indexer::reader::IndexReaderWrapper;
use crate::output::{SnippetConfig, strip_control_chars, truncate_body};

/// スニペット取得オプション
#[derive(Debug, Clone, Default)]
pub struct SnippetOptions {
    pub enabled: bool,
    pub config: SnippetConfig,
}

/// ファイルパスから tantivy インデックスのスニペットを取得する。
/// 取得失敗時は空文字列を返す（エラーで停止しない）。
pub(crate) fn fetch_snippet(
    reader: &IndexReaderWrapper,
    path: &str,
    config: SnippetConfig,
) -> String {
    match reader.search_by_exact_path(path) {
        Ok(docs) => {
            if let Some(first) = docs.first()
                && !first.body.is_empty()
            {
                let lines = if config.lines == 0 {
                    usize::MAX
                } else {
                    config.lines
                };
                let chars = if config.chars == 0 {
                    usize::MAX
                } else {
                    config.chars
                };
                let truncated = truncate_body(&first.body, lines, chars);
                let cleaned = strip_control_chars(&truncated);
                if !cleaned.is_empty() {
                    return cleaned;
                }
            }
            String::new()
        }
        Err(_) => String::new(),
    }
}

/// ImpactFileResult のスニペットを一括付与する。
/// with_snippet=false または format=Path の場合は何もしない。
pub(crate) fn enrich_impact_with_snippets(
    results: &mut [crate::output::ImpactFileResult],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for file in results.iter_mut() {
        file.snippet = Some(fetch_snippet(
            reader,
            &file.file_path,
            snippet_options.config,
        ));
    }
}

/// RelatedSearchResult のスニペットを一括付与する。
pub(crate) fn enrich_related_with_snippets(
    results: &mut [crate::output::RelatedSearchResult],
    reader: &IndexReaderWrapper,
    snippet_options: &SnippetOptions,
    format: crate::output::OutputFormat,
) {
    if !snippet_options.enabled || matches!(format, crate::output::OutputFormat::Path) {
        return;
    }
    for result in results.iter_mut() {
        result.snippet = Some(fetch_snippet(
            reader,
            &result.file_path,
            snippet_options.config,
        ));
    }
}

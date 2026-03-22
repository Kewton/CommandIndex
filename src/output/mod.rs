pub mod context_pack;
pub mod human;
pub mod json;
pub mod path;

use std::fmt;
use std::io::Write;

use clap::ValueEnum;
use serde::Serialize;

use crate::indexer::reader::SearchResult;

/// スニペット表示設定
#[derive(Debug, Clone, Copy)]
pub struct SnippetConfig {
    pub lines: usize,
    pub chars: usize,
}

impl Default for SnippetConfig {
    fn default() -> Self {
        Self {
            lines: 2,
            chars: 120,
        }
    }
}

/// 出力フォーマット
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Path,
}

/// 出力エラー型
#[derive(Debug)]
pub enum OutputError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::Io(e) => write!(f, "IO error: {e}"),
            OutputError::Json(e) => write!(f, "JSON serialization error: {e}"),
        }
    }
}

impl std::error::Error for OutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OutputError::Io(e) => Some(e),
            OutputError::Json(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for OutputError {
    fn from(e: std::io::Error) -> Self {
        OutputError::Io(e)
    }
}

impl From<serde_json::Error> for OutputError {
    fn from(e: serde_json::Error) -> Self {
        OutputError::Json(e)
    }
}

/// シンボル検索結果
#[derive(Debug, Clone)]
pub struct SymbolSearchResult {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_name: Option<String>,
    pub children: Vec<SymbolSearchResult>,
}

/// シンボル検索結果を指定フォーマットで出力する
pub fn format_symbol_results(
    results: &[SymbolSearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_symbol_human(results, writer),
        OutputFormat::Json => json::format_symbol_json(results, writer),
        OutputFormat::Path => path::format_symbol_path(results, writer),
    }
}

/// 関連検索結果
#[derive(Debug, Clone)]
pub struct RelatedSearchResult {
    pub file_path: String,
    pub score: f32,
    pub relation_types: Vec<RelationType>,
}

/// 関連タイプ
#[derive(Debug, Clone)]
pub enum RelationType {
    MarkdownLink,
    ImportDependency,
    TagMatch { matched_tags: Vec<String> },
    PathSimilarity,
    DirectoryProximity,
}

/// 関連検索結果を指定フォーマットで出力する
pub fn format_related_results(
    results: &[RelatedSearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_related_human(results, writer),
        OutputFormat::Json => json::format_related_json(results, writer),
        OutputFormat::Path => path::format_related_path(results, writer),
    }
}

/// セマンティック検索結果
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub path: String,
    pub heading: String,
    pub similarity: f32,
    pub body: String,
    pub tags: String,
    pub heading_level: u64,
}

/// セマンティック検索結果を指定フォーマットで出力する
pub fn format_semantic_results(
    results: &[SemanticSearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_semantic_human(results, writer),
        OutputFormat::Json => json::format_semantic_json(results, writer),
        OutputFormat::Path => path::format_semantic_path(results, writer),
    }
}

/// 検索結果を指定フォーマットで出力する
// NOTE: フォーマットが5種類以上に増えた場合、trait-based Formatterパターンへのリファクタリングを検討
pub fn format_results(
    results: &[SearchResult],
    format: OutputFormat,
    writer: &mut dyn Write,
) -> Result<(), OutputError> {
    match format {
        OutputFormat::Human => human::format_human(results, writer, SnippetConfig::default()),
        OutputFormat::Json => json::format_json(results, writer),
        OutputFormat::Path => path::format_path(results, writer),
    }
}

/// tags文字列をパースしてVec<&str>に変換する
/// SearchResult.tagsはスペース区切り文字列（例: "auth security"）
pub(crate) fn parse_tags(tags: &str) -> Vec<&str> {
    tags.split_whitespace().collect()
}

/// 本文を指定行数で切り詰める（マルチバイト文字安全）
pub(crate) fn truncate_body(body: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() > 1 {
        let taken: Vec<&str> = lines.iter().take(max_lines).copied().collect();
        let mut result = taken.join("\n");
        if lines.len() > max_lines {
            result.push_str("...");
        }
        result
    } else {
        let chars: String = body.chars().take(max_chars).collect();
        if body.chars().count() > max_chars {
            format!("{chars}...")
        } else {
            chars
        }
    }
}

/// 制御文字をストリッピングする（ANSIインジェクション対策）
/// 改行は保持し、それ以外の制御文字（0x00-0x1F, 0x7F）を除去
pub(crate) fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect()
}

/// AI向け文脈パッケージ
#[derive(Debug, Serialize)]
pub struct ContextPack {
    pub target_files: Vec<String>,
    pub context: Vec<ContextEntry>,
    pub summary: ContextSummary,
}

/// コンテキストエントリ
#[derive(Debug, Serialize)]
pub struct ContextEntry {
    pub path: String,
    pub relation: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
}

/// コンテキストサマリー
#[derive(Debug, Serialize)]
pub struct ContextSummary {
    pub total_related: usize,
    pub included: usize,
    pub estimated_tokens: usize,
}

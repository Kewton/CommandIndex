pub const IMPACT_AFTER_HELP: &str = "\
When to use:
  Understand which files are affected by changes to specified files.
  Supports stdin pipe for integration with git diff or other tools.

Examples:
  commandindexdev impact src/auth.rs --format json
  commandindexdev impact src/a.rs src/b.rs --format path
  echo src/auth.rs | commandindexdev impact --format json
  git diff --name-only | commandindexdev impact --format path
  # Impact with code snippets (JSON)
  commandindex impact src/auth.rs --with-snippet --format json";

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

use crate::cli::stdin::{
    StdinError, filter_existing_files, normalize_path_prefix, read_file_paths_from_stdin,
    validate_file_path,
};
use crate::indexer::reader::{IndexReaderWrapper, ReaderError};
use crate::indexer::symbol_store::{SymbolStore, SymbolStoreError};
use crate::output::{
    self, ImpactFileResult, ImpactResult, LlmFormatOptions, OutputError, OutputFormat,
};
use crate::search::related::{RelatedSearchEngine, RelatedSearchError};

const MAX_INPUT_FILES: usize = 500;

/// impact エラー型
#[derive(Debug)]
pub enum ImpactError {
    Stdin(StdinError),
    IndexNotFound,
    SymbolDbNotFound,
    Reader(ReaderError),
    SymbolStore(SymbolStoreError),
    RelatedSearch(RelatedSearchError),
    Output(OutputError),
    NoValidPaths,
    InvalidArgument(String),
}

impl fmt::Display for ImpactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImpactError::Stdin(e) => write!(f, "{e}"),
            ImpactError::IndexNotFound => {
                write!(f, "Index not found. Run `commandindex index` first.")
            }
            ImpactError::SymbolDbNotFound => {
                write!(
                    f,
                    "Symbol database not found. Run `commandindex index` first."
                )
            }
            ImpactError::Reader(e) => write!(f, "{e}"),
            ImpactError::SymbolStore(e) => write!(f, "{e}"),
            ImpactError::RelatedSearch(e) => write!(f, "{e}"),
            ImpactError::Output(e) => write!(f, "{e}"),
            ImpactError::NoValidPaths => {
                write!(f, "no valid file paths found after existence check")
            }
            ImpactError::InvalidArgument(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ImpactError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImpactError::Stdin(e) => Some(e),
            ImpactError::Reader(e) => Some(e),
            ImpactError::SymbolStore(e) => Some(e),
            ImpactError::RelatedSearch(e) => Some(e),
            ImpactError::Output(e) => Some(e),
            _ => None,
        }
    }
}

impl From<StdinError> for ImpactError {
    fn from(e: StdinError) -> Self {
        ImpactError::Stdin(e)
    }
}

impl From<ReaderError> for ImpactError {
    fn from(e: ReaderError) -> Self {
        ImpactError::Reader(e)
    }
}

impl From<SymbolStoreError> for ImpactError {
    fn from(e: SymbolStoreError) -> Self {
        ImpactError::SymbolStore(e)
    }
}

impl From<RelatedSearchError> for ImpactError {
    fn from(e: RelatedSearchError) -> Self {
        ImpactError::RelatedSearch(e)
    }
}

impl From<OutputError> for ImpactError {
    fn from(e: OutputError) -> Self {
        ImpactError::Output(e)
    }
}

/// impact サブコマンド実行
pub fn run_impact(
    files: &[String],
    format: OutputFormat,
    limit: Option<usize>,
    index_path: Option<&Path>,
    snippet_options: crate::cli::snippet_helper::SnippetOptions,
    max_tokens: Option<usize>,
    llm_options: &LlmFormatOptions,
) -> Result<(), ImpactError> {
    // 1. ファイルリスト取得（引数優先、なければstdin）
    let input_files = if files.is_empty() {
        read_file_paths_from_stdin(MAX_INPUT_FILES)?
    } else {
        validate_and_normalize(files)?
    };

    // 2. 存在チェック（warning出力してスキップ）
    let (valid_files, warnings) = filter_existing_files(&input_files);
    for w in &warnings {
        eprintln!("Warning: {w}");
    }
    if valid_files.is_empty() {
        return Err(ImpactError::NoValidPaths);
    }

    // 3. インデックス・DB確認（resolve_index_path で設定ファイル・CLIオプション対応）
    let config = crate::config::load_config(Path::new(".")).ok();
    let config_index_path = config.as_ref().and_then(|c| c.index.path.as_deref());
    let commandindex_dir =
        crate::indexer::resolve_index_path(index_path, config_index_path, Path::new("."))
            .unwrap_or_else(|_| Path::new(".").join(crate::INDEX_DIR_NAME));
    let tantivy_dir = crate::indexer::index_dir(&commandindex_dir);
    if !tantivy_dir.exists() {
        return Err(ImpactError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(&commandindex_dir);
    if !db_path.exists() {
        return Err(ImpactError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    // 4. 各ファイルの関連ファイル検索 & 集約
    let engine = RelatedSearchEngine::new(&reader, &store);
    let effective_limit = limit.unwrap_or(20);
    let mut result = aggregate_impact(&engine, &valid_files, effective_limit)?;

    // 4.5 スニペット一括付与（limit適用後）
    crate::cli::snippet_helper::enrich_impact_with_snippets(
        &mut result.impacted_files,
        &reader,
        &snippet_options,
        format,
    );

    // 4.6 トークン予算適用（--max-tokens）
    if let Some(max_tok) = max_tokens {
        result.impacted_files =
            crate::output::token_budget::apply_token_budget(result.impacted_files, max_tok, |r| {
                let mut tokens = crate::output::estimate_tokens(&r.file_path);
                if let Some(ref snippet) = r.snippet {
                    tokens = tokens.saturating_add(crate::output::estimate_tokens(snippet));
                }
                tokens
            });
        result.total_impacted_files = result.impacted_files.len();
    }

    // 5. 出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_impact_results(&result, format, &mut handle, llm_options)?;
    Ok(())
}

/// 引数からのファイルリストをバリデーション + 正規化
fn validate_and_normalize(files: &[String]) -> Result<Vec<String>, ImpactError> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for f in files {
        validate_file_path(f).map_err(ImpactError::Stdin)?;
        let normalized = normalize_path_prefix(f);
        if seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    if result.is_empty() {
        return Err(ImpactError::InvalidArgument(
            "At least one file is required".to_string(),
        ));
    }
    Ok(result)
}

/// 複数ファイルの関連結果を集約
fn aggregate_impact(
    engine: &RelatedSearchEngine,
    files: &[String],
    limit: usize,
) -> Result<ImpactResult, ImpactError> {
    let mut scores: HashMap<String, (f32, HashSet<String>, Vec<String>)> = HashMap::new();
    // key: file_path, value: (max_score, relation_types, impacted_by)

    for file in files {
        match engine.find_related(file, 1000) {
            Ok(results) => {
                for r in results {
                    let entry = scores
                        .entry(r.file_path.clone())
                        .or_insert_with(|| (0.0, HashSet::new(), Vec::new()));
                    // 最大スコア採用
                    if r.score > entry.0 {
                        entry.0 = r.score;
                    }
                    // relation_types union
                    for rt in &r.relation_types {
                        entry.1.insert(relation_type_to_string(rt));
                    }
                    // impacted_by 追加
                    if !entry.2.contains(file) {
                        entry.2.push(file.clone());
                    }
                }
            }
            Err(RelatedSearchError::FileNotFound(_))
            | Err(RelatedSearchError::FileNotIndexed(_)) => {
                // スキップ（warningは上で出力済み）
            }
            Err(e) => return Err(ImpactError::RelatedSearch(e)),
        }
    }

    // 入力ファイル自体を除外
    for file in files {
        scores.remove(file);
    }

    // スコア降順ソート & トリム
    let mut impacted: Vec<ImpactFileResult> = scores
        .into_iter()
        .map(|(path, (score, types, by))| ImpactFileResult {
            file_path: path,
            score,
            relation_types: {
                let mut v: Vec<String> = types.into_iter().collect();
                v.sort();
                v
            },
            impacted_by: by,
            snippet: None,
        })
        .collect();
    impacted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total_impacted = impacted.len().min(limit);
    impacted.truncate(limit);

    Ok(ImpactResult {
        input_files: files.to_vec(),
        impacted_files: impacted,
        total_input_files: files.len(),
        total_impacted_files: total_impacted,
    })
}

/// RelationType を snake_case 文字列に変換
fn relation_type_to_string(rt: &crate::output::RelationType) -> String {
    match rt {
        crate::output::RelationType::MarkdownLink => "markdown_link".to_string(),
        crate::output::RelationType::ImportDependency => "import_dependency".to_string(),
        crate::output::RelationType::TagMatch { .. } => "tag_match".to_string(),
        crate::output::RelationType::PathSimilarity => "path_similarity".to_string(),
        crate::output::RelationType::DirectoryProximity => "directory_proximity".to_string(),
        crate::output::RelationType::KnowledgeGraph => "knowledge_graph".to_string(),
    }
}

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
    self, ImpactPerFile, ImpactRelatedFile, ImpactResult, ImpactSummary, OutputError, OutputFormat,
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

    // 3. インデックス・DB確認
    let tantivy_dir = crate::indexer::index_dir(Path::new("."));
    if !tantivy_dir.exists() {
        return Err(ImpactError::IndexNotFound);
    }

    let db_path = crate::indexer::symbol_db_path(Path::new("."));
    if !db_path.exists() {
        return Err(ImpactError::SymbolDbNotFound);
    }

    let reader = IndexReaderWrapper::open(&tantivy_dir)?;
    let store = SymbolStore::open(&db_path)?;

    // 4. 各ファイルの関連ファイル検索 & 集約
    let engine = RelatedSearchEngine::new(&reader, &store);
    let effective_limit = limit.unwrap_or(20);
    let result = aggregate_impact(&engine, &valid_files, effective_limit)?;

    // 5. 出力
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    output::format_impact_results(&result, format, &mut handle)?;
    Ok(())
}

/// 引数からのファイルリストをバリデーション + 正規化
fn validate_and_normalize(files: &[String]) -> Result<Vec<String>, ImpactError> {
    if files.len() > MAX_INPUT_FILES {
        return Err(ImpactError::InvalidArgument(format!(
            "Too many input files ({}), maximum is {MAX_INPUT_FILES}",
            files.len()
        )));
    }
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

/// 内部検索の取得上限（per-file）
const INTERNAL_FETCH_LIMIT: usize = 1000;

/// 複数ファイルの関連結果を集約（Issue #90 仕様準拠）
fn aggregate_impact(
    engine: &RelatedSearchEngine,
    files: &[String],
    limit: usize,
) -> Result<ImpactResult, ImpactError> {
    let input_set: HashSet<&str> = files.iter().map(|f| f.as_str()).collect();
    let mut per_file_results: Vec<ImpactPerFile> = Vec::new();
    let mut overlap_map: HashMap<String, usize> = HashMap::new();
    let mut all_impacted: HashSet<String> = HashSet::new();

    for file in files {
        match engine.find_related(file, INTERNAL_FETCH_LIMIT) {
            Ok(results) => {
                // 入力ファイルを除外
                let filtered: Vec<_> = results
                    .into_iter()
                    .filter(|r| !input_set.contains(r.file_path.as_str()))
                    .collect();

                // overlap カウント + ユニーク集計（limit 前）
                for r in &filtered {
                    *overlap_map.entry(r.file_path.clone()).or_insert(0) += 1;
                    all_impacted.insert(r.file_path.clone());
                }

                // limit 適用して ImpactPerFile 構築
                let related: Vec<ImpactRelatedFile> = filtered
                    .iter()
                    .take(limit)
                    .map(|r| {
                        let relations: Vec<String> = r
                            .relation_types
                            .iter()
                            .map(relation_type_to_string)
                            .collect();
                        ImpactRelatedFile {
                            path: r.file_path.clone(),
                            score: r.score,
                            relations,
                        }
                    })
                    .collect();

                per_file_results.push(ImpactPerFile {
                    file: file.clone(),
                    related,
                });
            }
            Err(RelatedSearchError::FileNotFound(_))
            | Err(RelatedSearchError::FileNotIndexed(_)) => {
                eprintln!("Warning: skipping {file} (not found or not indexed)");
                per_file_results.push(ImpactPerFile {
                    file: file.clone(),
                    related: vec![],
                });
            }
            Err(e) => return Err(ImpactError::RelatedSearch(e)),
        }
    }

    // overlap: 2つ以上の入力ファイルから参照されるファイル
    let mut overlap: Vec<String> = overlap_map
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(path, _)| path)
        .collect();
    overlap.sort();

    let overlap_count = overlap.len();

    Ok(ImpactResult {
        changed_files: files.to_vec(),
        impact: per_file_results,
        overlap: overlap.clone(),
        summary: ImpactSummary {
            changed: files.len(),
            total_impacted: all_impacted.len(),
            overlap_count,
        },
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
    }
}

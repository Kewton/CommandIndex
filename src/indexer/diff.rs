use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::manifest::{Manifest, compute_file_hash, to_relative_path_string};
use crate::parser::ignore::IgnoreFilter;

/// 差分検知で発生しうるエラー
#[derive(Debug)]
pub enum DiffError {
    Io(std::io::Error),
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for DiffError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DiffError::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for DiffError {
    fn from(e: std::io::Error) -> Self {
        DiffError::Io(e)
    }
}

/// ファイルスキャン結果
pub struct ScanResult {
    /// 対象ファイルの絶対パス一覧
    pub files: Vec<PathBuf>,
    /// 無視されたファイル数
    pub ignored_count: u64,
}

/// 差分検知結果
pub struct DiffResult {
    /// 追加されたファイル（絶対パス）
    pub added: Vec<PathBuf>,
    /// 変更されたファイル（絶対パス）
    pub modified: Vec<PathBuf>,
    /// 削除されたファイル（相対パス）
    pub deleted: Vec<PathBuf>,
    /// 変更なしファイル数
    pub unchanged: usize,
}

impl DiffResult {
    /// 変更がないかどうかを返す（added, modified, deleted が全て空）
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }
}

/// 指定ディレクトリ配下のファイルをスキャンし、拡張子フィルタと IgnoreFilter を適用する
pub fn scan_files(
    base_path: &Path,
    ignore_filter: &IgnoreFilter,
    extensions: &[&str],
) -> Result<ScanResult, std::io::Error> {
    let mut files = Vec::new();
    let mut ignored_count: u64 = 0;

    for entry in WalkDir::new(base_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // 拡張子フィルタ
        let ext_match = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => extensions.contains(&ext),
            None => false,
        };
        if !ext_match {
            continue;
        }

        // IgnoreFilter 適用（相対パスで判定）
        let rel = path.strip_prefix(base_path).unwrap_or(path);
        if ignore_filter.is_ignored(rel) {
            ignored_count += 1;
            continue;
        }

        files.push(path.to_path_buf());
    }

    Ok(ScanResult {
        files,
        ignored_count,
    })
}

/// マニフェストと現在のファイル一覧を比較し、差分を検出する
pub fn detect_changes(
    base_path: &Path,
    manifest: &Manifest,
    current_files: &[PathBuf],
) -> Result<DiffResult, DiffError> {
    // 1. manifest.files → HashMap<相対パス, &FileEntry>
    let manifest_map: HashMap<&str, _> = manifest
        .files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();

    let mut visited: HashSet<String> = HashSet::new();
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    let mut unchanged: usize = 0;

    // 2. current_files ごとに判定
    for abs_path in current_files {
        let rel = to_relative_path_string(abs_path, base_path);
        visited.insert(rel.clone());

        match manifest_map.get(rel.as_str()) {
            None => {
                // マニフェストに存在しない → 追加
                added.push(abs_path.clone());
            }
            Some(entry) => {
                // ハッシュ比較
                match compute_file_hash(abs_path) {
                    Ok(hash) => {
                        if hash != entry.hash {
                            modified.push(abs_path.clone());
                        } else {
                            unchanged += 1;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // TOCTOU対策: スキャン後にファイルが消えた場合は deleted 扱い
                        deleted.push(PathBuf::from(&rel));
                    }
                    Err(e) => return Err(DiffError::Io(e)),
                }
            }
        }
    }

    // 3. manifest にあるが visited にないもの → deleted
    for entry in &manifest.files {
        if !visited.contains(&entry.path) {
            deleted.push(PathBuf::from(&entry.path));
        }
    }

    Ok(DiffResult {
        added,
        modified,
        deleted,
        unchanged,
    })
}

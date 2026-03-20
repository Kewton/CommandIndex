use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug)]
pub enum ManifestError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io(e) => write!(f, "IO error: {e}"),
            ManifestError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManifestError::Io(e) => Some(e),
            ManifestError::Json(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for ManifestError {
    fn from(e: std::io::Error) -> Self {
        ManifestError::Io(e)
    }
}

impl From<serde_json::Error> for ManifestError {
    fn from(e: serde_json::Error) -> Self {
        ManifestError::Json(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub hash: String,
    pub last_modified: DateTime<Utc>,
    pub sections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub files: Vec<FileEntry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

impl Manifest {
    /// 空のマニフェストを作成する
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// `.commandindex/` ディレクトリから manifest.json を読み込む
    pub fn load(commandindex_dir: &Path) -> Result<Self, ManifestError> {
        let path = commandindex_dir.join(MANIFEST_FILE);
        let content = std::fs::read_to_string(&path)?;
        let manifest: Self = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    /// `.commandindex/` ディレクトリに manifest.json を書き込む
    pub fn save(&self, commandindex_dir: &Path) -> Result<(), ManifestError> {
        std::fs::create_dir_all(commandindex_dir)?;
        let path = commandindex_dir.join(MANIFEST_FILE);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// ファイルエントリを追加する
    pub fn add_entry(&mut self, entry: FileEntry) {
        self.files.push(entry);
    }

    /// ファイル数を返す
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// パスでファイルエントリを検索する
    pub fn find_by_path(&self, path: &str) -> Option<&FileEntry> {
        self.files.iter().find(|e| e.path == path)
    }
}

/// ファイルのSHA-256ハッシュを計算する
pub fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read(path)?;
    let hash = Sha256::digest(&content);
    Ok(format!("sha256:{:x}", hash))
}

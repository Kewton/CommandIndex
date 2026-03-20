use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const MANIFEST_FILE: &str = "manifest.json";

/// ファイル種別を表す enum
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileType {
    #[default]
    Markdown,
    TypeScript,
    Python,
}

impl FileType {
    /// 拡張子から FileType を判定する
    pub fn from_extension(ext: &str) -> Option<FileType> {
        match ext {
            "md" => Some(FileType::Markdown),
            "ts" | "tsx" => Some(FileType::TypeScript),
            "py" => Some(FileType::Python),
            _ => None,
        }
    }

    /// 全サポート拡張子を返す
    pub fn all_extensions() -> &'static [&'static str] {
        &["md", "ts", "tsx", "py"]
    }

    /// コードファイルかどうかを判定（Markdown 以外は全てコード）
    pub fn is_code(&self) -> bool {
        !matches!(self, FileType::Markdown)
    }
}

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
    #[serde(default)]
    pub file_type: FileType,
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

    /// 指定パスのエントリを削除
    pub fn remove_by_path(&mut self, path: &str) {
        self.files.retain(|e| e.path != path);
    }

    /// エントリを追加または更新（既存パスがあれば上書き）
    pub fn upsert_entry(&mut self, entry: FileEntry) {
        if let Some(existing) = self.files.iter_mut().find(|e| e.path == entry.path) {
            *existing = entry;
        } else {
            self.files.push(entry);
        }
    }

    /// manifest.json を読み込む。ファイルが存在しない場合は空の Manifest を返す。
    pub fn load_or_default(commandindex_dir: &Path) -> Result<Self, ManifestError> {
        match Self::load(commandindex_dir) {
            Ok(m) => Ok(m),
            Err(ManifestError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::new())
            }
            Err(e) => Err(e),
        }
    }
}

/// 絶対パスを base_path からの相対パス文字列に変換する
pub fn to_relative_path_string(absolute: &Path, base: &Path) -> String {
    absolute
        .strip_prefix(base)
        .unwrap_or(absolute)
        .to_string_lossy()
        .to_string()
}

/// ファイルのSHA-256ハッシュを計算する
pub fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read(path)?;
    let hash = Sha256::digest(&content);
    Ok(format!("sha256:{:x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FileType tests ---

    #[test]
    fn file_type_from_extension_md() {
        assert_eq!(FileType::from_extension("md"), Some(FileType::Markdown));
    }

    #[test]
    fn file_type_from_extension_ts() {
        assert_eq!(FileType::from_extension("ts"), Some(FileType::TypeScript));
    }

    #[test]
    fn file_type_from_extension_tsx() {
        assert_eq!(FileType::from_extension("tsx"), Some(FileType::TypeScript));
    }

    #[test]
    fn file_type_from_extension_py() {
        assert_eq!(FileType::from_extension("py"), Some(FileType::Python));
    }

    #[test]
    fn file_type_from_extension_unknown() {
        assert_eq!(FileType::from_extension("rs"), None);
        assert_eq!(FileType::from_extension(""), None);
    }

    #[test]
    fn file_type_all_extensions_contains_all() {
        let exts = FileType::all_extensions();
        assert!(exts.contains(&"md"));
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"tsx"));
        assert!(exts.contains(&"py"));
    }

    #[test]
    fn file_type_is_code() {
        assert!(!FileType::Markdown.is_code());
        assert!(FileType::TypeScript.is_code());
        assert!(FileType::Python.is_code());
    }

    #[test]
    fn file_type_default_is_markdown() {
        assert_eq!(FileType::default(), FileType::Markdown);
    }

    #[test]
    fn file_entry_serde_backward_compat() {
        // Old JSON without file_type should deserialize with default (Markdown)
        let json = r#"{"path":"test.md","hash":"sha256:abc","last_modified":"2024-01-01T00:00:00Z","sections":1}"#;
        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.file_type, FileType::Markdown);
    }

    #[test]
    fn file_entry_serde_with_file_type() {
        let json = r#"{"path":"test.ts","hash":"sha256:abc","last_modified":"2024-01-01T00:00:00Z","sections":1,"file_type":"type_script"}"#;
        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.file_type, FileType::TypeScript);
    }

    #[test]
    fn file_entry_roundtrip_serde() {
        let entry = FileEntry {
            path: "src/main.py".to_string(),
            hash: "sha256:abc".to_string(),
            last_modified: chrono::Utc::now(),
            sections: 3,
            file_type: FileType::Python,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FileEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_type, FileType::Python);
    }
}

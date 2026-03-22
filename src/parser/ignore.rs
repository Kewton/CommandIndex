use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fmt;
use std::path::Path;

const DEFAULT_PATTERNS: &[&str] = &[
    "node_modules/**",
    "target/**",
    "dist/**",
    ".git/**",
    ".commandindex/**",
    "*.min.js",
    "*.lock",
];

#[derive(Debug)]
pub enum IgnoreError {
    Io(std::io::Error),
}

impl fmt::Display for IgnoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IgnoreError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for IgnoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IgnoreError::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for IgnoreError {
    fn from(e: std::io::Error) -> Self {
        IgnoreError::Io(e)
    }
}

pub struct IgnoreFilter {
    patterns: Vec<String>,
    glob_set: GlobSet,
}

impl Default for IgnoreFilter {
    fn default() -> Self {
        let patterns: Vec<String> = DEFAULT_PATTERNS.iter().map(|s| (*s).to_string()).collect();
        let glob_set = Self::build_glob_set(&patterns);
        Self { patterns, glob_set }
    }
}

impl IgnoreFilter {
    fn build_glob_set(patterns: &[String]) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            }
        }
        builder.build().unwrap_or_default()
    }

    /// カスタムインデックスパスがリポジトリ内にある場合、除外パターンに追加
    pub fn with_custom_index_path(mut self, index_path: &Path, repo_root: &Path) -> Self {
        if let Ok(rel) = index_path.strip_prefix(repo_root) {
            let pattern = format!("{}/**", rel.display());
            self.patterns.push(pattern);
            self.glob_set = Self::build_glob_set(&self.patterns);
        }
        self
    }
}

impl IgnoreFilter {
    /// `.cmindexignore` ファイルからフィルターを構築する。
    /// ファイルが存在しない場合はデフォルトルールを使用する。
    pub fn from_file(path: &Path) -> Result<Self, IgnoreError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            Ok(Self::from_content(&content))
        } else {
            Ok(Self::default())
        }
    }

    /// パターン文字列からフィルターを構築する
    pub fn from_content(content: &str) -> Self {
        let mut patterns = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Normalize directory patterns: "dir/" -> "dir/**"
            let pattern = if trimmed.ends_with('/') {
                format!("{trimmed}**")
            } else {
                trimmed.to_string()
            };

            patterns.push(pattern);
        }

        let glob_set = Self::build_glob_set(&patterns);
        Self { patterns, glob_set }
    }

    /// パスが除外対象かどうかを判定する
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.glob_set.is_match(path)
    }
}

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
    glob_set: GlobSet,
}

impl Default for IgnoreFilter {
    fn default() -> Self {
        let mut builder = GlobSetBuilder::new();
        for pattern in DEFAULT_PATTERNS {
            if let Ok(glob) = Glob::new(pattern) {
                builder.add(glob);
            }
        }
        let glob_set = builder.build().unwrap();
        Self { glob_set }
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
        let mut builder = GlobSetBuilder::new();

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

            match Glob::new(&pattern) {
                Ok(glob) => {
                    builder.add(glob);
                }
                Err(e) => {
                    tracing::warn!("Invalid glob pattern '{}': {}", trimmed, e);
                }
            }
        }

        let glob_set = builder.build().unwrap_or_else(|e| {
            tracing::warn!("Failed to build glob set: {}", e);
            GlobSetBuilder::new().build().unwrap()
        });

        Self { glob_set }
    }

    /// パスが除外対象かどうかを判定する
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.glob_set.is_match(path)
    }
}

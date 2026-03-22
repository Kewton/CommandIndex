use std::io::{self, BufRead, IsTerminal, Read};

/// stdin 入力全体のバイト数上限（巨大入力によるメモリ枯渇対策）
const MAX_STDIN_BYTES: u64 = 512 * 1024; // 512KB

/// stdin 入力ファイルパス数のデフォルト上限
pub const DEFAULT_MAX_STDIN_PATHS: usize = 500;

/// stdin エラー型
#[derive(Debug)]
pub enum StdinError {
    NotPiped,
    ReadError(io::Error),
    EmptyInput,
    NoValidPaths,
    TooManyPaths { count: usize, max: usize },
    InvalidPath { path: String, reason: String },
}

impl std::fmt::Display for StdinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPiped => write!(
                f,
                "stdin is a terminal. Pipe input required. Example: git diff --name-only | commandindex impact"
            ),
            Self::ReadError(e) => write!(f, "Failed to read from stdin: {e}"),
            Self::EmptyInput => write!(f, "no input received from stdin"),
            Self::NoValidPaths => write!(f, "no valid file paths found in input"),
            Self::TooManyPaths { count, max } => {
                write!(f, "too many paths ({count}), maximum is {max}")
            }
            Self::InvalidPath { path, reason } => {
                write!(
                    f,
                    "invalid path '{}': {reason}",
                    &path[..path.len().min(100)]
                )
            }
        }
    }
}

impl std::error::Error for StdinError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadError(e) => Some(e),
            _ => None,
        }
    }
}

/// stdin からファイルパスリストを読み取る
pub fn read_file_paths_from_stdin(max_paths: usize) -> Result<Vec<String>, StdinError> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Err(StdinError::NotPiped);
    }

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // バイト数上限でラップし、巨大入力によるメモリ枯渇を防止
    let reader = stdin.lock().take(MAX_STDIN_BYTES);
    for line in io::BufReader::new(reader).lines() {
        let line = line.map_err(StdinError::ReadError)?;
        let trimmed = line.trim().to_string();

        // 空行スキップ
        if trimmed.is_empty() {
            continue;
        }

        // パスバリデーション（warning出力してスキップ）
        if let Err(e) = validate_file_path(&trimmed) {
            eprintln!("Warning: {e}");
            continue;
        }

        // 正規化（./ 除去）
        let normalized = normalize_path_prefix(&trimmed);

        // 重複排除（正規化後で比較）
        if !seen.insert(normalized.clone()) {
            continue;
        }

        paths.push(normalized);

        if paths.len() > max_paths {
            return Err(StdinError::TooManyPaths {
                count: paths.len(),
                max: max_paths,
            });
        }
    }

    if paths.is_empty() {
        return Err(StdinError::EmptyInput);
    }

    Ok(paths)
}

/// パスバリデーション（共通関数。context.rs からも呼び出し可能）
pub(crate) fn validate_file_path(path: &str) -> Result<(), StdinError> {
    if path.is_empty() {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "empty path".to_string(),
        });
    }
    if path.len() > 1024 {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "path too long (max 1024)".to_string(),
        });
    }
    if path.contains('\0') {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "path contains null byte".to_string(),
        });
    }
    if path.contains("..") {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "path must not contain '..'".to_string(),
        });
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "absolute path not allowed".to_string(),
        });
    }
    if path.contains('\\') {
        return Err(StdinError::InvalidPath {
            path: path.to_string(),
            reason: "backslash not allowed".to_string(),
        });
    }
    Ok(())
}

/// パス正規化（./ 除去）
pub(crate) fn normalize_path_prefix(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}

/// 存在チェック + warning（impact / search --related-stdin 共通）
pub(crate) fn filter_existing_files(paths: &[String]) -> (Vec<String>, Vec<String>) {
    let mut valid = Vec::new();
    let mut warnings = Vec::new();
    for p in paths {
        if std::path::Path::new(p).exists() {
            valid.push(p.clone());
        } else {
            warnings.push(format!("file not found, skipping: {p}"));
        }
    }
    (valid, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_file_path tests ---

    #[test]
    fn validate_empty_path_rejected() {
        assert!(validate_file_path("").is_err());
    }

    #[test]
    fn validate_long_path_rejected() {
        let long = "a".repeat(1025);
        assert!(validate_file_path(&long).is_err());
    }

    #[test]
    fn validate_null_byte_rejected() {
        assert!(validate_file_path("src/main\0.rs").is_err());
    }

    #[test]
    fn validate_dotdot_rejected() {
        assert!(validate_file_path("../etc/passwd").is_err());
        assert!(validate_file_path("src/../main.rs").is_err());
    }

    #[test]
    fn validate_absolute_path_rejected() {
        assert!(validate_file_path("/etc/passwd").is_err());
        assert!(validate_file_path("\\windows\\system32").is_err());
    }

    #[test]
    fn validate_backslash_rejected() {
        assert!(validate_file_path("src\\main.rs").is_err());
    }

    #[test]
    fn validate_valid_paths_accepted() {
        assert!(validate_file_path("src/main.rs").is_ok());
        assert!(validate_file_path("README.md").is_ok());
        assert!(validate_file_path("./src/main.rs").is_ok());
        assert!(validate_file_path("docs/guide/setup.md").is_ok());
    }

    #[test]
    fn validate_max_length_accepted() {
        let path = "a".repeat(1024);
        assert!(validate_file_path(&path).is_ok());
    }

    // --- normalize_path_prefix tests ---

    #[test]
    fn normalize_strips_dot_slash() {
        assert_eq!(normalize_path_prefix("./src/main.rs"), "src/main.rs");
    }

    #[test]
    fn normalize_no_prefix_unchanged() {
        assert_eq!(normalize_path_prefix("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn normalize_dot_only() {
        assert_eq!(normalize_path_prefix("./"), "");
    }

    #[test]
    fn normalize_double_dot_slash_prefix() {
        // "./" is stripped only once (leading)
        assert_eq!(normalize_path_prefix("././file.rs"), "./file.rs");
    }

    // --- filter_existing_files tests ---

    #[test]
    fn filter_existing_files_nonexistent() {
        let paths = vec!["nonexistent_file_12345.rs".to_string()];
        let (valid, warnings) = filter_existing_files(&paths);
        assert!(valid.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nonexistent_file_12345.rs"));
    }

    #[test]
    fn filter_existing_files_existing() {
        // Cargo.toml should exist in the project root
        let paths = vec!["Cargo.toml".to_string()];
        let (valid, warnings) = filter_existing_files(&paths);
        assert_eq!(valid.len(), 1);
        assert!(warnings.is_empty());
    }

    // --- StdinError Display tests ---

    #[test]
    fn stdin_error_display_not_piped() {
        let err = StdinError::NotPiped;
        let msg = format!("{err}");
        assert!(msg.contains("terminal"));
        assert!(msg.contains("Pipe input required"));
    }

    #[test]
    fn stdin_error_display_empty_input() {
        let err = StdinError::EmptyInput;
        let msg = format!("{err}");
        assert!(msg.contains("no input received"));
    }

    #[test]
    fn stdin_error_display_too_many_paths() {
        let err = StdinError::TooManyPaths {
            count: 501,
            max: 500,
        };
        let msg = format!("{err}");
        assert!(msg.contains("501"));
        assert!(msg.contains("500"));
    }

    #[test]
    fn stdin_error_display_invalid_path() {
        let err = StdinError::InvalidPath {
            path: "bad\0path".to_string(),
            reason: "path contains null byte".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("null byte"));
    }
}

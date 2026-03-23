use std::path::Path;

use crate::cli::git::{GitError, get_changed_files};
use crate::cli::impact::{ImpactError, run_impact};
use crate::output::OutputFormat;

/// --changed-since エラー型
#[derive(Debug)]
pub enum ChangedSinceError {
    Git(GitError),
    Impact(ImpactError),
    NoChanges,
}

impl std::fmt::Display for ChangedSinceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangedSinceError::Git(e) => write!(f, "{e}"),
            ChangedSinceError::Impact(e) => write!(f, "{e}"),
            ChangedSinceError::NoChanges => write!(f, "No files changed in the specified period"),
        }
    }
}

impl std::error::Error for ChangedSinceError {}

/// --changed-since の実行
pub fn run_changed_since(
    since: &str,
    format: OutputFormat,
    limit: Option<usize>,
    index_path: Option<&Path>,
) -> Result<(), ChangedSinceError> {
    // 1. Git から変更ファイル取得
    let changed_files = get_changed_files(Path::new("."), since).map_err(ChangedSinceError::Git)?;

    if changed_files.is_empty() {
        return Err(ChangedSinceError::NoChanges);
    }

    // 2. impact サブコマンドに委譲
    run_impact(
        &changed_files,
        format,
        limit,
        index_path,
        crate::cli::snippet_helper::SnippetOptions::default(),
        &crate::output::LlmFormatOptions::default(),
    )
    .map_err(ChangedSinceError::Impact)
}

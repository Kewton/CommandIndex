pub mod changed_since;
pub mod clean;
pub mod config;
pub mod context;
pub mod diff;
pub mod embed;
pub mod export;
pub mod git;
pub mod help_llm;
pub mod impact;
pub mod import_index;
pub mod index;
pub mod search;
pub mod snippet_helper;
pub mod status;
pub mod stdin;
pub mod watch;
pub mod workspace;

/// Common file path validation for CLI handlers.
///
/// Checks:
/// - Non-empty slice
/// - File count <= `max_files`
/// - Each path: non-empty, length <= 1024, no `..`, no absolute path, no backslashes
pub(crate) fn validate_file_paths(
    file_paths: &[String],
    max_files: usize,
) -> Result<(), crate::cli::search::SearchError> {
    use crate::cli::search::SearchError;

    if file_paths.is_empty() {
        return Err(SearchError::InvalidArgument(
            "At least one file is required".to_string(),
        ));
    }
    if file_paths.len() > max_files {
        return Err(SearchError::InvalidArgument(format!(
            "Too many files specified (max {max_files})"
        )));
    }
    for f in file_paths {
        if f.is_empty() {
            return Err(SearchError::InvalidArgument(
                "File path cannot be empty".to_string(),
            ));
        }
        if f.len() > 1024 {
            return Err(SearchError::InvalidArgument(
                "File path too long (max 1024 characters)".to_string(),
            ));
        }
        // Sanitize path for error messages to prevent control character injection
        let safe_path = sanitize_path_for_display(f);
        if f.contains("..") {
            return Err(SearchError::InvalidArgument(format!(
                "File path must not contain '..': {safe_path}"
            )));
        }
        if f.starts_with('/') || f.starts_with('\\') {
            return Err(SearchError::InvalidArgument(format!(
                "File path must be relative: {safe_path}"
            )));
        }
        if f.contains('\\') {
            return Err(SearchError::InvalidArgument(format!(
                "File path must not contain backslashes: {safe_path}"
            )));
        }
    }
    Ok(())
}

/// Strips control characters from a file path for safe display in error messages.
fn sanitize_path_for_display(path: &str) -> String {
    path.chars().filter(|c| !c.is_control()).collect()
}

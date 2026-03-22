use std::path::Path;

use crate::cli::search::{SearchContext, SearchError};
use crate::config::workspace::{ResolvedRepository, load_workspace_config, resolve_repositories};
use crate::indexer::reader::{IndexReaderWrapper, SearchFilters, SearchOptions, SearchResult};
use crate::output::{OutputFormat, SnippetConfig, WorkspaceSearchResult, format_workspace_results};

/// ワークスペース横断検索を実行
#[allow(clippy::too_many_arguments)]
pub fn run_workspace_search(
    ws_path: &str,
    repo_filter: Option<&str>,
    options: &SearchOptions,
    filters: &SearchFilters,
    format: OutputFormat,
    snippet_config: SnippetConfig,
    _rerank: bool,
    _rerank_top: Option<usize>,
) -> Result<(), SearchError> {
    // 1. WorkspaceConfig読込
    let ws_config = load_workspace_config(Path::new(ws_path))?;
    let base_dir = Path::new(ws_path).parent().unwrap_or(Path::new("."));

    // 2. リポジトリ解決・バリデーション
    let (repos, warnings) = resolve_repositories(&ws_config, base_dir)?;

    // 3. 警告出力
    for w in &warnings {
        eprintln!("{w}");
    }

    // 4. --repo フィルタ（検索前フィルタ）
    let target_repos: Vec<&ResolvedRepository> = if let Some(filter) = repo_filter {
        let filtered: Vec<_> = repos.iter().filter(|r| r.alias == filter).collect();
        if filtered.is_empty() {
            return Err(SearchError::InvalidArgument(format!(
                "Repository '{}' not found in workspace",
                filter
            )));
        }
        filtered
    } else {
        repos.iter().collect()
    };

    if target_repos.is_empty() {
        return Err(SearchError::InvalidArgument(
            "No available repositories in workspace".to_string(),
        ));
    }

    // 5. 各リポジトリで逐次BM25検索
    let total = target_repos.len();
    let mut all_results: Vec<Vec<SearchResult>> = Vec::new();
    let mut repo_aliases: Vec<String> = Vec::new();

    for (i, repo) in target_repos.iter().enumerate() {
        eprintln!("[{}/{}] Searching {}...", i + 1, total, repo.alias);

        let index_dir = match SearchContext::from_path(&repo.path) {
            Ok(c) => c.index_dir(),
            Err(e) => {
                eprintln!("  Warning: skipping '{}': {}", repo.alias, e);
                continue;
            }
        };

        let reader = match IndexReaderWrapper::open(&index_dir) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Warning: skipping '{}': {}", repo.alias, e);
                continue;
            }
        };

        let results = match reader.search_with_options(options, filters) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Warning: skipping '{}': {}", repo.alias, e);
                continue;
            }
        };

        all_results.push(results);
        repo_aliases.push(repo.alias.clone());
    }

    if all_results.is_empty() {
        return Err(SearchError::InvalidArgument(
            "All repositories failed to search".to_string(),
        ));
    }

    // 6. pathにaliasプレフィックスを付与してキー衝突回避
    for (results, alias) in all_results.iter_mut().zip(repo_aliases.iter()) {
        for result in results.iter_mut() {
            result.path = format!("{}:{}", alias, result.path);
        }
    }

    // 7. rrf_merge_multipleで結果統合
    let limit = options.limit;
    let merged = crate::search::hybrid::rrf_merge_multiple(&all_results, limit);

    // 8. WorkspaceSearchResultに変換（aliasプレフィックスを分離）
    let workspace_results: Vec<WorkspaceSearchResult> = merged
        .into_iter()
        .map(|mut r| {
            let (alias, original_path) = r.path.split_once(':').unwrap_or(("unknown", &r.path));
            let repository = alias.to_string();
            r.path = original_path.to_string();
            WorkspaceSearchResult {
                repository,
                result: r,
            }
        })
        .collect();

    // 9. 出力
    let mut writer = std::io::stdout();
    format_workspace_results(&workspace_results, format, &mut writer, snippet_config)
        .map_err(SearchError::Output)?;

    Ok(())
}

/// ワークスペース横断ステータス表示
pub fn run_workspace_status(
    ws_path: &str,
    format: crate::cli::status::StatusFormat,
) -> Result<(), SearchError> {
    use crate::cli::status::{compute_dir_size, format_size};
    use crate::indexer::state::IndexState;

    let ws_config = load_workspace_config(Path::new(ws_path))?;
    let base_dir = Path::new(ws_path).parent().unwrap_or(Path::new("."));

    let (repos, warnings) = resolve_repositories(&ws_config, base_dir)?;

    for w in &warnings {
        eprintln!("{w}");
    }

    match format {
        crate::cli::status::StatusFormat::Human => {
            println!(
                "Workspace: {} ({} repositories)",
                ws_config.workspace.name,
                repos.len()
            );
            println!();
            println!(
                "{:<20} {:<40} {:>10} {:>20} {:<10}",
                "ALIAS", "PATH", "FILES", "LAST UPDATED", "STATUS"
            );
            println!("{}", "-".repeat(100));

            for repo in &repos {
                let commandindex_dir = repo.path.join(".commandindex");
                if !IndexState::exists(&commandindex_dir) {
                    println!(
                        "{:<20} {:<40} {:>10} {:>20} {:<10}",
                        repo.alias,
                        repo.path.display(),
                        "-",
                        "-",
                        "not indexed"
                    );
                    continue;
                }

                match IndexState::load(&commandindex_dir) {
                    Ok(state) => {
                        let size = compute_dir_size(&commandindex_dir);
                        println!(
                            "{:<20} {:<40} {:>10} {:>20} {:<10}",
                            repo.alias,
                            repo.path.display(),
                            state.total_files,
                            state.last_updated_at.format("%Y-%m-%d %H:%M"),
                            format!("ok ({})", format_size(size))
                        );
                    }
                    Err(e) => {
                        println!(
                            "{:<20} {:<40} {:>10} {:>20} {:<10}",
                            repo.alias,
                            repo.path.display(),
                            "-",
                            "-",
                            format!("error: {e}")
                        );
                    }
                }
            }
        }
        crate::cli::status::StatusFormat::Json => {
            use serde_json::json;

            let mut repo_infos = Vec::new();
            for repo in &repos {
                let commandindex_dir = repo.path.join(".commandindex");
                if !IndexState::exists(&commandindex_dir) {
                    repo_infos.push(json!({
                        "alias": repo.alias,
                        "path": repo.path.display().to_string(),
                        "status": "not indexed"
                    }));
                    continue;
                }

                match IndexState::load(&commandindex_dir) {
                    Ok(state) => {
                        let size = compute_dir_size(&commandindex_dir);
                        repo_infos.push(json!({
                            "alias": repo.alias,
                            "path": repo.path.display().to_string(),
                            "total_files": state.total_files,
                            "total_sections": state.total_sections,
                            "last_updated_at": state.last_updated_at.to_rfc3339(),
                            "index_size_bytes": size,
                            "status": "ok"
                        }));
                    }
                    Err(e) => {
                        repo_infos.push(json!({
                            "alias": repo.alias,
                            "path": repo.path.display().to_string(),
                            "status": format!("error: {e}")
                        }));
                    }
                }
            }

            let output = json!({
                "workspace": ws_config.workspace.name,
                "repositories": repo_infos
            });

            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            );
        }
    }

    Ok(())
}

/// ワークスペース横断インデックス更新
pub fn run_workspace_update(ws_path: &str, with_embedding: bool) -> Result<i32, SearchError> {
    let ws_config = load_workspace_config(Path::new(ws_path))?;
    let base_dir = Path::new(ws_path).parent().unwrap_or(Path::new("."));

    let (repos, warnings) = resolve_repositories(&ws_config, base_dir)?;

    for w in &warnings {
        eprintln!("{w}");
    }

    let total = repos.len();
    let mut errors: Vec<(String, String)> = Vec::new();

    for (i, repo) in repos.iter().enumerate() {
        eprintln!("[{}/{}] Updating {}...", i + 1, total, repo.alias);

        let options = crate::cli::index::IndexOptions { with_embedding };
        match crate::cli::index::run_incremental(&repo.path, &options) {
            Ok(summary) => {
                eprintln!(
                    "  Added: {} files ({} sections), Modified: {} files, Deleted: {} files, Duration: {:.2}s",
                    summary.added_files,
                    summary.added_sections,
                    summary.modified_files,
                    summary.deleted_files,
                    summary.duration.as_secs_f64()
                );
            }
            Err(e) => {
                let msg = format!("{e}");
                eprintln!("  Error: {msg}");
                errors.push((repo.alias.clone(), msg));
            }
        }
    }

    if !errors.is_empty() {
        eprintln!();
        eprintln!("Errors occurred in {} repositories:", errors.len());
        for (alias, msg) in &errors {
            eprintln!("  {}: {}", alias, msg);
        }
        Ok(1)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_workspace_search_missing_config() {
        let options = SearchOptions {
            query: "test".to_string(),
            tag: None,
            heading: None,
            limit: 10,
            no_semantic: true,
        };
        let filters = SearchFilters::default();
        let result = run_workspace_search(
            "/nonexistent/workspace.toml",
            None,
            &options,
            &filters,
            OutputFormat::Human,
            SnippetConfig::default(),
            false,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_run_workspace_status_missing_config() {
        let result = run_workspace_status(
            "/nonexistent/workspace.toml",
            crate::cli::status::StatusFormat::Human,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_run_workspace_update_missing_config() {
        let result = run_workspace_update("/nonexistent/workspace.toml", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_workspace_search_repo_filter_not_found() {
        // Create a temp workspace config
        let dir = tempfile::tempdir().unwrap();
        let ws_path = dir.path().join("workspace.toml");
        std::fs::write(
            &ws_path,
            r#"
[workspace]
name = "test-ws"

[[workspace.repositories]]
path = "."
alias = "repo-a"
"#,
        )
        .unwrap();

        let options = SearchOptions {
            query: "test".to_string(),
            tag: None,
            heading: None,
            limit: 10,
            no_semantic: true,
        };
        let filters = SearchFilters::default();
        let result = run_workspace_search(
            ws_path.to_str().unwrap(),
            Some("nonexistent-repo"),
            &options,
            &filters,
            OutputFormat::Human,
            SnippetConfig::default(),
            false,
            None,
        );
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("not found in workspace"),
            "Expected 'not found in workspace' in error: {err_msg}"
        );
    }
}

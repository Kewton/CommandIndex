use std::fs;
use std::path::PathBuf;

use commandindex::config::workspace::{
    MAX_ALIAS_LENGTH, MAX_CONFIG_FILE_SIZE, MAX_REPOSITORIES, WorkspaceConfigError,
    WorkspaceWarning, expand_path, load_workspace_config, resolve_repositories, validate_alias,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Constants sanity checks
// ---------------------------------------------------------------------------

#[test]
fn test_max_repositories_is_50() {
    assert_eq!(MAX_REPOSITORIES, 50);
}

#[test]
fn test_max_alias_length_is_64() {
    assert_eq!(MAX_ALIAS_LENGTH, 64);
}

#[test]
fn test_max_config_file_size_is_1mb() {
    assert_eq!(MAX_CONFIG_FILE_SIZE, 1_048_576);
}

// ---------------------------------------------------------------------------
// load_workspace_config – normal TOML parse
// ---------------------------------------------------------------------------

#[test]
fn test_load_workspace_config_normal() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        r#"
[workspace]
name = "my-workspace"

[[workspace.repositories]]
path = "/tmp/repo-a"
alias = "repo-a"

[[workspace.repositories]]
path = "/tmp/repo-b"
"#,
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    assert_eq!(config.workspace.name, "my-workspace");
    assert_eq!(config.workspace.repositories.len(), 2);
    assert_eq!(config.workspace.repositories[0].path, "/tmp/repo-a");
    assert_eq!(
        config.workspace.repositories[0].alias,
        Some("repo-a".to_string())
    );
    assert!(config.workspace.repositories[1].alias.is_none());
}

// ---------------------------------------------------------------------------
// resolve_repositories – path resolution
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_absolute_path() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    fs::create_dir_all(&repo_dir).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{}"
alias = "my-repo"
"#,
            repo_dir.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, warnings) = resolve_repositories(&config, tmp.path()).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].alias, "my-repo");
    assert_eq!(repos[0].path, repo_dir.canonicalize().unwrap());
    // No warnings expected for existing path
    assert!(
        warnings
            .iter()
            .all(|w| !matches!(w, WorkspaceWarning::RepositoryNotFound { .. }))
    );
}

#[test]
fn test_resolve_repositories_relative_path() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("sub").join("repo");
    fs::create_dir_all(&repo_dir).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "sub/repo"
alias = "sub-repo"
"#,
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, _) = resolve_repositories(&config, tmp.path()).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].alias, "sub-repo");
}

#[test]
fn test_resolve_repositories_tilde_path() {
    // Just test that expand_path handles ~ correctly
    let result = expand_path("~");
    assert!(result.is_ok());
    let home = result.unwrap();
    assert!(home.is_absolute());
}

// ---------------------------------------------------------------------------
// alias defaults to directory name
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_alias_defaults_to_dir_name() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-project");
    fs::create_dir_all(&repo_dir).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{}"
"#,
            repo_dir.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, _) = resolve_repositories(&config, tmp.path()).unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].alias, "my-project");
}

// ---------------------------------------------------------------------------
// duplicate alias detection
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_duplicate_alias() {
    let tmp = TempDir::new().unwrap();
    let repo_a = tmp.path().join("repo-a");
    let repo_b = tmp.path().join("repo-b");
    fs::create_dir_all(&repo_a).unwrap();
    fs::create_dir_all(&repo_b).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{}"
alias = "same-alias"

[[workspace.repositories]]
path = "{}"
alias = "same-alias"
"#,
            repo_a.display(),
            repo_b.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let result = resolve_repositories(&config, tmp.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        WorkspaceConfigError::DuplicateAlias(alias) => assert_eq!(alias, "same-alias"),
        e => panic!("Expected DuplicateAlias, got: {:?}", e),
    }
}

// ---------------------------------------------------------------------------
// duplicate path detection
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_duplicate_path() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{0}"
alias = "alias-a"

[[workspace.repositories]]
path = "{0}"
alias = "alias-b"
"#,
            repo.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let result = resolve_repositories(&config, tmp.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        WorkspaceConfigError::DuplicatePath(p) => {
            assert!(p.ends_with("repo"), "path should end with 'repo': {}", p)
        }
        e => panic!("Expected DuplicatePath, got: {:?}", e),
    }
}

// ---------------------------------------------------------------------------
// too many repositories
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_too_many() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");

    let mut repos_toml = String::new();
    for i in 0..=MAX_REPOSITORIES {
        let dir = tmp.path().join(format!("repo-{}", i));
        fs::create_dir_all(&dir).unwrap();
        repos_toml.push_str(&format!(
            r#"
[[workspace.repositories]]
path = "{}"
alias = "repo-{}"
"#,
            dir.display(),
            i
        ));
    }

    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"
{}
"#,
            repos_toml
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let result = resolve_repositories(&config, tmp.path());
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::TooManyRepositories
    ));
}

// ---------------------------------------------------------------------------
// invalid alias
// ---------------------------------------------------------------------------

#[test]
fn test_validate_alias_valid() {
    assert!(validate_alias("my-repo").is_ok());
    assert!(validate_alias("repo_123").is_ok());
    assert!(validate_alias("A").is_ok());
}

#[test]
fn test_validate_alias_control_characters() {
    let result = validate_alias("repo\x00name");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::InvalidName(_)
    ));
}

#[test]
fn test_validate_alias_special_characters() {
    assert!(validate_alias("repo/name").is_err());
    assert!(validate_alias("repo name").is_err());
    assert!(validate_alias("repo.name").is_err());
}

#[test]
fn test_validate_alias_too_long() {
    let long_alias = "a".repeat(MAX_ALIAS_LENGTH + 1);
    let result = validate_alias(&long_alias);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::InvalidName(_)
    ));
}

#[test]
fn test_validate_alias_max_length_ok() {
    let alias = "a".repeat(MAX_ALIAS_LENGTH);
    assert!(validate_alias(&alias).is_ok());
}

#[test]
fn test_validate_alias_empty() {
    let result = validate_alias("");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// unsafe path ($ sign, backtick)
// ---------------------------------------------------------------------------

#[test]
fn test_expand_path_dollar_sign_rejected() {
    let result = expand_path("$HOME/repo");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::UnsafePath(_)
    ));
}

#[test]
fn test_expand_path_backtick_rejected() {
    let result = expand_path("`whoami`/repo");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::UnsafePath(_)
    ));
}

// ---------------------------------------------------------------------------
// tilde expansion
// ---------------------------------------------------------------------------

#[test]
fn test_expand_path_tilde_only() {
    let result = expand_path("~").unwrap();
    assert!(result.is_absolute());
    // Should be the home directory
    let home = dirs::home_dir().unwrap();
    assert_eq!(result, home);
}

#[test]
fn test_expand_path_tilde_with_subpath() {
    let result = expand_path("~/some/path").unwrap();
    let home = dirs::home_dir().unwrap();
    assert_eq!(result, home.join("some/path"));
}

#[test]
fn test_expand_path_tilde_user_rejected() {
    let result = expand_path("~otheruser/repo");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::UnsafePath(_)
    ));
}

#[test]
fn test_expand_path_absolute() {
    let result = expand_path("/absolute/path").unwrap();
    assert_eq!(result, PathBuf::from("/absolute/path"));
}

#[test]
fn test_expand_path_relative() {
    let result = expand_path("relative/path").unwrap();
    assert_eq!(result, PathBuf::from("relative/path"));
}

// ---------------------------------------------------------------------------
// file too large
// ---------------------------------------------------------------------------

#[test]
fn test_load_workspace_config_file_too_large() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");

    // Create a file larger than 1MB
    let large_content = "a".repeat(MAX_CONFIG_FILE_SIZE as usize + 1);
    fs::write(&config_path, large_content).unwrap();

    let result = load_workspace_config(&config_path);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::FileTooLarge { .. }
    ));
}

// ---------------------------------------------------------------------------
// non-existent path → WorkspaceWarning::RepositoryNotFound
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_nonexistent_path_warning() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "/nonexistent/path/repo"
alias = "ghost"
"#,
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, warnings) = resolve_repositories(&config, tmp.path()).unwrap();
    // Non-existent path should be excluded from resolved repos
    assert_eq!(repos.len(), 0);
    // Should have a RepositoryNotFound warning
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, WorkspaceWarning::RepositoryNotFound { .. }))
    );
}

// ---------------------------------------------------------------------------
// symlink detection → WorkspaceWarning::SymlinkDetected
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn test_resolve_repositories_symlink_warning() {
    let tmp = TempDir::new().unwrap();
    let real_dir = tmp.path().join("real-repo");
    fs::create_dir_all(&real_dir).unwrap();

    let link_path = tmp.path().join("link-repo");
    std::os::unix::fs::symlink(&real_dir, &link_path).unwrap();

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{}"
alias = "linked"
"#,
            link_path.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, warnings) = resolve_repositories(&config, tmp.path()).unwrap();
    // Symlink should still be resolved
    assert_eq!(repos.len(), 1);
    // Should have a SymlinkDetected warning
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, WorkspaceWarning::SymlinkDetected { .. }))
    );
}

// ---------------------------------------------------------------------------
// read error (file not found)
// ---------------------------------------------------------------------------

#[test]
fn test_load_workspace_config_file_not_found() {
    let result = load_workspace_config(&PathBuf::from("/nonexistent/workspace.toml"));
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::ReadError { .. }
    ));
}

// ---------------------------------------------------------------------------
// parse error (invalid TOML)
// ---------------------------------------------------------------------------

#[test]
fn test_load_workspace_config_invalid_toml() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");
    fs::write(&config_path, "invalid toml {{{{").unwrap();

    let result = load_workspace_config(&config_path);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WorkspaceConfigError::ParseError { .. }
    ));
}

// ---------------------------------------------------------------------------
// workspace name validation
// ---------------------------------------------------------------------------

#[test]
fn test_load_workspace_config_invalid_workspace_name() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        r#"
[workspace]
name = "invalid name with spaces!"

[[workspace.repositories]]
path = "/tmp/repo"
"#,
    )
    .unwrap();

    let config = load_workspace_config(&config_path);
    assert!(config.is_err());
    assert!(matches!(
        config.unwrap_err(),
        WorkspaceConfigError::InvalidName(_)
    ));
}

// ---------------------------------------------------------------------------
// index not found warning
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_repositories_index_not_found_warning() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("repo-no-index");
    fs::create_dir_all(&repo_dir).unwrap();
    // Don't create .commandindex directory

    let config_path = tmp.path().join("workspace.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[workspace]
name = "test"

[[workspace.repositories]]
path = "{}"
alias = "no-index"
"#,
            repo_dir.display()
        ),
    )
    .unwrap();

    let config = load_workspace_config(&config_path).unwrap();
    let (repos, warnings) = resolve_repositories(&config, tmp.path()).unwrap();
    assert_eq!(repos.len(), 1);
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, WorkspaceWarning::IndexNotFound { .. }))
    );
}

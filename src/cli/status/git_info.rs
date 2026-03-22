use std::path::Path;
use std::process::Command;

use serde::Serialize;

/// インデックスの鮮度情報
#[derive(Debug, Serialize)]
pub struct StalenessInfo {
    pub last_commit_hash: Option<String>,
    pub commits_since_index: Option<u64>,
    pub files_changed_since_index: Option<u64>,
    pub recommendation: Option<String>,
}

/// コミットハッシュのバリデーション（4-40文字の16進数小文字）
pub fn validate_commit_hash(hash: &str) -> bool {
    (4..=40).contains(&hash.len())
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Git コマンドを実行し、成功時の stdout をトリム済み文字列で返す
fn run_git(repo_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 現在の HEAD コミットハッシュを取得する
pub fn get_current_commit_hash(repo_path: &Path) -> Option<String> {
    let hash = run_git(repo_path, &["rev-parse", "HEAD"])?;
    validate_commit_hash(&hash).then_some(hash)
}

/// インデックスの鮮度情報を取得する
pub fn get_staleness_info(
    base_path: &Path,
    last_commit_hash: Option<&str>,
) -> Option<StalenessInfo> {
    let current_hash = get_current_commit_hash(base_path);

    // If we can't get current hash, git is not available
    if current_hash.is_none() && last_commit_hash.is_none() {
        return None;
    }

    let (commits_since_index, files_changed_since_index) =
        match last_commit_hash.filter(|h| validate_commit_hash(h)) {
            Some(hash) => {
                let commits = count_commits_since(base_path, hash);
                let files = count_files_changed_since(base_path, hash);
                (commits, files)
            }
            None => (None, None),
        };

    let recommendation = determine_recommendation(commits_since_index, files_changed_since_index);

    Some(StalenessInfo {
        last_commit_hash: last_commit_hash.map(String::from),
        commits_since_index,
        files_changed_since_index,
        recommendation,
    })
}

/// 指定コミット以降のコミット数を取得する
fn count_commits_since(repo_path: &Path, commit_hash: &str) -> Option<u64> {
    run_git(
        repo_path,
        &["rev-list", "--count", &format!("{commit_hash}..HEAD")],
    )?
    .parse::<u64>()
    .ok()
}

/// 指定コミット以降に変更されたファイル数を取得する
fn count_files_changed_since(repo_path: &Path, commit_hash: &str) -> Option<u64> {
    let text = run_git(repo_path, &["diff", "--name-only", commit_hash, "HEAD"])?;
    Some(text.lines().filter(|l| !l.is_empty()).count() as u64)
}

/// 推奨アクションを決定する
fn determine_recommendation(
    commits_since: Option<u64>,
    files_changed: Option<u64>,
) -> Option<String> {
    match (commits_since, files_changed) {
        (Some(0), _) => Some("Index is up-to-date".to_string()),
        (Some(c), Some(f)) if c > 0 && f > 0 => Some(format!(
            "Run `commandindex update` ({c} commits, {f} files changed)"
        )),
        (Some(c), _) if c > 0 => Some(format!("Run `commandindex update` ({c} commits behind)")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_commit_hash_valid() {
        assert!(validate_commit_hash("abcd"));
        assert!(validate_commit_hash("0123456789abcdef"));
        assert!(validate_commit_hash("a".repeat(40).as_str()));
    }

    #[test]
    fn test_validate_commit_hash_invalid() {
        // Too short
        assert!(!validate_commit_hash("abc"));
        // Too long
        assert!(!validate_commit_hash(&"a".repeat(41)));
        // Uppercase
        assert!(!validate_commit_hash("ABCD"));
        // Non-hex
        assert!(!validate_commit_hash("ghij"));
        // Empty
        assert!(!validate_commit_hash(""));
    }

    #[test]
    fn test_determine_recommendation_up_to_date() {
        let rec = determine_recommendation(Some(0), Some(0));
        assert_eq!(rec, Some("Index is up-to-date".to_string()));
    }

    #[test]
    fn test_determine_recommendation_needs_update() {
        let rec = determine_recommendation(Some(5), Some(3));
        assert!(rec.unwrap().contains("commandindex update"));
    }

    #[test]
    fn test_determine_recommendation_none() {
        let rec = determine_recommendation(None, None);
        assert!(rec.is_none());
    }
}

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current export format version (integer increment, forward-compatible policy)
pub const EXPORT_FORMAT_VERSION: u32 = 1;

/// File name for export metadata inside the archive
pub const EXPORT_META_FILE: &str = "export_meta.json";

/// Export metadata stored as the first entry in the archive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportMeta {
    pub export_format_version: u32,
    pub commandindex_version: String,
    pub git_commit_hash: Option<String>,
    pub exported_at: DateTime<Utc>,
}

impl ExportMeta {
    /// Save export metadata to a file
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let content = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, content)
    }

    /// Load export metadata from a file
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        let meta: Self = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(meta)
    }
}

/// Get current git HEAD commit hash for the repository at `repo_path`.
///
/// Returns `None` if git is not available, the directory is not a git repository,
/// or the command fails for any reason.
pub fn current_git_hash(repo_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_meta_roundtrip() {
        let meta = ExportMeta {
            export_format_version: EXPORT_FORMAT_VERSION,
            commandindex_version: "0.0.5".to_string(),
            git_commit_hash: Some("abc123".to_string()),
            exported_at: Utc::now(),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ExportMeta = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.export_format_version, EXPORT_FORMAT_VERSION);
        assert_eq!(deserialized.commandindex_version, "0.0.5");
        assert_eq!(deserialized.git_commit_hash, Some("abc123".to_string()));
    }

    #[test]
    fn export_meta_none_git_hash() {
        let meta = ExportMeta {
            export_format_version: 1,
            commandindex_version: "0.0.5".to_string(),
            git_commit_hash: None,
            exported_at: Utc::now(),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ExportMeta = serde_json::from_str(&json).unwrap();
        assert!(deserialized.git_commit_hash.is_none());
    }

    #[test]
    fn export_meta_deny_unknown_fields() {
        let json = r#"{"export_format_version":1,"commandindex_version":"0.0.5","git_commit_hash":null,"exported_at":"2024-01-01T00:00:00Z","unknown_field":"value"}"#;
        let result: Result<ExportMeta, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn export_meta_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export_meta.json");

        let meta = ExportMeta {
            export_format_version: EXPORT_FORMAT_VERSION,
            commandindex_version: "0.0.5".to_string(),
            git_commit_hash: Some("def456".to_string()),
            exported_at: Utc::now(),
        };

        meta.save(&path).unwrap();
        let loaded = ExportMeta::load(&path).unwrap();

        assert_eq!(loaded.export_format_version, meta.export_format_version);
        assert_eq!(loaded.commandindex_version, meta.commandindex_version);
        assert_eq!(loaded.git_commit_hash, meta.git_commit_hash);
    }
}

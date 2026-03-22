use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::embedding::{EmbeddingConfig, ProviderType};
use crate::rerank::RerankConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Team-shared config file (repository root)
pub const TEAM_CONFIG_FILE: &str = "commandindex.toml";
/// Local personal config file (under .commandindex/)
pub const LOCAL_CONFIG_FILE: &str = "config.local.toml";
/// Legacy config file (deprecated fallback)
pub const LEGACY_CONFIG_FILE: &str = "config.toml";

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ConfigError {
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        source: toml::de::Error,
    },
    SerializeError(toml::ser::Error),
    /// Team config contains api_key (security violation)
    SecretInTeamConfig {
        path: PathBuf,
        field: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadError { path, source } => {
                write!(
                    f,
                    "Failed to read config file '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ParseError { path, source } => {
                write!(
                    f,
                    "Failed to parse config file '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::SerializeError(e) => write!(f, "Failed to serialize config: {}", e),
            Self::SecretInTeamConfig { path, field } => write!(
                f,
                "Security: '{}' contains '{}'. API keys must be in config.local.toml or environment variables.",
                path.display(),
                field
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

// ---------------------------------------------------------------------------
// RawConfig (intermediate structs for merging, all fields Option)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct RawConfig {
    pub index: Option<RawIndexConfig>,
    pub search: Option<RawSearchConfig>,
    pub embedding: Option<RawEmbeddingConfig>,
    pub rerank: Option<RawRerankConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawSearchConfig {
    pub default_limit: Option<usize>,
    pub snippet_lines: Option<usize>,
    pub snippet_chars: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawIndexConfig {
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawEmbeddingConfig {
    pub provider: Option<ProviderType>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
}

/// RawRerankConfig for merge (no provider field - Ollama is fixed)
#[derive(Debug, Default, Deserialize)]
pub struct RawRerankConfig {
    pub model: Option<String>,
    pub top_candidates: Option<usize>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub timeout_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// AppConfig (final merged config - NO Serialize for security)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub index: IndexConfig,
    pub search: SearchConfig,
    pub embedding: EmbeddingConfig,
    pub rerank: RerankConfig,
    /// Loaded config file path information
    pub loaded_sources: Vec<ConfigSource>,
}

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub path: PathBuf,
    pub kind: ConfigSourceKind,
}

#[derive(Debug, Clone)]
pub enum ConfigSourceKind {
    Team,   // commandindex.toml
    Local,  // .commandindex/config.local.toml
    Legacy, // .commandindex/config.toml (deprecated)
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexConfig {
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchConfig {
    pub default_limit: usize, // default: 20
    pub snippet_lines: usize, // default: 2
    pub snippet_chars: usize, // default: 120
}

// ---------------------------------------------------------------------------
// View models for config show (Serialize allowed on these only)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct AppConfigView {
    pub index: IndexConfig,
    pub search: SearchConfig,
    pub embedding: EmbeddingConfigView,
    pub rerank: RerankConfigView,
}

#[derive(Serialize)]
pub struct EmbeddingConfigView {
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub api_key: String,
}

#[derive(Serialize)]
pub struct RerankConfigView {
    pub model: String,
    pub top_candidates: usize,
    pub endpoint: String,
    pub api_key: String,
    pub timeout_secs: u64,
}

impl AppConfig {
    /// Create a masked view model for config show output
    pub fn to_masked_view(&self) -> AppConfigView {
        let provider_str = match self.embedding.provider {
            ProviderType::Ollama => "ollama",
            ProviderType::OpenAi => "openai",
        };
        AppConfigView {
            index: self.index.clone(),
            search: self.search.clone(),
            embedding: EmbeddingConfigView {
                provider: provider_str.to_string(),
                model: self.embedding.model.clone(),
                endpoint: self.embedding.endpoint.clone(),
                api_key: if self.embedding.api_key.is_some() {
                    "***".to_string()
                } else {
                    "(not set)".to_string()
                },
            },
            rerank: RerankConfigView {
                model: self.rerank.model.clone(),
                top_candidates: self.rerank.top_candidates,
                endpoint: self.rerank.endpoint.clone(),
                api_key: if self.rerank.api_key.is_some() {
                    "***".to_string()
                } else {
                    "(not set)".to_string()
                },
                timeout_secs: self.rerank.timeout_secs,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Loader functions
// ---------------------------------------------------------------------------

/// Load config with priority: env > local > team > legacy > defaults
///
/// base_path determination:
/// - Commands with --path (index, update, embed, clean): --path value
/// - Commands without --path (search, config): current directory "."
pub fn load_config(base_path: &Path) -> Result<AppConfig, ConfigError> {
    let mut sources = Vec::new();
    let mut merged = RawConfig::default();

    let legacy_path = base_path
        .join(crate::INDEX_DIR_NAME)
        .join(LEGACY_CONFIG_FILE);
    let team_path = base_path.join(TEAM_CONFIG_FILE);
    let local_path = base_path
        .join(crate::INDEX_DIR_NAME)
        .join(LOCAL_CONFIG_FILE);

    // 1. Legacy config file (deprecated fallback)
    if legacy_path.exists() {
        if team_path.exists() {
            eprintln!(
                "Warning: {} is ignored because {} exists.",
                legacy_path.display(),
                team_path.display()
            );
        } else {
            let raw = read_toml(&legacy_path)?;
            merged = merge_raw(merged, raw);
            sources.push(ConfigSource {
                path: legacy_path,
                kind: ConfigSourceKind::Legacy,
            });
            eprintln!(
                "Warning: {} is deprecated. Please migrate to {}",
                sources.last().unwrap().path.display(),
                team_path.display()
            );
        }
    }

    // 2. Team shared config (with api_key validation)
    if team_path.exists() {
        let raw = read_toml(&team_path)?;
        validate_no_secrets(&team_path, &raw)?;
        merged = merge_raw(merged, raw);
        sources.push(ConfigSource {
            path: team_path,
            kind: ConfigSourceKind::Team,
        });
    }

    // 3. Local personal config
    if local_path.exists() {
        let raw = read_toml(&local_path)?;
        merged = merge_raw(merged, raw);
        sources.push(ConfigSource {
            path: local_path,
            kind: ConfigSourceKind::Local,
        });
    }

    // 4. Convert RawConfig -> AppConfig with defaults
    Ok(resolve_config(merged, sources))
}

/// Read a TOML file into RawConfig
fn read_toml(path: &Path) -> Result<RawConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
        path: path.to_path_buf(),
        source: e,
    })?;
    toml::from_str(&content).map_err(|e| ConfigError::ParseError {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Validate that team config does not contain api_key
fn validate_no_secrets(path: &Path, raw: &RawConfig) -> Result<(), ConfigError> {
    if let Some(ref emb) = raw.embedding
        && emb.api_key.is_some()
    {
        return Err(ConfigError::SecretInTeamConfig {
            path: path.to_path_buf(),
            field: "embedding.api_key".to_string(),
        });
    }
    if let Some(ref rer) = raw.rerank
        && rer.api_key.is_some()
    {
        return Err(ConfigError::SecretInTeamConfig {
            path: path.to_path_buf(),
            field: "rerank.api_key".to_string(),
        });
    }
    Ok(())
}

/// Field-level merge: higher priority wins
fn merge_raw(base: RawConfig, higher: RawConfig) -> RawConfig {
    RawConfig {
        index: merge_index(base.index, higher.index),
        search: merge_search(base.search, higher.search),
        embedding: merge_embedding(base.embedding, higher.embedding),
        rerank: merge_rerank(base.rerank, higher.rerank),
    }
}

fn merge_index(
    base: Option<RawIndexConfig>,
    higher: Option<RawIndexConfig>,
) -> Option<RawIndexConfig> {
    match (base, higher) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(h)) => Some(h),
        (Some(b), Some(h)) => Some(RawIndexConfig {
            languages: h.languages.or(b.languages),
        }),
    }
}

fn merge_search(
    base: Option<RawSearchConfig>,
    higher: Option<RawSearchConfig>,
) -> Option<RawSearchConfig> {
    match (base, higher) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(h)) => Some(h),
        (Some(b), Some(h)) => Some(RawSearchConfig {
            default_limit: h.default_limit.or(b.default_limit),
            snippet_lines: h.snippet_lines.or(b.snippet_lines),
            snippet_chars: h.snippet_chars.or(b.snippet_chars),
        }),
    }
}

fn merge_embedding(
    base: Option<RawEmbeddingConfig>,
    higher: Option<RawEmbeddingConfig>,
) -> Option<RawEmbeddingConfig> {
    match (base, higher) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(h)) => Some(h),
        (Some(b), Some(h)) => Some(RawEmbeddingConfig {
            provider: h.provider.or(b.provider),
            model: h.model.or(b.model),
            endpoint: h.endpoint.or(b.endpoint),
            api_key: h.api_key.or(b.api_key),
        }),
    }
}

fn merge_rerank(
    base: Option<RawRerankConfig>,
    higher: Option<RawRerankConfig>,
) -> Option<RawRerankConfig> {
    match (base, higher) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(h)) => Some(h),
        (Some(b), Some(h)) => Some(RawRerankConfig {
            model: h.model.or(b.model),
            top_candidates: h.top_candidates.or(b.top_candidates),
            endpoint: h.endpoint.or(b.endpoint),
            api_key: h.api_key.or(b.api_key),
            timeout_secs: h.timeout_secs.or(b.timeout_secs),
        }),
    }
}

/// Convert RawConfig to AppConfig with defaults applied
fn resolve_config(raw: RawConfig, sources: Vec<ConfigSource>) -> AppConfig {
    let index = IndexConfig {
        languages: raw.index.and_then(|i| i.languages).unwrap_or_default(),
    };

    let search = SearchConfig {
        default_limit: raw
            .search
            .as_ref()
            .and_then(|s| s.default_limit)
            .unwrap_or(20),
        snippet_lines: raw
            .search
            .as_ref()
            .and_then(|s| s.snippet_lines)
            .unwrap_or(2),
        snippet_chars: raw.search.and_then(|s| s.snippet_chars).unwrap_or(120),
    };

    let embedding = if let Some(emb) = raw.embedding {
        EmbeddingConfig {
            provider: emb.provider.unwrap_or_default(),
            model: emb.model.unwrap_or_else(|| "nomic-embed-text".to_string()),
            endpoint: emb
                .endpoint
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            api_key: emb.api_key,
        }
    } else {
        EmbeddingConfig::default()
    };

    let rerank = if let Some(rer) = raw.rerank {
        RerankConfig {
            model: rer.model.unwrap_or_else(|| "llama3".to_string()),
            top_candidates: rer.top_candidates.unwrap_or(20),
            endpoint: rer
                .endpoint
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            api_key: rer.api_key,
            timeout_secs: rer.timeout_secs.unwrap_or(30),
        }
    } else {
        RerankConfig::default()
    };

    AppConfig {
        index,
        search,
        embedding,
        rerank,
        loaded_sources: sources,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- RawConfig defaults ---

    #[test]
    fn test_raw_config_default_is_all_none() {
        let raw = RawConfig::default();
        assert!(raw.index.is_none());
        assert!(raw.search.is_none());
        assert!(raw.embedding.is_none());
        assert!(raw.rerank.is_none());
    }

    // --- ConfigError Display ---

    #[test]
    fn test_config_error_display_read() {
        let err = ConfigError::ReadError {
            path: PathBuf::from("/tmp/config.toml"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Failed to read"));
        assert!(msg.contains("/tmp/config.toml"));
    }

    #[test]
    fn test_config_error_display_secret() {
        let err = ConfigError::SecretInTeamConfig {
            path: PathBuf::from("commandindex.toml"),
            field: "embedding.api_key".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Security"));
        assert!(msg.contains("embedding.api_key"));
        assert!(msg.contains("config.local.toml"));
    }

    // --- merge_raw ---

    #[test]
    fn test_merge_raw_higher_wins() {
        let base = RawConfig {
            search: Some(RawSearchConfig {
                default_limit: Some(10),
                snippet_lines: Some(3),
                snippet_chars: None,
            }),
            embedding: Some(RawEmbeddingConfig {
                provider: Some(ProviderType::Ollama),
                model: Some("base-model".to_string()),
                endpoint: None,
                api_key: None,
            }),
            ..RawConfig::default()
        };
        let higher = RawConfig {
            search: Some(RawSearchConfig {
                default_limit: Some(50),
                snippet_lines: None,
                snippet_chars: Some(200),
            }),
            embedding: Some(RawEmbeddingConfig {
                provider: None,
                model: Some("higher-model".to_string()),
                endpoint: Some("http://custom:8080".to_string()),
                api_key: Some("sk-key".to_string()),
            }),
            ..RawConfig::default()
        };

        let merged = merge_raw(base, higher);
        let search = merged.search.unwrap();
        assert_eq!(search.default_limit, Some(50)); // higher wins
        assert_eq!(search.snippet_lines, Some(3)); // base preserved
        assert_eq!(search.snippet_chars, Some(200)); // higher wins

        let emb = merged.embedding.unwrap();
        assert_eq!(emb.provider, Some(ProviderType::Ollama)); // base preserved
        assert_eq!(emb.model.as_deref(), Some("higher-model")); // higher wins
        assert_eq!(emb.endpoint.as_deref(), Some("http://custom:8080")); // higher wins
        assert_eq!(emb.api_key.as_deref(), Some("sk-key")); // higher wins
    }

    #[test]
    fn test_merge_raw_both_none() {
        let base = RawConfig::default();
        let higher = RawConfig::default();
        let merged = merge_raw(base, higher);
        assert!(merged.index.is_none());
        assert!(merged.search.is_none());
        assert!(merged.embedding.is_none());
        assert!(merged.rerank.is_none());
    }

    #[test]
    fn test_merge_raw_rerank_fields() {
        let base = RawConfig {
            rerank: Some(RawRerankConfig {
                model: Some("base-model".to_string()),
                top_candidates: Some(10),
                endpoint: None,
                api_key: None,
                timeout_secs: Some(60),
            }),
            ..RawConfig::default()
        };
        let higher = RawConfig {
            rerank: Some(RawRerankConfig {
                model: None,
                top_candidates: Some(30),
                endpoint: Some("http://custom:1234".to_string()),
                api_key: Some("key".to_string()),
                timeout_secs: None,
            }),
            ..RawConfig::default()
        };
        let merged = merge_raw(base, higher);
        let rer = merged.rerank.unwrap();
        assert_eq!(rer.model.as_deref(), Some("base-model")); // base preserved
        assert_eq!(rer.top_candidates, Some(30)); // higher wins
        assert_eq!(rer.endpoint.as_deref(), Some("http://custom:1234"));
        assert_eq!(rer.api_key.as_deref(), Some("key"));
        assert_eq!(rer.timeout_secs, Some(60)); // base preserved
    }

    // --- validate_no_secrets ---

    #[test]
    fn test_validate_no_secrets_clean() {
        let raw = RawConfig {
            embedding: Some(RawEmbeddingConfig {
                provider: Some(ProviderType::Ollama),
                model: Some("model".to_string()),
                endpoint: None,
                api_key: None,
            }),
            ..RawConfig::default()
        };
        let result = validate_no_secrets(Path::new("commandindex.toml"), &raw);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_secrets_embedding_api_key_rejected() {
        let raw = RawConfig {
            embedding: Some(RawEmbeddingConfig {
                api_key: Some("sk-secret".to_string()),
                ..RawEmbeddingConfig::default()
            }),
            ..RawConfig::default()
        };
        let result = validate_no_secrets(Path::new("commandindex.toml"), &raw);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("embedding.api_key"));
    }

    #[test]
    fn test_validate_no_secrets_rerank_api_key_rejected() {
        let raw = RawConfig {
            rerank: Some(RawRerankConfig {
                api_key: Some("sk-secret".to_string()),
                ..RawRerankConfig::default()
            }),
            ..RawConfig::default()
        };
        let result = validate_no_secrets(Path::new("commandindex.toml"), &raw);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("rerank.api_key"));
    }

    // --- resolve_config ---

    #[test]
    fn test_resolve_config_defaults() {
        let raw = RawConfig::default();
        let config = resolve_config(raw, vec![]);
        assert_eq!(config.search.default_limit, 20);
        assert_eq!(config.search.snippet_lines, 2);
        assert_eq!(config.search.snippet_chars, 120);
        assert_eq!(config.embedding.provider, ProviderType::Ollama);
        assert_eq!(config.embedding.model, "nomic-embed-text");
        assert_eq!(config.embedding.endpoint, "http://localhost:11434");
        assert!(config.embedding.api_key.is_none());
        assert_eq!(config.rerank.model, "llama3");
        assert_eq!(config.rerank.top_candidates, 20);
        assert_eq!(config.rerank.timeout_secs, 30);
        assert!(config.loaded_sources.is_empty());
    }

    #[test]
    fn test_resolve_config_with_values() {
        let raw = RawConfig {
            search: Some(RawSearchConfig {
                default_limit: Some(50),
                snippet_lines: None,
                snippet_chars: Some(200),
            }),
            embedding: Some(RawEmbeddingConfig {
                provider: Some(ProviderType::OpenAi),
                model: Some("text-embedding-3-small".to_string()),
                endpoint: Some("https://api.openai.com".to_string()),
                api_key: Some("sk-key".to_string()),
            }),
            rerank: Some(RawRerankConfig {
                model: Some("gemma2".to_string()),
                top_candidates: Some(30),
                endpoint: None,
                api_key: None,
                timeout_secs: Some(60),
            }),
            ..RawConfig::default()
        };
        let config = resolve_config(raw, vec![]);
        assert_eq!(config.search.default_limit, 50);
        assert_eq!(config.search.snippet_lines, 2); // default
        assert_eq!(config.search.snippet_chars, 200);
        assert_eq!(config.embedding.provider, ProviderType::OpenAi);
        assert_eq!(config.embedding.model, "text-embedding-3-small");
        assert_eq!(config.embedding.api_key.as_deref(), Some("sk-key"));
        assert_eq!(config.rerank.model, "gemma2");
        assert_eq!(config.rerank.top_candidates, 30);
        assert_eq!(config.rerank.timeout_secs, 60);
    }

    // --- to_masked_view ---

    #[test]
    fn test_to_masked_view_masks_api_keys() {
        let config = AppConfig {
            index: IndexConfig { languages: vec![] },
            search: SearchConfig {
                default_limit: 20,
                snippet_lines: 2,
                snippet_chars: 120,
            },
            embedding: EmbeddingConfig {
                provider: ProviderType::OpenAi,
                model: "test-model".to_string(),
                endpoint: "https://api.openai.com".to_string(),
                api_key: Some("sk-secret-key".to_string()),
            },
            rerank: RerankConfig {
                model: "llama3".to_string(),
                top_candidates: 20,
                endpoint: "http://localhost:11434".to_string(),
                api_key: Some("sk-rerank-key".to_string()),
                timeout_secs: 30,
            },
            loaded_sources: vec![],
        };

        let view = config.to_masked_view();
        assert_eq!(view.embedding.api_key, "***");
        assert_eq!(view.rerank.api_key, "***");
        assert_eq!(view.embedding.provider, "openai");
    }

    #[test]
    fn test_to_masked_view_no_api_keys() {
        let config = AppConfig {
            index: IndexConfig { languages: vec![] },
            search: SearchConfig {
                default_limit: 20,
                snippet_lines: 2,
                snippet_chars: 120,
            },
            embedding: EmbeddingConfig::default(),
            rerank: RerankConfig::default(),
            loaded_sources: vec![],
        };

        let view = config.to_masked_view();
        assert_eq!(view.embedding.api_key, "(not set)");
        assert_eq!(view.rerank.api_key, "(not set)");
    }

    // --- load_config with temp directories ---

    #[test]
    fn test_load_config_no_files_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let config = load_config(tmp.path()).unwrap();
        assert_eq!(config.search.default_limit, 20);
        assert!(config.loaded_sources.is_empty());
    }

    #[test]
    fn test_load_config_team_config_only() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("commandindex.toml"),
            r#"
[search]
default_limit = 50

[embedding]
provider = "ollama"
model = "custom-model"
"#,
        )
        .unwrap();

        let config = load_config(tmp.path()).unwrap();
        assert_eq!(config.search.default_limit, 50);
        assert_eq!(config.embedding.model, "custom-model");
        assert_eq!(config.loaded_sources.len(), 1);
        assert!(matches!(
            config.loaded_sources[0].kind,
            ConfigSourceKind::Team
        ));
    }

    #[test]
    fn test_load_config_local_overrides_team() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("commandindex.toml"),
            r#"
[embedding]
provider = "ollama"
model = "team-model"
"#,
        )
        .unwrap();

        let ci_dir = tmp.path().join(".commandindex");
        std::fs::create_dir_all(&ci_dir).unwrap();
        std::fs::write(
            ci_dir.join("config.local.toml"),
            r#"
[embedding]
model = "local-model"
api_key = "sk-local-key"
"#,
        )
        .unwrap();

        let config = load_config(tmp.path()).unwrap();
        assert_eq!(config.embedding.model, "local-model"); // local wins
        assert_eq!(config.embedding.api_key.as_deref(), Some("sk-local-key"));
        assert_eq!(config.loaded_sources.len(), 2);
    }

    #[test]
    fn test_load_config_legacy_fallback() {
        let tmp = TempDir::new().unwrap();
        let ci_dir = tmp.path().join(".commandindex");
        std::fs::create_dir_all(&ci_dir).unwrap();
        std::fs::write(
            ci_dir.join("config.toml"),
            r#"
[embedding]
provider = "ollama"
model = "legacy-model"
"#,
        )
        .unwrap();

        let config = load_config(tmp.path()).unwrap();
        assert_eq!(config.embedding.model, "legacy-model");
        assert_eq!(config.loaded_sources.len(), 1);
        assert!(matches!(
            config.loaded_sources[0].kind,
            ConfigSourceKind::Legacy
        ));
    }

    #[test]
    fn test_load_config_legacy_ignored_when_team_exists() {
        let tmp = TempDir::new().unwrap();

        // Team config
        std::fs::write(
            tmp.path().join("commandindex.toml"),
            r#"
[embedding]
provider = "ollama"
model = "team-model"
"#,
        )
        .unwrap();

        // Legacy config (should be ignored)
        let ci_dir = tmp.path().join(".commandindex");
        std::fs::create_dir_all(&ci_dir).unwrap();
        std::fs::write(
            ci_dir.join("config.toml"),
            r#"
[embedding]
provider = "ollama"
model = "legacy-model"
"#,
        )
        .unwrap();

        let config = load_config(tmp.path()).unwrap();
        assert_eq!(config.embedding.model, "team-model"); // team wins, legacy ignored
        assert_eq!(config.loaded_sources.len(), 1);
        assert!(matches!(
            config.loaded_sources[0].kind,
            ConfigSourceKind::Team
        ));
    }

    #[test]
    fn test_load_config_team_with_api_key_rejected() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("commandindex.toml"),
            r#"
[embedding]
provider = "openai"
api_key = "sk-should-not-be-here"
"#,
        )
        .unwrap();

        let result = load_config(tmp.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("embedding.api_key"));
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("commandindex.toml"), "invalid toml {{{{").unwrap();

        let result = load_config(tmp.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Failed to parse"));
    }

    // --- TOML roundtrip / field sync test ---

    #[test]
    fn test_toml_roundtrip_all_fields() {
        let toml_str = r#"
[index]
languages = ["typescript", "python"]

[search]
default_limit = 50
snippet_lines = 5
snippet_chars = 200

[embedding]
provider = "openai"
model = "text-embedding-3-small"
endpoint = "https://api.openai.com"
api_key = "sk-test"

[rerank]
model = "gemma2"
top_candidates = 30
endpoint = "http://localhost:11434"
api_key = "sk-rerank"
timeout_secs = 60
"#;
        let raw: RawConfig = toml::from_str(toml_str).unwrap();
        let config = resolve_config(raw, vec![]);

        assert_eq!(config.index.languages, vec!["typescript", "python"]);
        assert_eq!(config.search.default_limit, 50);
        assert_eq!(config.search.snippet_lines, 5);
        assert_eq!(config.search.snippet_chars, 200);
        assert_eq!(config.embedding.provider, ProviderType::OpenAi);
        assert_eq!(config.embedding.model, "text-embedding-3-small");
        assert_eq!(config.embedding.endpoint, "https://api.openai.com");
        assert_eq!(config.embedding.api_key.as_deref(), Some("sk-test"));
        assert_eq!(config.rerank.model, "gemma2");
        assert_eq!(config.rerank.top_candidates, 30);
        assert_eq!(config.rerank.endpoint, "http://localhost:11434");
        assert_eq!(config.rerank.api_key.as_deref(), Some("sk-rerank"));
        assert_eq!(config.rerank.timeout_secs, 60);
    }

    // --- view model TOML serialization ---

    #[test]
    fn test_view_model_serializes_to_toml() {
        let config = AppConfig {
            index: IndexConfig {
                languages: vec!["rust".to_string()],
            },
            search: SearchConfig {
                default_limit: 20,
                snippet_lines: 2,
                snippet_chars: 120,
            },
            embedding: EmbeddingConfig {
                provider: ProviderType::Ollama,
                model: "nomic-embed-text".to_string(),
                endpoint: "http://localhost:11434".to_string(),
                api_key: Some("secret".to_string()),
            },
            rerank: RerankConfig::default(),
            loaded_sources: vec![],
        };
        let view = config.to_masked_view();
        let toml_str = toml::to_string_pretty(&view).unwrap();
        assert!(toml_str.contains("api_key = \"***\""));
        assert!(!toml_str.contains("secret"));
    }
}

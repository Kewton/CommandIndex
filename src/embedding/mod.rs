pub mod ollama;
pub mod openai;
pub mod store;

use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// EmbeddingProvider trait
// ---------------------------------------------------------------------------

/// Embedding生成プロバイダーのトレイト
pub trait EmbeddingProvider: Send + Sync {
    /// テキスト群のEmbeddingを生成
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    /// Embeddingの次元数を返す
    fn dimension(&self) -> usize;
    /// プロバイダー名を返す
    fn provider_name(&self) -> &str;
    /// モデル名を返す
    fn model_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// EmbeddingError
// ---------------------------------------------------------------------------

/// Errors that can occur during embedding generation.
#[derive(Debug)]
pub enum EmbeddingError {
    /// HTTP接続失敗
    NetworkError(String),
    /// API応答エラー（ステータスコード + メッセージ）
    ApiError { status: u16, message: String },
    /// モデルが見つからない
    ModelNotFound(String),
    /// レスポンスのパースに失敗
    InvalidResponse(String),
    /// レート制限
    RateLimited,
    /// タイムアウト
    Timeout,
    /// 設定エラー
    ConfigError(String),
}

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkError(msg) => write!(f, "Network error: {msg}"),
            Self::ApiError { status, message } => write!(f, "API error ({status}): {message}"),
            Self::ModelNotFound(model) => write!(f, "Model not found: {model}"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
            Self::RateLimited => write!(f, "Rate limited"),
            Self::Timeout => write!(f, "Request timeout"),
            Self::ConfigError(msg) => write!(f, "Config error: {msg}"),
        }
    }
}

impl std::error::Error for EmbeddingError {}

// ---------------------------------------------------------------------------
// ProviderType
// ---------------------------------------------------------------------------

/// プロバイダー種別
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    #[default]
    Ollama,
    #[serde(rename = "openai")]
    OpenAi,
}

// ---------------------------------------------------------------------------
// EmbeddingConfig
// ---------------------------------------------------------------------------

fn default_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_endpoint() -> String {
    "http://localhost:11434".to_string()
}

#[derive(Clone, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default)]
    pub provider: ProviderType,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    pub api_key: Option<String>,
}

impl fmt::Debug for EmbeddingConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .finish()
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::Ollama,
            model: default_model(),
            endpoint: default_endpoint(),
            api_key: None,
        }
    }
}

impl EmbeddingConfig {
    /// APIキーを解決する。環境変数 COMMANDINDEX_OPENAI_API_KEY を優先。
    pub fn resolve_api_key(&self) -> Option<String> {
        std::env::var("COMMANDINDEX_OPENAI_API_KEY")
            .ok()
            .or_else(|| self.api_key.clone())
    }
}

// ---------------------------------------------------------------------------
// Config (top-level config.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub embedding: Option<EmbeddingConfig>,
}

impl Config {
    /// `.commandindex/config.toml` を読み込む。存在しない場合はNone。
    pub fn load(commandindex_dir: &Path) -> Result<Option<Self>, EmbeddingError> {
        let config_path = commandindex_dir.join("config.toml");
        if !config_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&config_path)
            .map_err(|e| EmbeddingError::ConfigError(format!("Failed to read config.toml: {e}")))?;
        let config: Config = toml::from_str(&content).map_err(|e| {
            EmbeddingError::ConfigError(format!("Failed to parse config.toml: {e}"))
        })?;
        Ok(Some(config))
    }
}

// ---------------------------------------------------------------------------
// Shared utilities for providers
// ---------------------------------------------------------------------------

/// HTTPレスポンスのステータスコードをEmbeddingErrorに変換する。
/// OllamaProvider・OpenAiProvider共通のエラーハンドリング。
pub(crate) fn map_status_to_error(
    status: reqwest::StatusCode,
    model: &str,
    response: reqwest::blocking::Response,
) -> EmbeddingError {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return EmbeddingError::RateLimited;
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return EmbeddingError::ModelNotFound(model.to_string());
    }
    let body = response.text().unwrap_or_default();
    EmbeddingError::ApiError {
        status: status.as_u16(),
        message: body,
    }
}

/// テキストを最大文字数に切り詰める。プロバイダー共通ユーティリティ。
pub(crate) fn truncate_text(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        text.to_string()
    } else {
        text.chars().take(max_length).collect()
    }
}

/// HTTPリクエストエラーをEmbeddingErrorに変換する。
pub(crate) fn map_reqwest_error(e: reqwest::Error) -> EmbeddingError {
    if e.is_timeout() {
        EmbeddingError::Timeout
    } else {
        EmbeddingError::NetworkError(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// 設定からプロバイダーを生成
pub fn create_provider(
    config: &EmbeddingConfig,
) -> Result<Box<dyn EmbeddingProvider>, EmbeddingError> {
    match &config.provider {
        ProviderType::Ollama => Ok(Box::new(ollama::OllamaProvider::from_config(config))),
        ProviderType::OpenAi => Ok(Box::new(openai::OpenAiProvider::from_config(config)?)),
    }
}

// ---------------------------------------------------------------------------
// MockProvider (test only)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub struct MockProvider {
    pub dimension: usize,
    pub model: String,
}

#[cfg(test)]
impl MockProvider {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            model: "mock-model".to_string(),
        }
    }
}

#[cfg(test)]
impl EmbeddingProvider for MockProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts
            .iter()
            .map(|_| vec![0.0_f32; self.dimension])
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn provider_name(&self) -> &str {
        "mock"
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- ProviderType / EmbeddingConfig defaults ---

    #[test]
    fn test_provider_type_default_is_ollama() {
        assert_eq!(ProviderType::default(), ProviderType::Ollama);
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, ProviderType::Ollama);
        assert_eq!(config.model, "nomic-embed-text");
        assert_eq!(config.endpoint, "http://localhost:11434");
        assert!(config.api_key.is_none());
    }

    // --- Debug masking ---

    #[test]
    fn test_embedding_config_debug_masks_api_key() {
        let config = EmbeddingConfig {
            api_key: Some("sk-secret-key-12345".to_string()),
            ..EmbeddingConfig::default()
        };
        let debug_str = format!("{config:?}");
        assert!(!debug_str.contains("sk-secret-key-12345"));
        assert!(debug_str.contains("***"));
    }

    #[test]
    fn test_embedding_config_debug_no_api_key() {
        let config = EmbeddingConfig::default();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("None"));
    }

    // --- TOML parsing ---

    #[test]
    fn test_config_parse_full_toml() {
        let toml_str = r#"
[embedding]
provider = "openai"
model = "text-embedding-3-small"
endpoint = "https://api.openai.com"
api_key = "sk-test"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let emb = config.embedding.unwrap();
        assert_eq!(emb.provider, ProviderType::OpenAi);
        assert_eq!(emb.model, "text-embedding-3-small");
        assert_eq!(emb.endpoint, "https://api.openai.com");
        assert_eq!(emb.api_key, Some("sk-test".to_string()));
    }

    #[test]
    fn test_config_parse_minimal_toml() {
        let toml_str = r#"
[embedding]
provider = "ollama"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let emb = config.embedding.unwrap();
        assert_eq!(emb.provider, ProviderType::Ollama);
        assert_eq!(emb.model, "nomic-embed-text");
        assert_eq!(emb.endpoint, "http://localhost:11434");
        assert!(emb.api_key.is_none());
    }

    #[test]
    fn test_config_parse_no_embedding_section() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.embedding.is_none());
    }

    // --- Config::load ---

    #[test]
    fn test_config_load_nonexistent_returns_none() {
        let tmp = TempDir::new().unwrap();
        let result = Config::load(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_config_load_valid_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
[embedding]
provider = "ollama"
model = "nomic-embed-text"
"#,
        )
        .unwrap();
        let config = Config::load(tmp.path()).unwrap().unwrap();
        let emb = config.embedding.unwrap();
        assert_eq!(emb.provider, ProviderType::Ollama);
    }

    #[test]
    fn test_config_load_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        fs::write(&config_path, "not valid toml {{{{").unwrap();
        let result = Config::load(tmp.path());
        assert!(result.is_err());
    }

    // --- resolve_api_key ---

    #[test]
    fn test_resolve_api_key_from_config() {
        // Remove env var if set
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig {
            api_key: Some("sk-from-config".to_string()),
            ..EmbeddingConfig::default()
        };
        assert_eq!(config.resolve_api_key(), Some("sk-from-config".to_string()));
    }

    #[test]
    fn test_resolve_api_key_env_var_priority() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::set_var("COMMANDINDEX_OPENAI_API_KEY", "sk-from-env") };
        let config = EmbeddingConfig {
            api_key: Some("sk-from-config".to_string()),
            ..EmbeddingConfig::default()
        };
        assert_eq!(config.resolve_api_key(), Some("sk-from-env".to_string()));
        // SAFETY: test-only cleanup
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
    }

    #[test]
    fn test_resolve_api_key_none() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig::default();
        assert!(config.resolve_api_key().is_none());
    }

    // --- MockProvider ---

    #[test]
    fn test_mock_provider_embed() {
        let provider = MockProvider::new(384);
        let texts = vec!["hello".to_string(), "world".to_string()];
        let result = provider.embed(&texts).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 384);
        assert_eq!(result[1].len(), 384);
    }

    #[test]
    fn test_mock_provider_dimension() {
        let provider = MockProvider::new(768);
        assert_eq!(provider.dimension(), 768);
    }

    #[test]
    fn test_mock_provider_names() {
        let provider = MockProvider::new(768);
        assert_eq!(provider.provider_name(), "mock");
        assert_eq!(provider.model_name(), "mock-model");
    }

    // --- EmbeddingError Display ---

    #[test]
    fn test_embedding_error_display() {
        let err = EmbeddingError::NetworkError("connection refused".to_string());
        assert_eq!(format!("{err}"), "Network error: connection refused");

        let err = EmbeddingError::ApiError {
            status: 429,
            message: "too many requests".to_string(),
        };
        assert_eq!(format!("{err}"), "API error (429): too many requests");

        let err = EmbeddingError::ModelNotFound("unknown-model".to_string());
        assert_eq!(format!("{err}"), "Model not found: unknown-model");

        let err = EmbeddingError::RateLimited;
        assert_eq!(format!("{err}"), "Rate limited");

        let err = EmbeddingError::Timeout;
        assert_eq!(format!("{err}"), "Request timeout");
    }

    // --- create_provider factory ---

    #[test]
    fn test_create_provider_ollama() {
        let config = EmbeddingConfig::default();
        let provider = create_provider(&config).unwrap();
        assert_eq!(provider.provider_name(), "ollama");
    }

    #[test]
    fn test_create_provider_openai_without_api_key_fails() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig {
            provider: ProviderType::OpenAi,
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: None,
        };
        let result = create_provider(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_provider_openai_with_api_key() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig {
            provider: ProviderType::OpenAi,
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: Some("sk-test-key".to_string()),
        };
        let provider = create_provider(&config).unwrap();
        assert_eq!(provider.provider_name(), "openai");
    }
}

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{
    EmbeddingConfig, EmbeddingError, EmbeddingProvider, map_reqwest_error, map_status_to_error,
    truncate_text,
};

/// Maximum number of texts per request batch for OpenAI.
const BATCH_SIZE: usize = 100;
/// Maximum text length (characters) before truncation.
const MAX_TEXT_LENGTH: usize = 32000;
/// Connect timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 10;
/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OpenAiEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// OpenAiProvider
// ---------------------------------------------------------------------------

/// Embedding provider for OpenAI API (and compatible endpoints like Azure OpenAI).
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    endpoint: String,
    client: reqwest::blocking::Client,
    cached_dimension: OnceLock<usize>,
}

impl std::fmt::Debug for OpenAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiProvider")
            .field("api_key", &"***")
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl OpenAiProvider {
    /// Create a new OpenAiProvider.
    pub fn new(api_key: &str, model: &str, endpoint: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            endpoint: endpoint.to_string(),
            client,
            cached_dimension: OnceLock::new(),
        }
    }

    /// Create from EmbeddingConfig. Returns error if api_key is not available.
    pub fn from_config(config: &EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let api_key = config.resolve_api_key().ok_or_else(|| {
            EmbeddingError::ConfigError(
                "OpenAI API key is required. Set COMMANDINDEX_OPENAI_API_KEY environment variable or api_key in config.toml".to_string(),
            )
        })?;
        Ok(Self::new(&api_key, &config.model, &config.endpoint))
    }

    /// Known model-to-dimension mappings.
    fn known_dimension(model: &str) -> Option<usize> {
        match model {
            "text-embedding-3-small" => Some(1536),
            "text-embedding-3-large" => Some(3072),
            "text-embedding-ada-002" => Some(1536),
            _ => None,
        }
    }
}

impl EmbeddingProvider for OpenAiProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(BATCH_SIZE) {
            let truncated: Vec<String> = chunk
                .iter()
                .map(|t| truncate_text(t, MAX_TEXT_LENGTH))
                .collect();

            let request = OpenAiEmbedRequest {
                model: self.model.clone(),
                input: truncated,
            };

            let url = format!("{}/v1/embeddings", self.endpoint);
            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request)
                .send()
                .map_err(map_reqwest_error)?;

            let status = response.status();
            if !status.is_success() {
                return Err(map_status_to_error(status, &self.model, response));
            }

            let body: OpenAiEmbedResponse = response.json().map_err(|e| {
                EmbeddingError::InvalidResponse(format!("Failed to parse response: {e}"))
            })?;

            // Cache the dimension from the first response.
            if let Some(first) = body.data.first() {
                let _ = self.cached_dimension.set(first.embedding.len());
            }

            for item in body.data {
                all_embeddings.push(item.embedding);
            }
        }

        Ok(all_embeddings)
    }

    fn dimension(&self) -> usize {
        if let Some(&dim) = self.cached_dimension.get() {
            return dim;
        }
        Self::known_dimension(&self.model).unwrap_or(0)
    }

    fn provider_name(&self) -> &str {
        "openai"
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
    use crate::embedding::{EmbeddingConfig, ProviderType};

    #[test]
    fn test_openai_provider_debug_masks_api_key() {
        let provider = OpenAiProvider::new(
            "sk-super-secret-key-12345",
            "text-embedding-3-small",
            "https://api.openai.com",
        );
        let debug_str = format!("{provider:?}");
        assert!(!debug_str.contains("sk-super-secret-key-12345"));
        assert!(debug_str.contains("***"));
        assert!(debug_str.contains("text-embedding-3-small"));
    }

    #[test]
    fn test_from_config_no_api_key_fails() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig {
            provider: ProviderType::OpenAi,
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: None,
        };
        let result = OpenAiProvider::from_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::ConfigError(msg) => {
                assert!(msg.contains("API key is required"));
            }
            other => panic!("Expected ConfigError, got: {other}"),
        }
    }

    #[test]
    fn test_from_config_with_api_key() {
        // SAFETY: test-only, single-threaded test execution via cargo test
        unsafe { std::env::remove_var("COMMANDINDEX_OPENAI_API_KEY") };
        let config = EmbeddingConfig {
            provider: ProviderType::OpenAi,
            model: "text-embedding-3-small".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: Some("sk-test-key".to_string()),
        };
        let provider = OpenAiProvider::from_config(&config).unwrap();
        assert_eq!(provider.model, "text-embedding-3-small");
        assert_eq!(provider.endpoint, "https://api.openai.com");
        assert_eq!(provider.api_key, "sk-test-key");
        assert_eq!(provider.provider_name(), "openai");
        assert_eq!(provider.model_name(), "text-embedding-3-small");
    }

    #[test]
    fn test_dimension_known_models() {
        let provider = OpenAiProvider::new(
            "sk-test",
            "text-embedding-3-small",
            "https://api.openai.com",
        );
        assert_eq!(provider.dimension(), 1536);

        let provider = OpenAiProvider::new(
            "sk-test",
            "text-embedding-3-large",
            "https://api.openai.com",
        );
        assert_eq!(provider.dimension(), 3072);

        let provider = OpenAiProvider::new(
            "sk-test",
            "text-embedding-ada-002",
            "https://api.openai.com",
        );
        assert_eq!(provider.dimension(), 1536);
    }

    #[test]
    fn test_dimension_unknown_model() {
        let provider = OpenAiProvider::new("sk-test", "unknown-model", "https://api.openai.com");
        assert_eq!(provider.dimension(), 0);
    }

    #[test]
    fn test_truncate_text_short() {
        let text = "short text";
        assert_eq!(truncate_text(text, MAX_TEXT_LENGTH), "short text");
    }

    #[test]
    fn test_truncate_text_long() {
        let text = "a".repeat(MAX_TEXT_LENGTH + 100);
        let result = truncate_text(&text, MAX_TEXT_LENGTH);
        assert_eq!(result.len(), MAX_TEXT_LENGTH);
    }
}

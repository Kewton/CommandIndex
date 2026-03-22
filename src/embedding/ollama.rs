use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{
    EmbeddingConfig, EmbeddingError, EmbeddingProvider, map_reqwest_error, map_status_to_error,
    truncate_text,
};

/// Maximum number of texts per request batch for Ollama.
const BATCH_SIZE: usize = 10;
/// Maximum text length (characters) before truncation.
const MAX_TEXT_LENGTH: usize = 8192;
/// Connect timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 10;
/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

// ---------------------------------------------------------------------------
// OllamaProvider
// ---------------------------------------------------------------------------

/// Embedding provider for Ollama (local LLM).
#[derive(Debug)]
pub struct OllamaProvider {
    model: String,
    endpoint: String,
    client: reqwest::blocking::Client,
    cached_dimension: OnceLock<usize>,
}

impl OllamaProvider {
    /// Create a new OllamaProvider.
    pub fn new(model: &str, endpoint: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            model: model.to_string(),
            endpoint: endpoint.to_string(),
            client,
            cached_dimension: OnceLock::new(),
        }
    }

    /// Create from EmbeddingConfig.
    pub fn from_config(config: &EmbeddingConfig) -> Self {
        Self::new(&config.model, &config.endpoint)
    }

    /// Known model-to-dimension mappings.
    fn known_dimension(model: &str) -> Option<usize> {
        match model {
            "nomic-embed-text" => Some(768),
            "all-minilm" => Some(384),
            "mxbai-embed-large" => Some(1024),
            _ => None,
        }
    }
}

impl EmbeddingProvider for OllamaProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(BATCH_SIZE) {
            let truncated: Vec<String> = chunk
                .iter()
                .map(|t| truncate_text(t, MAX_TEXT_LENGTH))
                .collect();

            let request = OllamaEmbedRequest {
                model: self.model.clone(),
                input: truncated,
            };

            let url = format!("{}/api/embed", self.endpoint);
            let response = self
                .client
                .post(&url)
                .json(&request)
                .send()
                .map_err(map_reqwest_error)?;

            let status = response.status();
            if !status.is_success() {
                return Err(map_status_to_error(status, &self.model, response));
            }

            let body: OllamaEmbedResponse = response.json().map_err(|e| {
                EmbeddingError::InvalidResponse(format!("Failed to parse response: {e}"))
            })?;

            // Cache the dimension from the first response.
            if let Some(first) = body.embeddings.first() {
                let _ = self.cached_dimension.set(first.len());
            }

            all_embeddings.extend(body.embeddings);
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
        "ollama"
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
    use crate::embedding::EmbeddingConfig;

    #[test]
    fn test_from_config() {
        let config = EmbeddingConfig {
            model: "nomic-embed-text".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            ..EmbeddingConfig::default()
        };
        let provider = OllamaProvider::from_config(&config);
        assert_eq!(provider.model, "nomic-embed-text");
        assert_eq!(provider.endpoint, "http://localhost:11434");
        assert_eq!(provider.provider_name(), "ollama");
        assert_eq!(provider.model_name(), "nomic-embed-text");
    }

    #[test]
    fn test_dimension_known_models() {
        let provider = OllamaProvider::new("nomic-embed-text", "http://localhost:11434");
        assert_eq!(provider.dimension(), 768);

        let provider = OllamaProvider::new("all-minilm", "http://localhost:11434");
        assert_eq!(provider.dimension(), 384);

        let provider = OllamaProvider::new("mxbai-embed-large", "http://localhost:11434");
        assert_eq!(provider.dimension(), 1024);
    }

    #[test]
    fn test_dimension_unknown_model() {
        let provider = OllamaProvider::new("unknown-model", "http://localhost:11434");
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

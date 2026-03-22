pub mod ollama;

use std::fmt;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// RerankConfig
// ---------------------------------------------------------------------------

fn default_rerank_model() -> String {
    "llama3".to_string()
}

fn default_top_candidates() -> usize {
    20
}

fn default_rerank_endpoint() -> String {
    "http://localhost:11434".to_string()
}

fn default_timeout_secs() -> u64 {
    30
}

#[derive(Clone, Deserialize)]
pub struct RerankConfig {
    #[serde(default = "default_rerank_model")]
    pub model: String,
    #[serde(default = "default_top_candidates")]
    pub top_candidates: usize,
    #[serde(default = "default_rerank_endpoint")]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

impl fmt::Debug for RerankConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RerankConfig")
            .field("model", &self.model)
            .field("top_candidates", &self.top_candidates)
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            model: default_rerank_model(),
            top_candidates: default_top_candidates(),
            endpoint: default_rerank_endpoint(),
            api_key: None,
            timeout_secs: default_timeout_secs(),
        }
    }
}

// ---------------------------------------------------------------------------
// RerankCandidate / RerankResult
// ---------------------------------------------------------------------------

pub struct RerankCandidate {
    pub document_text: String,
    pub original_index: usize,
}

pub struct RerankResult {
    pub index: usize,
    pub score: f32,
}

// ---------------------------------------------------------------------------
// RerankError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum RerankError {
    NetworkError(String),
    ApiError { status: u16, message: String },
    ModelNotFound(String),
    InvalidResponse(String),
    Timeout,
    ConfigError(String),
}

impl fmt::Display for RerankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkError(msg) => write!(f, "Network error: {msg}"),
            Self::ApiError { status, message } => write!(f, "API error ({status}): {message}"),
            Self::ModelNotFound(model) => write!(f, "Model not found: {model}"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {msg}"),
            Self::Timeout => write!(f, "Request timeout"),
            Self::ConfigError(msg) => write!(f, "Config error: {msg}"),
        }
    }
}

impl std::error::Error for RerankError {}

// ---------------------------------------------------------------------------
// RerankProvider trait
// ---------------------------------------------------------------------------

pub trait RerankProvider {
    fn rerank(
        &self,
        query: &str,
        documents: &[RerankCandidate],
    ) -> Result<Vec<RerankResult>, RerankError>;
}

// ---------------------------------------------------------------------------
// build_document_text
// ---------------------------------------------------------------------------

const MAX_DOCUMENT_TEXT_LENGTH: usize = 4096;

pub fn build_document_text(heading: &str, body: &str) -> String {
    let combined = if heading.is_empty() {
        body.to_string()
    } else {
        format!("{heading}\n{body}")
    };
    combined.chars().take(MAX_DOCUMENT_TEXT_LENGTH).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_document_text_normal() {
        let result = build_document_text("Introduction", "This is the body text.");
        assert_eq!(result, "Introduction\nThis is the body text.");
    }

    #[test]
    fn test_build_document_text_empty_heading() {
        let result = build_document_text("", "Body only.");
        assert_eq!(result, "Body only.");
    }

    #[test]
    fn test_build_document_text_truncate_long_text() {
        let heading = "H";
        let body = "a".repeat(5000);
        let result = build_document_text(heading, &body);
        assert_eq!(result.chars().count(), MAX_DOCUMENT_TEXT_LENGTH);
        assert!(result.starts_with("H\n"));
    }

    #[test]
    fn test_build_document_text_japanese() {
        let result = build_document_text("見出し", "本文テキスト");
        assert_eq!(result, "見出し\n本文テキスト");
    }

    #[test]
    fn test_build_document_text_japanese_truncate() {
        let heading = "見出し";
        let body = "あ".repeat(5000);
        let result = build_document_text(heading, &body);
        assert_eq!(result.chars().count(), MAX_DOCUMENT_TEXT_LENGTH);
    }

    #[test]
    fn test_rerank_config_defaults() {
        let config = RerankConfig::default();
        assert_eq!(config.model, "llama3");
        assert_eq!(config.top_candidates, 20);
        assert_eq!(config.endpoint, "http://localhost:11434");
        assert!(config.api_key.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_rerank_config_toml_defaults() {
        let toml_str = "[rerank]";
        #[derive(Deserialize)]
        struct Wrapper {
            rerank: RerankConfig,
        }
        let wrapper: Wrapper = toml::from_str(toml_str).unwrap();
        assert_eq!(wrapper.rerank.model, "llama3");
        assert_eq!(wrapper.rerank.top_candidates, 20);
    }

    #[test]
    fn test_rerank_config_debug_masks_api_key() {
        let config = RerankConfig {
            api_key: Some("sk-secret-rerank-key".to_string()),
            ..RerankConfig::default()
        };
        let debug_str = format!("{config:?}");
        assert!(!debug_str.contains("sk-secret-rerank-key"));
        assert!(debug_str.contains("***"));
    }

    #[test]
    fn test_rerank_config_debug_no_api_key() {
        let config = RerankConfig::default();
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("None"));
    }

    #[test]
    fn test_rerank_error_display() {
        let err = RerankError::NetworkError("conn refused".to_string());
        assert_eq!(format!("{err}"), "Network error: conn refused");

        let err = RerankError::Timeout;
        assert_eq!(format!("{err}"), "Request timeout");
    }
}

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use super::{RerankCandidate, RerankConfig, RerankError, RerankProvider, RerankResult};

const CONNECT_TIMEOUT_SECS: u64 = 10;
const REQUEST_TIMEOUT_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaGenerateOptions,
}

#[derive(Serialize)]
struct OllamaGenerateOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

// ---------------------------------------------------------------------------
// OllamaRerankProvider
// ---------------------------------------------------------------------------

pub struct OllamaRerankProvider {
    client: Client,
    model: String,
    endpoint: String,
    timeout_secs: u64,
}

impl OllamaRerankProvider {
    pub fn new(config: &RerankConfig) -> Result<Self, RerankError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| RerankError::ConfigError(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            model: config.model.clone(),
            endpoint: config.endpoint.clone(),
            timeout_secs: config.timeout_secs,
        })
    }
}

impl RerankProvider for OllamaRerankProvider {
    fn rerank(
        &self,
        query: &str,
        documents: &[RerankCandidate],
    ) -> Result<Vec<RerankResult>, RerankError> {
        let deadline = Instant::now() + Duration::from_secs(self.timeout_secs);
        let mut results = Vec::with_capacity(documents.len());
        let url = format!("{}/api/generate", self.endpoint);

        for doc in documents {
            // Check timeout before each request
            if Instant::now() >= deadline {
                eprintln!(
                    "[rerank] Timeout reached after scoring {} of {} candidates",
                    results.len(),
                    documents.len()
                );
                break;
            }

            let prompt = build_prompt(query, &doc.document_text);
            let request = OllamaGenerateRequest {
                model: self.model.clone(),
                prompt,
                stream: false,
                options: OllamaGenerateOptions {
                    temperature: 0.0,
                    num_predict: 10,
                },
            };

            let response = self.client.post(&url).json(&request).send().map_err(|e| {
                if e.is_timeout() {
                    RerankError::Timeout
                } else {
                    RerankError::NetworkError(e.to_string())
                }
            })?;

            let status = response.status();
            if !status.is_success() {
                if status == reqwest::StatusCode::NOT_FOUND {
                    return Err(RerankError::ModelNotFound(self.model.clone()));
                }
                let body = response.text().unwrap_or_default();
                let truncated = if body.len() > 500 {
                    format!("{}...(truncated)", &body[..500])
                } else {
                    body
                };
                return Err(RerankError::ApiError {
                    status: status.as_u16(),
                    message: truncated,
                });
            }

            let body: OllamaGenerateResponse = response.json().map_err(|e| {
                RerankError::InvalidResponse(format!("Failed to parse response: {e}"))
            })?;

            let score = parse_score(&body.response);

            results.push(RerankResult {
                index: doc.original_index,
                score,
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn build_prompt(query: &str, document_text: &str) -> String {
    format!(
        "Rate the relevance of the following document to the query on a scale of 0 to 10.\n\
         Only respond with a single number.\n\n\
         Query: {query}\n\n\
         Document:\n```\n{document_text}\n```\n\n\
         Relevance score (0-10):"
    )
}

fn parse_score(response: &str) -> f32 {
    let trimmed = response.trim();
    // Try direct parse first
    if let Ok(score) = trimmed.parse::<f32>() {
        return score.clamp(0.0, 10.0);
    }
    // Extract the first number from the response (handles "8/10", "Score: 8", etc.)
    let mut number_buf = String::new();
    let mut found_digit = false;
    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            found_digit = true;
            number_buf.push(c);
        } else if c == '.' && found_digit && !number_buf.contains('.') {
            number_buf.push(c);
        } else if found_digit {
            break;
        }
    }
    // Remove trailing dot (e.g. "8." -> "8")
    if number_buf.ends_with('.') {
        number_buf.pop();
    }
    if let Ok(score) = number_buf.parse::<f32>() {
        return score.clamp(0.0, 10.0);
    }
    0.0
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub fn create_rerank_provider(
    config: &RerankConfig,
) -> Result<Box<dyn RerankProvider>, RerankError> {
    OllamaRerankProvider::new(config).map(|p| Box::new(p) as Box<dyn RerankProvider>)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_contains_query_and_document() {
        let prompt = build_prompt("how to install", "Run npm install to get started.");
        assert!(prompt.contains("how to install"));
        assert!(prompt.contains("Run npm install to get started."));
        assert!(prompt.contains("0 to 10"));
        assert!(prompt.contains("```"));
    }

    #[test]
    fn test_build_prompt_format() {
        let prompt = build_prompt("test query", "test doc");
        assert!(prompt.starts_with("Rate the relevance"));
        assert!(prompt.contains("Query: test query"));
        assert!(prompt.contains("Document:"));
        assert!(prompt.ends_with("Relevance score (0-10):"));
    }

    #[test]
    fn test_parse_score_integer() {
        assert!((parse_score("8") - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_float() {
        assert!((parse_score("8.5") - 8.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_invalid() {
        assert!((parse_score("invalid") - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_empty() {
        assert!((parse_score("") - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_with_whitespace() {
        assert!((parse_score("  7.5  ") - 7.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_clamp_high() {
        assert!((parse_score("15") - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_clamp_negative() {
        assert!((parse_score("-3") - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_fraction_format() {
        // "8/10" should extract 8
        assert!((parse_score("8/10") - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_labeled_format() {
        // "Score: 8" should extract 8
        assert!((parse_score("Score: 8") - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_labeled_float() {
        assert!((parse_score("Score: 7.5") - 7.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_score_with_text_after() {
        assert!((parse_score("8 out of 10") - 8.0).abs() < f32::EPSILON);
    }
}

//! Ollama provider implementation (stub -- completed in Task 2).
//!
//! Supports the Ollama local inference API (`/api/chat`) with no authentication.

use async_trait::async_trait;

use crate::error::AiError;
use crate::provider::AiProvider;
use crate::types::{CompletionRequest, CompletionResponse, ProviderCapabilities};

/// An LLM provider that communicates with the Ollama local inference server.
///
/// Sends requests to `/api/chat` with no authentication required.
pub struct OllamaProvider {
    _client: reqwest::Client,
    _base_url: String,
    _model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Custom base URL, defaults to `http://localhost:11434`
    /// * `model` - Model identifier (e.g., "llama3.2")
    /// * `timeout` - HTTP request timeout
    pub fn new(base_url: Option<String>, model: String, timeout: std::time::Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build reqwest client");

        Self {
            _client: client,
            _base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            _model: model,
        }
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn complete(
        &self,
        _request: &CompletionRequest,
    ) -> Result<CompletionResponse, AiError> {
        // Stub implementation -- completed in Task 2
        todo!("OllamaProvider::complete will be implemented in Task 2")
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: false,
            max_context_tokens: 32_000,
        }
    }

    fn name(&self) -> &str {
        "ollama"
    }
}

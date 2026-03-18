//! Ollama provider implementation.
//!
//! Supports the Ollama local inference API (`/api/chat`) with no authentication
//! required. Ollama runs as a local service (default: `http://localhost:11434`).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AiError;
use crate::provider::AiProvider;
use crate::types::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ProviderCapabilities,
    ResponseFormat, Role, Usage,
};

/// An LLM provider that communicates with the Ollama local inference server.
///
/// Sends requests to `/api/chat` with no authentication required.
/// Always uses non-streaming mode (`stream: false`).
pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
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
            client,
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            model,
        }
    }
}

// --- Internal request/response types (private, serde) ---

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
    model: String,
    done: bool,
    eval_count: Option<u32>,
    prompt_eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}

/// Map our Message to Ollama's message format.
fn map_message(msg: &Message) -> OllamaMessage {
    OllamaMessage {
        role: match msg.role {
            Role::System => "system".to_string(),
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
        },
        content: msg.content.clone(),
    }
}

/// Map our ResponseFormat to Ollama's format field.
fn map_response_format(format: &ResponseFormat) -> serde_json::Value {
    match format {
        ResponseFormat::Json => serde_json::json!({"type": "object"}),
        ResponseFormat::JsonSchema(schema) => schema.clone(),
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, AiError> {
        // Build messages: system prompt first, then conversation messages
        let mut messages = Vec::with_capacity(request.messages.len() + 1);
        messages.push(OllamaMessage {
            role: "system".to_string(),
            content: request.system_prompt.clone(),
        });
        for msg in &request.messages {
            messages.push(map_message(msg));
        }

        let format = request.response_format.as_ref().map(map_response_format);

        let body = OllamaRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            format,
            options: Some(OllamaOptions {
                temperature: request.temperature,
                num_predict: request.max_tokens,
            }),
        };

        let url = format!("{}/api/chat", self.base_url);

        let response = self.client.post(&url).json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            return Err(AiError::ProviderError {
                status: status.as_u16(),
                body: body_text,
            });
        }

        let response_text = response.text().await?;
        let parsed: OllamaResponse = serde_json::from_str(&response_text)
            .map_err(|e| AiError::ParseError(format!("Ollama response parse error: {e}")))?;

        let finish_reason = if parsed.done {
            FinishReason::Stop
        } else {
            FinishReason::Other("incomplete".to_string())
        };

        Ok(CompletionResponse {
            content: parsed.message.content,
            model: parsed.model,
            usage: Usage {
                prompt_tokens: parsed.prompt_eval_count.unwrap_or(0),
                completion_tokens: parsed.eval_count.unwrap_or(0),
            },
            finish_reason,
        })
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

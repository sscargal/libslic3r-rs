//! Anthropic provider implementation.
//!
//! Supports the Anthropic Messages API (`/v1/messages`) with `x-api-key`
//! header authentication. Compatible with Claude models.

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::error::AiError;
use crate::provider::AiProvider;
use crate::types::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ProviderCapabilities,
    ResponseFormat, Role, Usage,
};

/// An LLM provider that communicates with the Anthropic Messages API.
///
/// Sends requests to `/v1/messages` using `x-api-key` header authentication
/// and the `anthropic-version` header.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: SecretString,
    model: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key (sent via `x-api-key` header)
    /// * `model` - Model identifier (e.g., "claude-sonnet-4-20250514")
    /// * `timeout` - HTTP request timeout
    pub fn new(api_key: SecretString, model: String, timeout: std::time::Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            api_key,
            model,
        }
    }
}

// --- Internal request/response types (private, serde) ---

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Map our Message to Anthropic's message format.
///
/// Filters out System messages since Anthropic uses a top-level `system` field
/// instead of system messages in the messages array.
fn map_message(msg: &Message) -> Option<AnthropicMessage> {
    match msg.role {
        Role::System => None, // System handled separately
        Role::User => Some(AnthropicMessage {
            role: "user".to_string(),
            content: msg.content.clone(),
        }),
        Role::Assistant => Some(AnthropicMessage {
            role: "assistant".to_string(),
            content: msg.content.clone(),
        }),
    }
}

/// Map Anthropic's stop_reason to our FinishReason enum.
fn map_finish_reason(reason: Option<&str>) -> FinishReason {
    match reason {
        Some("end_turn") => FinishReason::Stop,
        Some("max_tokens") => FinishReason::Length,
        Some(other) => FinishReason::Other(other.to_string()),
        None => FinishReason::Other("unknown".to_string()),
    }
}

/// Build the system prompt, appending JSON instruction if JSON response format is requested.
///
/// Anthropic does not have a native `response_format` field. Instead, we
/// instruct the model to respond with JSON via the system prompt.
fn build_system_prompt(system_prompt: &str, response_format: Option<&ResponseFormat>) -> String {
    match response_format {
        Some(ResponseFormat::Json) => {
            format!(
                "{system_prompt}\n\nIMPORTANT: Respond only with valid JSON. Do not include any text outside the JSON object."
            )
        }
        Some(ResponseFormat::JsonSchema(schema)) => {
            format!(
                "{system_prompt}\n\nIMPORTANT: Respond only with valid JSON matching this schema: {schema}"
            )
        }
        None => system_prompt.to_string(),
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, AiError> {
        let system = build_system_prompt(
            &request.system_prompt,
            request.response_format.as_ref(),
        );

        // Only include user/assistant messages (not system)
        let messages: Vec<AnthropicMessage> = request
            .messages
            .iter()
            .filter_map(map_message)
            .collect();

        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens,
            system,
            messages,
            temperature: Some(request.temperature),
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

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
        let parsed: AnthropicResponse = serde_json::from_str(&response_text)
            .map_err(|e| AiError::ParseError(format!("Anthropic response parse error: {e}")))?;

        let first_block = parsed.content.first().ok_or(AiError::EmptyResponse)?;

        Ok(CompletionResponse {
            content: first_block.text.clone(),
            model: parsed.model,
            usage: Usage {
                prompt_tokens: parsed.usage.input_tokens,
                completion_tokens: parsed.usage.output_tokens,
            },
            finish_reason: map_finish_reason(parsed.stop_reason.as_deref()),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: true,
            max_context_tokens: 200_000,
        }
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

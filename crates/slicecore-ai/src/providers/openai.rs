//! OpenAI provider implementation.
//!
//! Supports the OpenAI Chat Completions API (`/v1/chat/completions`) with
//! Bearer token authentication. Compatible with GPT-4, GPT-4o, and other
//! models available through the OpenAI API.

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::error::AiError;
use crate::provider::AiProvider;
use crate::types::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ProviderCapabilities,
    ResponseFormat, Role, Usage,
};

/// An LLM provider that communicates with the OpenAI Chat Completions API.
///
/// Sends requests to `/v1/chat/completions` using Bearer token authentication.
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: SecretString,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key (Bearer token)
    /// * `model` - Model identifier (e.g., "gpt-4o", "gpt-4-turbo")
    /// * `base_url` - Custom base URL, defaults to `https://api.openai.com`
    /// * `timeout` - HTTP request timeout
    pub fn new(
        api_key: SecretString,
        model: String,
        base_url: Option<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }
}

// --- Internal request/response types (private, serde) ---

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<OpenAiResponseFormat>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    model: String,
    usage: OpenAiUsage,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
    finish_reason: String,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

/// Map our ResponseFormat to OpenAI's format type.
fn map_response_format(format: &ResponseFormat) -> OpenAiResponseFormat {
    match format {
        ResponseFormat::Json => OpenAiResponseFormat {
            format_type: "json_object".to_string(),
        },
        ResponseFormat::JsonSchema(_) => OpenAiResponseFormat {
            format_type: "json_object".to_string(),
        },
    }
}

/// Map our Message to OpenAI's message format.
fn map_message(msg: &Message) -> OpenAiMessage {
    OpenAiMessage {
        role: match msg.role {
            Role::System => "system".to_string(),
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
        },
        content: msg.content.clone(),
    }
}

/// Map OpenAI's finish_reason string to our FinishReason enum.
fn map_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "stop" => FinishReason::Stop,
        "length" => FinishReason::Length,
        other => FinishReason::Other(other.to_string()),
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, AiError> {
        // Build messages: system prompt first, then conversation messages
        let mut messages = Vec::with_capacity(request.messages.len() + 1);
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: request.system_prompt.clone(),
        });
        for msg in &request.messages {
            messages.push(map_message(msg));
        }

        let body = OpenAiRequest {
            model: self.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            response_format: request.response_format.as_ref().map(map_response_format),
        };

        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(self.api_key.expose_secret())
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
        let parsed: OpenAiResponse = serde_json::from_str(&response_text)
            .map_err(|e| AiError::ParseError(format!("OpenAI response parse error: {e}")))?;

        let choice = parsed.choices.first().ok_or(AiError::EmptyResponse)?;

        Ok(CompletionResponse {
            content: choice.message.content.clone(),
            model: parsed.model,
            usage: Usage {
                prompt_tokens: parsed.usage.prompt_tokens,
                completion_tokens: parsed.usage.completion_tokens,
            },
            finish_reason: map_finish_reason(&choice.finish_reason),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: true,
            max_context_tokens: 128_000,
        }
    }

    fn name(&self) -> &str {
        "openai"
    }
}

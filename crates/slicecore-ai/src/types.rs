//! Request and response types for LLM interactions.
//!
//! These types model the common interface across all LLM providers. Each
//! provider implementation translates these types to/from their specific
//! API format.

use serde::{Deserialize, Serialize};

/// A request to generate a completion from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// System-level instructions for the model's behavior.
    pub system_prompt: String,

    /// The conversation messages to send.
    pub messages: Vec<Message>,

    /// Sampling temperature (0.0 = deterministic, 2.0 = creative).
    pub temperature: f32,

    /// Maximum number of tokens to generate in the response.
    pub max_tokens: u32,

    /// Optional response format constraint (e.g., JSON mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

/// The response from an LLM completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The generated text content.
    pub content: String,

    /// The model identifier that produced the response.
    pub model: String,

    /// Token usage statistics.
    pub usage: Usage,

    /// The reason the model stopped generating.
    pub finish_reason: FinishReason,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender.
    pub role: Role,

    /// The text content of the message.
    pub content: String,
}

/// The role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System-level instructions.
    System,
    /// User input.
    User,
    /// Assistant (model) output.
    Assistant,
}

/// Constraints on the format of the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Request the model to respond with valid JSON.
    Json,
    /// Request the model to respond with JSON matching a specific schema.
    JsonSchema(serde_json::Value),
}

/// Token usage statistics for a completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,

    /// Number of tokens in the generated response.
    pub completion_tokens: u32,
}

/// The reason the model stopped generating tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// Natural stop (end of response).
    Stop,
    /// Reached the max_tokens limit.
    Length,
    /// Provider-specific or unknown reason.
    Other(String),
}

/// Capabilities of an LLM provider.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Whether the provider supports structured JSON output mode.
    pub supports_structured_output: bool,

    /// Whether the provider supports streaming responses.
    pub supports_streaming: bool,

    /// Maximum context window size in tokens.
    pub max_context_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_serde_roundtrip() {
        let roles = [Role::System, Role::User, Role::Assistant];
        for role in &roles {
            let json = serde_json::to_string(role).unwrap();
            let deserialized: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(*role, deserialized);
        }
    }

    #[test]
    fn finish_reason_serde_roundtrip() {
        let reasons = [
            FinishReason::Stop,
            FinishReason::Length,
            FinishReason::Other("content_filter".to_string()),
        ];
        for reason in &reasons {
            let json = serde_json::to_string(reason).unwrap();
            let deserialized: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(*reason, deserialized);
        }
    }

    #[test]
    fn message_construction() {
        let msg = Message {
            role: Role::User,
            content: "Hello".to_string(),
        };
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn completion_request_json_format() {
        let request = CompletionRequest {
            system_prompt: "You are helpful.".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: "Hi".to_string(),
            }],
            temperature: 0.7,
            max_tokens: 1024,
            response_format: Some(ResponseFormat::Json),
        };
        let json = serde_json::to_value(&request).unwrap();
        // f32 -> f64 conversion introduces floating-point imprecision
        assert!(
            json["temperature"].as_f64().unwrap() > 0.69
                && json["temperature"].as_f64().unwrap() < 0.71
        );
        assert_eq!(json["max_tokens"], 1024);
        assert!(json["response_format"].is_object());
    }

    #[test]
    fn usage_copy_semantics() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
        };
        let copy = usage;
        assert_eq!(copy.prompt_tokens, 100);
        assert_eq!(copy.completion_tokens, 50);
    }

    #[test]
    fn provider_capabilities_defaults() {
        let caps = ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: false,
            max_context_tokens: 128_000,
        };
        assert!(caps.supports_structured_output);
        assert!(!caps.supports_streaming);
        assert_eq!(caps.max_context_tokens, 128_000);
    }
}

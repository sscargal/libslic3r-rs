//! Error types for the AI integration crate.
//!
//! [`AiError`] covers all failure modes that can occur when interacting with
//! LLM providers: network errors, authentication failures, response parsing
//! issues, and validation problems.

use thiserror::Error;

/// Errors that can occur during AI provider operations.
#[derive(Debug, Error)]
pub enum AiError {
    /// Network or connection failure when communicating with the provider.
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// The provider returned a non-2xx HTTP response.
    #[error("Provider returned error (HTTP {status}): {body}")]
    ProviderError {
        /// HTTP status code from the provider.
        status: u16,
        /// Response body text.
        body: String,
    },

    /// Failed to parse the provider's response body.
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// The LLM returned content that is not valid JSON or does not match
    /// the expected schema.
    #[error("Invalid JSON in LLM response: {0}")]
    InvalidJsonResponse(String),

    /// The provider returned an empty response (no choices or content).
    #[error("Provider returned empty response with no content")]
    EmptyResponse,

    /// A required API key was not provided for a provider that needs one.
    #[error("Missing API key for provider: {0}")]
    MissingApiKey(&'static str),

    /// Failed to create or use the async runtime.
    #[error("Runtime error: {0}")]
    RuntimeError(String),

    /// A value in the response was outside the valid range.
    #[error("Validation error: {0}")]
    ValidationError(String),
}

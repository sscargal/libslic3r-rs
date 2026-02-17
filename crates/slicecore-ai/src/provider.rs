//! The [`AiProvider`] trait defines the interface for LLM providers.
//!
//! All provider implementations (OpenAI, Anthropic, Ollama) implement this
//! trait, enabling provider-agnostic code throughout the AI integration layer.
//!
//! Uses `async_trait` to enable dynamic dispatch (`Box<dyn AiProvider>`),
//! which is not yet supported with native `async fn` in traits.

use crate::error::AiError;
use crate::types::{CompletionRequest, CompletionResponse, ProviderCapabilities};

/// A provider-agnostic interface for LLM completion requests.
///
/// Implementations handle the specifics of each provider's API format,
/// authentication, and response parsing.
///
/// # Example
///
/// ```rust,ignore
/// use slicecore_ai::{AiProvider, CompletionRequest, CompletionResponse};
///
/// async fn ask(provider: &dyn AiProvider, request: &CompletionRequest) {
///     let response = provider.complete(request).await.unwrap();
///     println!("Response: {}", response.content);
/// }
/// ```
#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    /// Send a completion request to the LLM and await a response.
    ///
    /// # Errors
    ///
    /// Returns [`AiError`] on network failures, authentication errors,
    /// response parsing failures, or empty responses.
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, AiError>;

    /// Return the capabilities of this provider (structured output, streaming, context size).
    fn capabilities(&self) -> ProviderCapabilities;

    /// Return a human-readable name for this provider (e.g., "openai", "anthropic", "ollama").
    fn name(&self) -> &str;
}

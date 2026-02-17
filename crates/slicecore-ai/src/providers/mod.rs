//! LLM provider implementations and factory function.
//!
//! This module contains concrete implementations of the [`AiProvider`] trait
//! for OpenAI, Anthropic, and Ollama backends. The [`create_provider`] factory
//! function constructs the correct provider from an [`AiConfig`].

pub mod anthropic;
pub mod ollama;
pub mod openai;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;

use crate::{AiConfig, AiError, AiProvider, ProviderType};
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;

/// Create an [`AiProvider`] implementation from the given configuration.
///
/// Dispatches on [`ProviderType`] to construct the correct provider:
///
/// - [`ProviderType::OpenAi`] -- requires `api_key` in config
/// - [`ProviderType::Anthropic`] -- requires `api_key` in config
/// - [`ProviderType::Ollama`] -- no API key required (local service)
///
/// # Errors
///
/// Returns [`AiError::MissingApiKey`] if OpenAI or Anthropic is selected
/// without an API key configured.
///
/// # Example
///
/// ```rust,ignore
/// use slicecore_ai::{AiConfig, providers::create_provider};
///
/// let config = AiConfig::default(); // Ollama
/// let provider = create_provider(&config).unwrap();
/// assert_eq!(provider.name(), "ollama");
/// ```
pub fn create_provider(config: &AiConfig) -> Result<Box<dyn AiProvider>, AiError> {
    let timeout = Duration::from_secs(config.timeout_secs);
    match config.provider {
        ProviderType::OpenAi => {
            let api_key = config
                .api_key
                .as_ref()
                .ok_or(AiError::MissingApiKey("OpenAI"))?;
            let key = SecretString::from(api_key.expose_secret().to_string());
            Ok(Box::new(OpenAiProvider::new(
                key,
                config.model.clone(),
                config.base_url.clone(),
                timeout,
            )))
        }
        ProviderType::Anthropic => {
            let api_key = config
                .api_key
                .as_ref()
                .ok_or(AiError::MissingApiKey("Anthropic"))?;
            let key = SecretString::from(api_key.expose_secret().to_string());
            Ok(Box::new(AnthropicProvider::new(
                key,
                config.model.clone(),
                timeout,
            )))
        }
        ProviderType::Ollama => Ok(Box::new(OllamaProvider::new(
            config.base_url.clone(),
            config.model.clone(),
            timeout,
        ))),
    }
}

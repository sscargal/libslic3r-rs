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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AiConfig;

    #[test]
    fn create_provider_openai_missing_api_key() {
        let config = AiConfig {
            provider: ProviderType::OpenAi,
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            timeout_secs: 30,
        };
        let result = create_provider(&config);
        match result {
            Err(AiError::MissingApiKey("OpenAI")) => {} // expected
            Err(e) => panic!("Expected MissingApiKey(\"OpenAI\"), got: {e:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn create_provider_anthropic_missing_api_key() {
        let config = AiConfig {
            provider: ProviderType::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: None,
            base_url: None,
            timeout_secs: 30,
        };
        let result = create_provider(&config);
        match result {
            Err(AiError::MissingApiKey("Anthropic")) => {} // expected
            Err(e) => panic!("Expected MissingApiKey(\"Anthropic\"), got: {e:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn create_provider_ollama_no_api_key_needed() {
        let config = AiConfig {
            provider: ProviderType::Ollama,
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: None,
            timeout_secs: 30,
        };
        let result = create_provider(&config);
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn create_provider_openai_with_api_key() {
        let config = AiConfig {
            provider: ProviderType::OpenAi,
            model: "gpt-4o".to_string(),
            api_key: Some(SecretString::from("sk-test-key".to_string())),
            base_url: None,
            timeout_secs: 30,
        };
        let result = create_provider(&config);
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn create_provider_anthropic_with_api_key() {
        let config = AiConfig {
            provider: ProviderType::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: Some(SecretString::from("sk-ant-test".to_string())),
            base_url: None,
            timeout_secs: 30,
        };
        let result = create_provider(&config);
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn default_config_produces_ollama() {
        let config = AiConfig::default();
        assert_eq!(config.provider, ProviderType::Ollama);
        let result = create_provider(&config);
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn provider_capabilities() {
        let openai_config = AiConfig {
            provider: ProviderType::OpenAi,
            model: "gpt-4o".to_string(),
            api_key: Some(SecretString::from("key".to_string())),
            base_url: None,
            timeout_secs: 30,
        };
        let openai = create_provider(&openai_config).unwrap();
        assert_eq!(openai.capabilities().max_context_tokens, 128_000);
        assert!(openai.capabilities().supports_structured_output);
        assert!(openai.capabilities().supports_streaming);

        let anthropic_config = AiConfig {
            provider: ProviderType::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: Some(SecretString::from("key".to_string())),
            base_url: None,
            timeout_secs: 30,
        };
        let anthropic = create_provider(&anthropic_config).unwrap();
        assert_eq!(anthropic.capabilities().max_context_tokens, 200_000);
        assert!(anthropic.capabilities().supports_structured_output);
        assert!(anthropic.capabilities().supports_streaming);

        let ollama = create_provider(&AiConfig::default()).unwrap();
        assert_eq!(ollama.capabilities().max_context_tokens, 32_000);
        assert!(ollama.capabilities().supports_structured_output);
        assert!(!ollama.capabilities().supports_streaming);
    }
}

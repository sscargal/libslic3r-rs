//! Configuration types for AI provider selection and connection settings.
//!
//! [`AiConfig`] holds all settings needed to connect to an LLM provider,
//! including the provider type, model name, API key, and connection parameters.
//! API keys are wrapped in [`secrecy::SecretString`] to prevent accidental
//! logging or serialization of secrets.

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The LLM provider to use for AI operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// OpenAI API (GPT-4, GPT-4o, etc.).
    OpenAi,
    /// Anthropic API (Claude models).
    Anthropic,
    /// Ollama local inference server.
    Ollama,
}

/// Configuration for connecting to an LLM provider.
///
/// # Security
///
/// The `api_key` field is wrapped in [`SecretString`] to prevent accidental
/// exposure through `Debug` output or logging. The custom [`Debug`]
/// implementation shows `[REDACTED]` for the key value.
///
/// # Example
///
/// ```
/// use slicecore_ai::AiConfig;
///
/// // Default config uses Ollama with llama3.2
/// let config = AiConfig::default();
/// let debug = format!("{:?}", config);
/// assert!(!debug.contains("REDACTED")); // No key set
/// ```
#[derive(Deserialize)]
pub struct AiConfig {
    /// Which LLM provider to use.
    #[serde(default = "default_provider")]
    pub provider: ProviderType,

    /// The model name/identifier (e.g., "gpt-4o", "claude-sonnet-4-20250514", "llama3.2").
    #[serde(default = "default_model")]
    pub model: String,

    /// API key for the provider. Not needed for Ollama (local).
    #[serde(default)]
    pub api_key: Option<SecretString>,

    /// Custom base URL for the provider API. Useful for proxies or self-hosted instances.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_provider() -> ProviderType {
    ProviderType::Ollama
}

fn default_model() -> String {
    "llama3.2".to_string()
}

fn default_timeout_secs() -> u64 {
    30
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            api_key: None,
            base_url: None,
            timeout_secs: default_timeout_secs(),
        }
    }
}

impl Clone for AiConfig {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider,
            model: self.model.clone(),
            api_key: self.api_key.as_ref().map(|key| {
                use secrecy::ExposeSecret;
                SecretString::from(key.expose_secret().to_string())
            }),
            base_url: self.base_url.clone(),
            timeout_secs: self.timeout_secs,
        }
    }
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field(
                "api_key",
                &if self.api_key.is_some() {
                    "[REDACTED]"
                } else {
                    "None"
                },
            )
            .field("base_url", &self.base_url)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

impl AiConfig {
    /// Parse an `AiConfig` from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns a TOML deserialization error if the input is not valid TOML
    /// or does not match the expected configuration schema.
    ///
    /// # Example
    ///
    /// ```
    /// use slicecore_ai::AiConfig;
    ///
    /// let toml = r#"
    /// provider = "open_ai"
    /// model = "gpt-4o"
    /// timeout_secs = 60
    /// "#;
    /// let config = AiConfig::from_toml(toml).unwrap();
    /// assert_eq!(config.model, "gpt-4o");
    /// ```
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = AiConfig::default();
        assert_eq!(config.provider, ProviderType::Ollama);
        assert_eq!(config.model, "llama3.2");
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn debug_redacts_api_key() {
        let config = AiConfig {
            api_key: Some(SecretString::from("sk-super-secret-key-12345".to_string())),
            ..Default::default()
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("[REDACTED]"), "Debug should show [REDACTED]");
        assert!(
            !debug.contains("sk-super-secret"),
            "Debug must not leak API key"
        );
    }

    #[test]
    fn debug_shows_none_when_no_key() {
        let config = AiConfig::default();
        let debug = format!("{:?}", config);
        assert!(
            debug.contains("\"None\""),
            "Debug should show None for absent key"
        );
    }

    #[test]
    fn from_toml_full_config() {
        let toml = r#"
provider = "open_ai"
model = "gpt-4o"
api_key = "sk-test-key"
base_url = "https://custom.api.com"
timeout_secs = 60
"#;
        let config = AiConfig::from_toml(toml).unwrap();
        assert_eq!(config.provider, ProviderType::OpenAi);
        assert_eq!(config.model, "gpt-4o");
        assert!(config.api_key.is_some());
        assert_eq!(
            config.base_url.as_deref(),
            Some("https://custom.api.com")
        );
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn from_toml_minimal_uses_defaults() {
        let toml = "";
        let config = AiConfig::from_toml(toml).unwrap();
        assert_eq!(config.provider, ProviderType::Ollama);
        assert_eq!(config.model, "llama3.2");
        assert!(config.api_key.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn from_toml_anthropic_provider() {
        let toml = r#"
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key = "sk-ant-test"
"#;
        let config = AiConfig::from_toml(toml).unwrap();
        assert_eq!(config.provider, ProviderType::Anthropic);
        assert_eq!(config.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn from_toml_ollama_no_key() {
        let toml = r#"
provider = "ollama"
model = "llama3.2"
base_url = "http://localhost:11434"
"#;
        let config = AiConfig::from_toml(toml).unwrap();
        assert_eq!(config.provider, ProviderType::Ollama);
        assert!(config.api_key.is_none());
        assert_eq!(
            config.base_url.as_deref(),
            Some("http://localhost:11434")
        );
    }

    #[test]
    fn provider_type_serde_roundtrip() {
        let providers = [
            ProviderType::OpenAi,
            ProviderType::Anthropic,
            ProviderType::Ollama,
        ];
        for provider in &providers {
            let json = serde_json::to_string(provider).unwrap();
            let deserialized: ProviderType = serde_json::from_str(&json).unwrap();
            assert_eq!(*provider, deserialized);
        }
    }

    #[test]
    fn clone_preserves_fields() {
        let config = AiConfig {
            provider: ProviderType::OpenAi,
            model: "gpt-4o".to_string(),
            api_key: Some(SecretString::from("test-key".to_string())),
            base_url: Some("https://api.example.com".to_string()),
            timeout_secs: 45,
        };
        let cloned = config.clone();
        assert_eq!(cloned.provider, ProviderType::OpenAi);
        assert_eq!(cloned.model, "gpt-4o");
        assert!(cloned.api_key.is_some());
        assert_eq!(
            cloned.base_url.as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(cloned.timeout_secs, 45);
    }
}

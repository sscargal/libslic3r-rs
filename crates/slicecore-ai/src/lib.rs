//! AI/LLM integration for geometry analysis and print profile suggestions.
//!
//! This crate provides a provider-agnostic interface for interacting with
//! Large Language Models (LLMs) to analyze 3D model geometry and suggest
//! optimal print settings. It supports OpenAI, Anthropic, and Ollama providers.
//!
//! # Architecture
//!
//! - [`AiProvider`] trait: async interface for LLM completion requests
//! - [`AiConfig`]: provider selection, model, API key, connection settings
//! - [`CompletionRequest`] / [`CompletionResponse`]: request/response types
//! - [`AiError`]: comprehensive error handling for all failure modes
//!
//! # Security
//!
//! API keys are stored as [`secrecy::SecretString`] values to prevent
//! accidental logging. The [`AiConfig`] `Debug` implementation redacts
//! key values.
//!
//! # Example
//!
//! ```rust,ignore
//! use slicecore_ai::{AiConfig, AiProvider, CompletionRequest, Message, Role};
//!
//! let config = AiConfig::default(); // Ollama, llama3.2
//! // Provider implementations are in separate plan (08-02)
//! ```

pub mod config;
pub mod error;
pub mod provider;
pub mod types;

pub use config::{AiConfig, ProviderType};
pub use error::AiError;
pub use provider::AiProvider;
pub use types::{
    CompletionRequest, CompletionResponse, FinishReason, Message, ProviderCapabilities,
    ResponseFormat, Role, Usage,
};

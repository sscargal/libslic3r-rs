#![warn(clippy::all, clippy::pedantic, missing_docs)]
//! Setting schema types and registry for the `ConfigSchema` system.
//!
//! This crate provides the foundation types that the `#[derive(ConfigSchema)]`
//! macro generates code against, and that the engine uses at runtime for
//! validation, UI rendering, and JSON Schema generation.

pub mod registry;
pub mod types;

pub use registry::SettingRegistry;
pub use types::{
    Constraint, EnumVariant, HasSettingSchema, SettingCategory, SettingDefinition, SettingKey, Tier,
    ValueType,
};

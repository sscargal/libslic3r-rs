//! Derive macro for the ConfigSchema setting metadata system.
//!
//! Provides `#[derive(SettingSchema)]` which generates `HasSettingSchema`
//! implementations for config structs and enums, producing
//! `Vec<SettingDefinition>` from `#[setting(...)]` field attributes.

#![warn(clippy::all)]

mod codegen;
mod parse;

use proc_macro::TokenStream;

/// Derive `HasSettingSchema` for config structs and enums.
///
/// # Struct attributes
/// - `#[setting(category = "Speed")]` - default category for all fields
///
/// # Field attributes
/// - `#[setting(tier = 1, description = "...", units = "mm/s")]`
/// - `#[setting(min = 0.0, max = 100.0)]`
/// - `#[setting(affects = ["key1", "key2"])]`
/// - `#[setting(depends_on = "other.key")]`
/// - `#[setting(skip)]` - exclude from schema
/// - `#[setting(flatten)]` - delegate to sub-struct
///
/// # Enum variant attributes
/// - `#[setting(display = "Human Name", description = "...")]`
#[proc_macro_derive(SettingSchema, attributes(setting))]
pub fn derive_setting_schema(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match codegen::expand_setting_schema(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

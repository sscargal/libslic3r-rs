//! Code generation for `HasSettingSchema` trait implementations.

use proc_macro2::TokenStream;

/// Expands a `#[derive(SettingSchema)]` into a `HasSettingSchema` impl.
///
/// # Errors
///
/// Returns a `syn::Error` if the input is not a supported struct or enum.
pub fn expand_setting_schema(_input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    // Stub implementation - will be completed in Task 2.
    Ok(TokenStream::new())
}

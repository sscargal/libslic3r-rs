//! Attribute parsing for `#[setting(...)]` on fields, structs, and enum variants.

/// Parsed `#[setting(...)]` attributes from a struct field.
#[derive(Debug, Default)]
pub struct SettingAttrs {
    /// Progressive disclosure tier (0-4).
    pub tier: Option<u8>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Human-readable display name.
    pub display_name: Option<String>,
    /// Setting category name (must match a `SettingCategory` variant).
    pub category: Option<String>,
    /// Unit string for display (e.g., "mm/s").
    pub units: Option<String>,
    /// Minimum value constraint.
    pub min: Option<f64>,
    /// Maximum value constraint.
    pub max: Option<f64>,
    /// Keys of settings this affects.
    pub affects: Vec<String>,
    /// Key of a setting this depends on.
    pub depends_on: Option<String>,
    /// Freeform tags for filtering.
    pub tags: Vec<String>,
    /// Version when this setting was introduced.
    pub since_version: Option<String>,
    /// Deprecation reason/migration guidance.
    pub deprecated: Option<String>,
    /// Whether to skip this field entirely.
    pub skip: bool,
    /// Whether to flatten (delegate to sub-struct).
    pub flatten: bool,
    /// Custom prefix for flattened fields.
    pub prefix: Option<String>,
}

impl SettingAttrs {
    /// Parses `#[setting(...)]` attributes from a list of `syn::Attribute`s.
    ///
    /// # Errors
    ///
    /// Returns a `syn::Error` if an attribute is malformed.
    pub fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut result = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("setting") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tier") {
                    let value = meta.value()?;
                    let lit: syn::LitInt = value.parse()?;
                    result.tier = Some(lit.base10_parse()?);
                } else if meta.path.is_ident("description") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.description = Some(lit.value());
                } else if meta.path.is_ident("display_name") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.display_name = Some(lit.value());
                } else if meta.path.is_ident("category") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.category = Some(lit.value());
                } else if meta.path.is_ident("units") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.units = Some(lit.value());
                } else if meta.path.is_ident("min") {
                    let value = meta.value()?;
                    result.min = Some(parse_f64_lit(value)?);
                } else if meta.path.is_ident("max") {
                    let value = meta.value()?;
                    result.max = Some(parse_f64_lit(value)?);
                } else if meta.path.is_ident("affects") {
                    let value = meta.value()?;
                    result.affects = parse_string_array(value)?;
                } else if meta.path.is_ident("depends_on") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.depends_on = Some(lit.value());
                } else if meta.path.is_ident("tags") {
                    let value = meta.value()?;
                    result.tags = parse_string_array(value)?;
                } else if meta.path.is_ident("since_version") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.since_version = Some(lit.value());
                } else if meta.path.is_ident("deprecated") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.deprecated = Some(lit.value());
                } else if meta.path.is_ident("skip") {
                    result.skip = true;
                } else if meta.path.is_ident("flatten") {
                    result.flatten = true;
                } else if meta.path.is_ident("prefix") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.prefix = Some(lit.value());
                } else {
                    return Err(meta.error(format_args!(
                        "unknown setting attribute `{}`",
                        meta.path
                            .get_ident()
                            .map_or_else(|| "??".to_owned(), ToString::to_string)
                    )));
                }
                Ok(())
            })?;
        }

        Ok(result)
    }
}

/// Parsed struct-level `#[setting(...)]` attributes.
#[derive(Debug, Default)]
pub struct StructAttrs {
    /// Default category for all fields in this struct.
    pub category: Option<String>,
}

impl StructAttrs {
    /// Parses struct-level `#[setting(...)]` attributes.
    ///
    /// # Errors
    ///
    /// Returns a `syn::Error` if an attribute is malformed.
    pub fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut result = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("setting") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("category") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.category = Some(lit.value());
                } else {
                    return Err(meta.error(format_args!(
                        "unknown struct-level setting attribute `{}`",
                        meta.path
                            .get_ident()
                            .map_or_else(|| "??".to_owned(), ToString::to_string)
                    )));
                }
                Ok(())
            })?;
        }

        Ok(result)
    }
}

/// Parsed `#[setting(...)]` attributes from an enum variant.
#[derive(Debug, Default)]
pub struct EnumVariantAttrs {
    /// Human-readable display name for the variant.
    pub display: Option<String>,
    /// Description of what this variant does.
    pub description: Option<String>,
}

impl EnumVariantAttrs {
    /// Parses `#[setting(...)]` attributes on an enum variant.
    ///
    /// # Errors
    ///
    /// Returns a `syn::Error` if an attribute is malformed.
    pub fn from_attrs(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let mut result = Self::default();

        for attr in attrs {
            if !attr.path().is_ident("setting") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("display") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.display = Some(lit.value());
                } else if meta.path.is_ident("description") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    result.description = Some(lit.value());
                } else {
                    return Err(meta.error(format_args!(
                        "unknown enum variant setting attribute `{}`",
                        meta.path
                            .get_ident()
                            .map_or_else(|| "??".to_owned(), ToString::to_string)
                    )));
                }
                Ok(())
            })?;
        }

        Ok(result)
    }
}

/// Parses a numeric literal (int or float) as `f64`.
fn parse_f64_lit(input: syn::parse::ParseStream<'_>) -> syn::Result<f64> {
    let lookahead = input.lookahead1();
    if lookahead.peek(syn::LitFloat) {
        let lit: syn::LitFloat = input.parse()?;
        lit.base10_parse()
    } else if lookahead.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        lit.base10_parse()
    } else {
        Err(lookahead.error())
    }
}

/// Parses `["a", "b", "c"]` as a `Vec<String>`.
fn parse_string_array(input: syn::parse::ParseStream<'_>) -> syn::Result<Vec<String>> {
    let content;
    syn::bracketed!(content in input);
    let items = content.parse_terminated(
        |input: syn::parse::ParseStream<'_>| input.parse::<syn::LitStr>(),
        syn::Token![,],
    )?;
    Ok(items.iter().map(syn::LitStr::value).collect())
}

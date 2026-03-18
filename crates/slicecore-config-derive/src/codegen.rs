//! Code generation for `HasSettingSchema` trait implementations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;

use crate::parse::{EnumVariantAttrs, SettingAttrs, StructAttrs};

/// Expands a `#[derive(SettingSchema)]` into a `HasSettingSchema` impl.
///
/// # Errors
///
/// Returns a `syn::Error` if the input is not a supported struct or enum.
pub fn expand_setting_schema(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        syn::Data::Struct(data) => generate_struct_impl(name, data, &input.attrs)?,
        syn::Data::Enum(data) => generate_enum_impl(name, data)?,
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "SettingSchema cannot be derived for unions",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics ::slicecore_config_schema::HasSettingSchema for #name #ty_generics #where_clause {
            fn setting_definitions(prefix: &str) -> ::std::vec::Vec<::slicecore_config_schema::SettingDefinition> {
                #body
            }
        }
    })
}

/// Generates the body of `setting_definitions` for a struct.
fn generate_struct_impl(
    _name: &syn::Ident,
    data: &syn::DataStruct,
    attrs: &[syn::Attribute],
) -> syn::Result<TokenStream> {
    let struct_attrs = StructAttrs::from_attrs(attrs)?;

    let default_category = match &struct_attrs.category {
        Some(cat) => category_tokens(cat),
        None => quote! { ::slicecore_config_schema::SettingCategory::Advanced },
    };

    let fields = match &data.fields {
        syn::Fields::Named(fields) => &fields.named,
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "SettingSchema can only be derived for structs with named fields",
            ));
        }
    };

    let mut field_stmts = Vec::new();

    for field in fields {
        let field_name = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new(field.span(), "expected named field"))?;
        let field_attrs = SettingAttrs::from_attrs(&field.attrs)?;

        // Skip fields with #[setting(skip)]
        if field_attrs.skip {
            continue;
        }

        // Flatten: delegate to sub-struct
        if field_attrs.flatten {
            let field_ty = &field.ty;
            let field_name_string = field_name.to_string();
            let child_prefix = field_attrs.prefix.as_deref().unwrap_or(&field_name_string);
            let child_prefix_str = child_prefix.to_string();

            field_stmts.push(quote! {
                {
                    let child_prefix = if prefix.is_empty() {
                        #child_prefix_str.to_string()
                    } else {
                        format!("{}.{}", prefix, #child_prefix_str)
                    };
                    defs.extend(<#field_ty as ::slicecore_config_schema::HasSettingSchema>::setting_definitions(&child_prefix));
                }
            });
            continue;
        }

        // Validate min < max at macro expansion time
        if let (Some(min), Some(max)) = (field_attrs.min, field_attrs.max) {
            if min >= max {
                return Err(syn::Error::new(
                    field.span(),
                    format!("min ({min}) must be less than max ({max})"),
                ));
            }
        }

        let field_name_str = field_name.to_string();
        let display_name = field_attrs
            .display_name
            .clone()
            .unwrap_or_else(|| snake_to_title_case(&field_name_str));
        let description = field_attrs.description.as_deref().unwrap_or("");
        let tier = tier_tokens(field_attrs.tier.unwrap_or(4));
        let category = field_attrs
            .category
            .as_ref()
            .map_or_else(|| default_category.clone(), |c| category_tokens(c));
        let value_type = infer_value_type(&field.ty);
        let units = match &field_attrs.units {
            Some(u) => quote! { ::std::option::Option::Some(#u.to_string()) },
            None => quote! { ::std::option::Option::None },
        };
        let since_version = field_attrs.since_version.as_deref().unwrap_or("0.1.0");
        let deprecated = match &field_attrs.deprecated {
            Some(d) => quote! { ::std::option::Option::Some(#d.to_string()) },
            None => quote! { ::std::option::Option::None },
        };

        // Build constraints
        let mut constraint_exprs = Vec::new();
        if let (Some(min), Some(max)) = (field_attrs.min, field_attrs.max) {
            constraint_exprs.push(quote! {
                ::slicecore_config_schema::Constraint::Range { min: #min, max: #max }
            });
        }
        if let Some(dep) = &field_attrs.depends_on {
            constraint_exprs.push(quote! {
                ::slicecore_config_schema::Constraint::DependsOn {
                    key: ::slicecore_config_schema::SettingKey::new(#dep),
                    condition: ::std::string::String::new(),
                }
            });
        }

        // Build affects
        let affects_exprs: Vec<_> = field_attrs
            .affects
            .iter()
            .map(|a| quote! { ::slicecore_config_schema::SettingKey::new(#a) })
            .collect();

        // Build tags
        let tag_exprs: Vec<_> = field_attrs
            .tags
            .iter()
            .map(|t| quote! { #t.to_string() })
            .collect();

        field_stmts.push(quote! {
            {
                let key_str = if prefix.is_empty() {
                    #field_name_str.to_string()
                } else {
                    format!("{}.{}", prefix, #field_name_str)
                };
                defs.push(::slicecore_config_schema::SettingDefinition {
                    key: ::slicecore_config_schema::SettingKey::new(key_str),
                    display_name: #display_name.to_string(),
                    description: #description.to_string(),
                    tier: #tier,
                    category: #category,
                    value_type: #value_type,
                    default_value: ::serde_json::Value::Null,
                    constraints: ::std::vec![#(#constraint_exprs),*],
                    affects: ::std::vec![#(#affects_exprs),*],
                    affected_by: ::std::vec::Vec::new(),
                    units: #units,
                    tags: ::std::vec![#(#tag_exprs),*],
                    since_version: #since_version.to_string(),
                    deprecated: #deprecated,
                });
            }
        });
    }

    Ok(quote! {
        let mut defs = ::std::vec::Vec::new();
        #(#field_stmts)*
        defs
    })
}

/// Generates the body of `setting_definitions` for an enum.
fn generate_enum_impl(_name: &syn::Ident, data: &syn::DataEnum) -> syn::Result<TokenStream> {
    let mut variant_exprs = Vec::new();

    for variant in &data.variants {
        let variant_attrs = EnumVariantAttrs::from_attrs(&variant.attrs)?;
        let value = camel_to_snake_case(&variant.ident.to_string());
        let display = variant_attrs
            .display
            .unwrap_or_else(|| camel_to_spaced(&variant.ident.to_string()));
        let description = variant_attrs.description.unwrap_or_default();

        variant_exprs.push(quote! {
            ::slicecore_config_schema::EnumVariant {
                value: #value.to_string(),
                display: #display.to_string(),
                description: #description.to_string(),
            }
        });
    }

    Ok(quote! {
        let key_str = if prefix.is_empty() {
            ::std::string::String::new()
        } else {
            prefix.to_string()
        };
        ::std::vec![::slicecore_config_schema::SettingDefinition {
            key: ::slicecore_config_schema::SettingKey::new(key_str),
            display_name: ::std::string::String::new(),
            description: ::std::string::String::new(),
            tier: ::slicecore_config_schema::Tier::Developer,
            category: ::slicecore_config_schema::SettingCategory::Advanced,
            value_type: ::slicecore_config_schema::ValueType::Enum {
                variants: ::std::vec![#(#variant_exprs),*],
            },
            default_value: ::serde_json::Value::Null,
            constraints: ::std::vec::Vec::new(),
            affects: ::std::vec::Vec::new(),
            affected_by: ::std::vec::Vec::new(),
            units: ::std::option::Option::None,
            tags: ::std::vec::Vec::new(),
            since_version: "0.1.0".to_string(),
            deprecated: ::std::option::Option::None,
        }]
    })
}

/// Infers `ValueType` from a Rust type.
fn infer_value_type(ty: &syn::Type) -> TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            match ident.as_str() {
                "f64" | "f32" => return quote! { ::slicecore_config_schema::ValueType::Float },
                "bool" => return quote! { ::slicecore_config_schema::ValueType::Bool },
                "i32" | "u32" | "i64" | "u64" | "usize" => {
                    return quote! { ::slicecore_config_schema::ValueType::Int };
                }
                "String" => return quote! { ::slicecore_config_schema::ValueType::String },
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                            if is_f64_type(inner) {
                                return quote! { ::slicecore_config_schema::ValueType::FloatVec };
                            }
                        }
                    }
                    return quote! { ::slicecore_config_schema::ValueType::String };
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                            return infer_value_type(inner);
                        }
                    }
                    return quote! { ::slicecore_config_schema::ValueType::String };
                }
                _ => {}
            }
        }
    }
    // Unknown types default to String
    quote! { ::slicecore_config_schema::ValueType::String }
}

/// Checks if a type is `f64`.
fn is_f64_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "f64";
        }
    }
    false
}

/// Converts a tier number (0-4) to the corresponding `Tier` variant tokens.
fn tier_tokens(tier: u8) -> TokenStream {
    match tier {
        0 => quote! { ::slicecore_config_schema::Tier::AiAuto },
        1 => quote! { ::slicecore_config_schema::Tier::Simple },
        2 => quote! { ::slicecore_config_schema::Tier::Intermediate },
        3 => quote! { ::slicecore_config_schema::Tier::Advanced },
        _ => quote! { ::slicecore_config_schema::Tier::Developer },
    }
}

/// Converts a category string to the corresponding `SettingCategory` variant tokens.
fn category_tokens(category: &str) -> TokenStream {
    match category {
        "Quality" => quote! { ::slicecore_config_schema::SettingCategory::Quality },
        "Speed" => quote! { ::slicecore_config_schema::SettingCategory::Speed },
        "LineWidth" => quote! { ::slicecore_config_schema::SettingCategory::LineWidth },
        "Cooling" => quote! { ::slicecore_config_schema::SettingCategory::Cooling },
        "Retraction" => quote! { ::slicecore_config_schema::SettingCategory::Retraction },
        "Support" => quote! { ::slicecore_config_schema::SettingCategory::Support },
        "Infill" => quote! { ::slicecore_config_schema::SettingCategory::Infill },
        "Adhesion" => quote! { ::slicecore_config_schema::SettingCategory::Adhesion },
        "Machine" => quote! { ::slicecore_config_schema::SettingCategory::Machine },
        "Filament" => quote! { ::slicecore_config_schema::SettingCategory::Filament },
        "Acceleration" => quote! { ::slicecore_config_schema::SettingCategory::Acceleration },
        "PostProcess" => quote! { ::slicecore_config_schema::SettingCategory::PostProcess },
        "Timelapse" => quote! { ::slicecore_config_schema::SettingCategory::Timelapse },
        "MultiMaterial" => quote! { ::slicecore_config_schema::SettingCategory::MultiMaterial },
        "Calibration" => quote! { ::slicecore_config_schema::SettingCategory::Calibration },
        _ => quote! { ::slicecore_config_schema::SettingCategory::Advanced },
    }
}

/// Converts `snake_case` to `Title Case`.
fn snake_to_title_case(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let mut result = c.to_uppercase().to_string();
                    result.extend(chars);
                    result
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Converts `CamelCase` to `snake_case`.
fn camel_to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Converts `CamelCase` to spaced words (e.g., `InnerFirst` -> `Inner First`).
fn camel_to_spaced(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push(' ');
        }
        result.push(c);
    }
    result
}

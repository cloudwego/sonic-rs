//! `SonicDeserialize` derives the official `serde::Deserialize` trait while
//! replacing wide generated field-name matches with a compile-time PHF table.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Result};

mod attr;
mod codegen;
mod model;

#[proc_macro_derive(SonicDeserialize, attributes(serde, sonic))]
pub fn derive_sonic_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: DeriveInput) -> Result<proc_macro2::TokenStream> {
    let model = model::StructModel::from_input(input)?;
    Ok(codegen::expand(&model))
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn assert_rejects(input: DeriveInput, expected: &str) {
        let error = expand(input).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }

    #[test]
    fn rejects_generics_instead_of_guessing_bounds() {
        assert_rejects(
            parse_quote! {
                struct Generic<T> { value: T }
            },
            "non-generic",
        );
    }

    #[test]
    fn rejects_flatten_instead_of_changing_buffering_semantics() {
        assert_rejects(
            parse_quote! {
                struct Flattened {
                    #[serde(flatten)]
                    extra: std::collections::BTreeMap<String, String>,
                }
            },
            "field-level Serde option",
        );
    }

    #[test]
    fn rejects_enums_until_variant_codegen_is_implemented() {
        assert_rejects(
            parse_quote! {
                enum Choice { A, B }
            },
            "does not yet support enums",
        );
    }

    #[test]
    fn rejects_container_options_that_change_field_or_type_semantics() {
        assert_rejects(
            parse_quote! {
                #[serde(rename_all = "camelCase")]
                struct RenameAll { value: i64 }
            },
            "container-level Serde option",
        );
        assert_rejects(
            parse_quote! {
                #[serde(transparent)]
                struct Transparent { value: i64 }
            },
            "container-level Serde option",
        );
        assert_rejects(
            parse_quote! {
                #[serde(from = "i64")]
                struct FromAttr { value: i64 }
            },
            "container-level Serde option",
        );
        assert_rejects(
            parse_quote! {
                #[serde(try_from = "i64")]
                struct TryFromAttr { value: i64 }
            },
            "container-level Serde option",
        );
    }

    #[test]
    fn rejects_field_options_that_need_extra_lifetime_or_buffering_logic() {
        assert_rejects(
            parse_quote! {
                struct Borrowed {
                    #[serde(borrow)]
                    value: std::borrow::Cow<'static, str>,
                }
            },
            "field-level Serde option",
        );
        assert_rejects(
            parse_quote! {
                struct Bounded {
                    #[serde(bound(deserialize = "String: serde::Deserialize<'de>"))]
                    value: String,
                }
            },
            "field-level Serde option",
        );
    }
}

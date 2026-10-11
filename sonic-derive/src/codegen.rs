use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{Ident, LitStr, Path};

use crate::{
    attr::DefaultKind,
    model::{FieldInfo, StructModel},
};

const PHF_THRESHOLD: usize = 32;

pub(crate) fn expand(model: &StructModel) -> TokenStream2 {
    let struct_ident = &model.ident;
    let container = &model.container;
    let serde_path = container
        .serde_path
        .clone()
        .unwrap_or_else(|| syn::parse_str("::serde").expect("valid serde path"));
    let sonic_path: TokenStream2 = container
        .sonic_path
        .clone()
        .unwrap_or_else(|| syn::parse_str("::sonic_rs").expect("valid sonic-rs path"))
        .into_token_stream();
    let use_phf = container.force_phf || model.accepted.len() >= PHF_THRESHOLD;

    let field_names: Vec<_> = model
        .fields
        .iter()
        .filter(|field| field.de_id.is_some())
        .flat_map(|field| field.names.iter())
        .map(|name| LitStr::new(name, Span::call_site()))
        .collect();
    let numeric_entries = numeric_field_entries(&model.fields);
    let accepted_entries: Vec<_> = model
        .accepted
        .iter()
        .map(|(name, id)| (LitStr::new(name, Span::call_site()), *id as u32))
        .collect();

    let de_fields: Vec<_> = model
        .fields
        .iter()
        .filter_map(|field| field.de_id.map(|id| (field, id)))
        .collect();
    let field_variants: Vec<_> = de_fields
        .iter()
        .map(|(_, id)| format_ident!("__field{id}"))
        .collect();
    let ignore_variant = (!container.deny_unknown_fields).then(|| quote!(__ignore,));

    let lookup = field_lookup(use_phf, &accepted_entries, &sonic_path);
    let id_to_variant = de_fields.iter().map(|(_, id)| {
        let id = *id as u32;
        let variant = format_ident!("__field{id}");
        quote!(::core::option::Option::Some(#id) =>
            ::core::result::Result::Ok(__Field::#variant))
    });
    let numeric_to_variant = numeric_entries.iter().map(|(fields_index, id)| {
        let numeric = *fields_index;
        let variant = format_ident!("__field{id}");
        quote!(#numeric => ::core::result::Result::Ok(__Field::#variant))
    });

    let unknown_str = if container.deny_unknown_fields {
        quote!(::core::result::Result::Err(
            <__E as #serde_path::de::Error>::unknown_field(__value, FIELDS)))
    } else {
        quote!(::core::result::Result::Ok(__Field::__ignore))
    };
    let unknown_numeric = if container.deny_unknown_fields {
        let msg = LitStr::new(
            &format!("field index 0 <= i < {}", field_names.len()),
            Span::call_site(),
        );
        quote!(::core::result::Result::Err(
            <__E as #serde_path::de::Error>::invalid_value(
                #serde_path::de::Unexpected::Unsigned(__value), &#msg)))
    } else {
        quote!(::core::result::Result::Ok(__Field::__ignore))
    };
    let invalid_bytes = if container.deny_unknown_fields {
        quote!(::core::result::Result::Err(
            <__E as #serde_path::de::Error>::invalid_value(
                #serde_path::de::Unexpected::Bytes(__value), &self)))
    } else {
        quote!(::core::result::Result::Ok(__Field::__ignore))
    };

    let wrappers = de_fields.iter().filter_map(|(field, id)| {
        let path = field.attrs.deserialize_with.as_ref()?;
        let wrapper = format_ident!("__SonicWith{id}");
        let ty = &field.ty;
        Some(quote! {
            struct #wrapper {
                value: #ty,
            }

            impl<'de> #serde_path::Deserialize<'de> for #wrapper {
                #[inline]
                fn deserialize<__D>(__deserializer: __D)
                    -> ::core::result::Result<Self, __D::Error>
                where
                    __D: #serde_path::Deserializer<'de>,
                {
                    ::core::result::Result::Ok(#wrapper {
                        value: #path(__deserializer)?,
                    })
                }
            }
        })
    });

    let default_init = container_default_init(&container.default, struct_ident);
    let map_declarations = de_fields.iter().map(|(field, id)| {
        let var = format_ident!("__field{id}");
        let ty = &field.ty;
        quote!(let mut #var: ::core::option::Option<#ty> = ::core::option::Option::None;)
    });
    let map_arms = de_fields.iter().map(|(field, id)| {
        let var = format_ident!("__field{id}");
        let variant = format_ident!("__field{id}");
        let canonical = LitStr::new(&field.canonical, Span::call_site());
        let read = if field.attrs.deserialize_with.is_some() {
            let wrapper = format_ident!("__SonicWith{id}");
            quote!(#serde_path::de::MapAccess::next_value::<#wrapper>(&mut __map)?.value)
        } else {
            let ty = &field.ty;
            quote!(#serde_path::de::MapAccess::next_value::<#ty>(&mut __map)?)
        };
        quote! {
            __Field::#variant => {
                if ::core::option::Option::is_some(&#var) {
                    return ::core::result::Result::Err(
                        <__A::Error as #serde_path::de::Error>::duplicate_field(#canonical));
                }
                #var = ::core::option::Option::Some(#read);
            }
        }
    });
    let ignored_map_arm = (!container.deny_unknown_fields).then(|| {
        quote! {
            __Field::__ignore => {
                let _ = #serde_path::de::MapAccess::next_value::<#serde_path::de::IgnoredAny>(
                    &mut __map,
                )?;
            }
        }
    });

    let map_finalize = de_fields.iter().map(|(field, id)| {
        let var = format_ident!("__field{id}");
        let missing = missing_map_expr(field, &container.default, &serde_path, &sonic_path);
        quote! {
            let #var = match #var {
                ::core::option::Option::Some(__value) => __value,
                ::core::option::Option::None => #missing,
            };
        }
    });

    let mut seq_index = 0usize;
    let seq_reads: Vec<_> = model
        .fields
        .iter()
        .enumerate()
        .map(|(field_index, field)| {
            let var = format_ident!("__seq_field{field_index}");
            if let Some(id) = field.de_id {
                let read = if field.attrs.deserialize_with.is_some() {
                    let wrapper = format_ident!("__SonicWith{id}");
                    quote!(::core::option::Option::map(
                        #serde_path::de::SeqAccess::next_element::<#wrapper>(&mut __seq)?,
                        |__wrap| __wrap.value,
                    ))
                } else {
                    let ty = &field.ty;
                    quote!(#serde_path::de::SeqAccess::next_element::<#ty>(&mut __seq)?)
                };
                let missing = missing_seq_expr(
                    field,
                    &container.default,
                    seq_index,
                    &model.expecting,
                    &serde_path,
                );
                seq_index += 1;
                quote! {
                    let #var = match #read {
                        ::core::option::Option::Some(__value) => __value,
                        ::core::option::Option::None => #missing,
                    };
                }
            } else {
                let missing = skipped_expr(field, &container.default);
                quote!(let #var = #missing;)
            }
        })
        .collect();

    let map_construct_fields = model.fields.iter().map(|field| {
        let ident = &field.ident;
        if let Some(id) = field.de_id {
            let value = format_ident!("__field{id}");
            quote!(#ident: #value)
        } else {
            let skipped = skipped_expr(field, &container.default);
            quote!(#ident: #skipped)
        }
    });
    let seq_construct_fields = model.fields.iter().enumerate().map(|(index, field)| {
        let ident = &field.ident;
        let var = format_ident!("__seq_field{index}");
        quote!(#ident: #var)
    });

    let type_name = LitStr::new(&model.type_name, Span::call_site());
    let expecting = LitStr::new(&model.expecting, Span::call_site());

    quote! {
        #[automatically_derived]
        impl<'de> #serde_path::Deserialize<'de> for #struct_ident {
            #[inline]
            fn deserialize<__D>(__deserializer: __D)
                -> ::core::result::Result<Self, __D::Error>
            where
                __D: #serde_path::Deserializer<'de>,
            {
                #(#wrappers)*

                #[allow(non_camel_case_types)]
                enum __Field {
                    #(#field_variants,)*
                    #ignore_variant
                }

                struct __FieldVisitor;

                impl<'de> #serde_path::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;

                    fn expecting(&self, __formatter: &mut ::core::fmt::Formatter)
                        -> ::core::fmt::Result
                    {
                        __formatter.write_str("field identifier")
                    }

                    #[inline]
                    fn visit_u64<__E>(self, __value: u64)
                        -> ::core::result::Result<Self::Value, __E>
                    where
                        __E: #serde_path::de::Error,
                    {
                        match __value {
                            #(#numeric_to_variant,)*
                            _ => #unknown_numeric,
                        }
                    }

                    #[inline]
                    fn visit_str<__E>(self, __value: &str)
                        -> ::core::result::Result<Self::Value, __E>
                    where
                        __E: #serde_path::de::Error,
                    {
                        #lookup
                        match __field_id {
                            #(#id_to_variant,)*
                            _ => #unknown_str,
                        }
                    }

                    #[inline]
                    fn visit_bytes<__E>(self, __value: &[u8])
                        -> ::core::result::Result<Self::Value, __E>
                    where
                        __E: #serde_path::de::Error,
                    {
                        match ::core::str::from_utf8(__value) {
                            ::core::result::Result::Ok(__value) =>
                                #serde_path::de::Visitor::visit_str(self, __value),
                            ::core::result::Result::Err(_) => #invalid_bytes,
                        }
                    }
                }

                impl<'de> #serde_path::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(__deserializer: __D)
                        -> ::core::result::Result<Self, __D::Error>
                    where
                        __D: #serde_path::Deserializer<'de>,
                    {
                        #serde_path::Deserializer::deserialize_identifier(
                            __deserializer,
                            __FieldVisitor,
                        )
                    }
                }

                struct __Visitor {
                    marker: ::core::marker::PhantomData<#struct_ident>,
                }

                impl<'de> #serde_path::de::Visitor<'de> for __Visitor {
                    type Value = #struct_ident;

                    fn expecting(&self, __formatter: &mut ::core::fmt::Formatter)
                        -> ::core::fmt::Result
                    {
                        __formatter.write_str(#expecting)
                    }

                    #[inline]
                    fn visit_seq<__A>(self, mut __seq: __A)
                        -> ::core::result::Result<Self::Value, __A::Error>
                    where
                        __A: #serde_path::de::SeqAccess<'de>,
                    {
                        #default_init
                        #(#seq_reads)*
                        ::core::result::Result::Ok(#struct_ident {
                            #(#seq_construct_fields,)*
                        })
                    }

                    #[inline]
                    fn visit_map<__A>(self, mut __map: __A)
                        -> ::core::result::Result<Self::Value, __A::Error>
                    where
                        __A: #serde_path::de::MapAccess<'de>,
                    {
                        #(#map_declarations)*
                        while let ::core::option::Option::Some(__key) =
                            #serde_path::de::MapAccess::next_key::<__Field>(&mut __map)?
                        {
                            match __key {
                                #(#map_arms)*
                                #ignored_map_arm
                            }
                        }
                        #default_init
                        #(#map_finalize)*
                        ::core::result::Result::Ok(#struct_ident {
                            #(#map_construct_fields,)*
                        })
                    }
                }

                const FIELDS: &'static [&'static str] = &[#(#field_names,)*];
                #serde_path::Deserializer::deserialize_struct(
                    __deserializer,
                    #type_name,
                    FIELDS,
                    __Visitor { marker: ::core::marker::PhantomData },
                )
            }
        }
    }
}

fn field_lookup(
    use_phf: bool,
    accepted_entries: &[(LitStr, u32)],
    sonic_path: &TokenStream2,
) -> TokenStream2 {
    let keys = accepted_entries.iter().map(|(name, _)| name);
    let ids = accepted_entries.iter().map(|(_, id)| id);
    if use_phf {
        quote! {
            static __SONIC_FIELDS: #sonic_path::__private::phf::Map<&'static str, u32> =
                #sonic_path::__private::phf::phf_map! {
                    #(#keys => #ids,)*
                };
            let __field_id = __SONIC_FIELDS.get(__value).copied();
        }
    } else {
        quote! {
            let __field_id = match __value {
                #(#keys => ::core::option::Option::Some(#ids),)*
                _ => ::core::option::Option::None,
            };
        }
    }
}

fn numeric_field_entries(fields: &[FieldInfo]) -> Vec<(u64, usize)> {
    let mut fields_index = 0u64;
    let mut entries = Vec::new();
    for field in fields {
        if let Some(id) = field.de_id {
            for _ in &field.names {
                entries.push((fields_index, id));
                fields_index += 1;
            }
        }
    }
    entries
}

fn container_default_init(default: &DefaultKind, struct_ident: &Ident) -> TokenStream2 {
    match default {
        DefaultKind::None => quote!(),
        DefaultKind::Default => {
            quote!(let __default: #struct_ident = ::core::default::Default::default();)
        }
        DefaultKind::Path(path) => quote!(let __default: #struct_ident = #path();),
    }
}

fn missing_map_expr(
    field: &FieldInfo,
    container_default: &DefaultKind,
    serde_path: &Path,
    sonic_path: &TokenStream2,
) -> TokenStream2 {
    if let Some(expr) = explicit_default_expr(&field.attrs.default) {
        return expr;
    }
    if !matches!(container_default, DefaultKind::None) {
        let ident = &field.ident;
        return quote!(__default.#ident);
    }
    let canonical = LitStr::new(&field.canonical, Span::call_site());
    if field.attrs.deserialize_with.is_some() {
        quote! {
            return ::core::result::Result::Err(
                <__A::Error as #serde_path::de::Error>::missing_field(#canonical))
        }
    } else {
        let ty = &field.ty;
        quote!(#sonic_path::__private::missing_field::<#ty, __A::Error>(#canonical)?)
    }
}

fn missing_seq_expr(
    field: &FieldInfo,
    container_default: &DefaultKind,
    index: usize,
    expecting: &str,
    serde_path: &Path,
) -> TokenStream2 {
    if let Some(expr) = explicit_default_expr(&field.attrs.default) {
        return expr;
    }
    if !matches!(container_default, DefaultKind::None) {
        let ident = &field.ident;
        return quote!(__default.#ident);
    }
    let expecting = LitStr::new(expecting, Span::call_site());
    quote! {
        return ::core::result::Result::Err(
            <__A::Error as #serde_path::de::Error>::invalid_length(#index, &#expecting))
    }
}

fn skipped_expr(field: &FieldInfo, container_default: &DefaultKind) -> TokenStream2 {
    if let Some(expr) = explicit_default_expr(&field.attrs.default) {
        return expr;
    }
    if !matches!(container_default, DefaultKind::None) {
        let ident = &field.ident;
        quote!(__default.#ident)
    } else {
        quote!(::core::default::Default::default())
    }
}

fn explicit_default_expr(default: &DefaultKind) -> Option<TokenStream2> {
    match default {
        DefaultKind::None => None,
        DefaultKind::Default => Some(quote!(::core::default::Default::default())),
        DefaultKind::Path(path) => Some(quote!(#path())),
    }
}

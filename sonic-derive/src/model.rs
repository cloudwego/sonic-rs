use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::Ident;
use syn::{Data, DeriveInput, Fields, Result, Type};

use crate::attr::{parse_container_attrs, parse_field_attrs, ContainerAttrs, FieldAttrs};

pub(crate) struct FieldInfo {
    pub(crate) ident: Ident,
    pub(crate) ty: Type,
    pub(crate) attrs: FieldAttrs,
    pub(crate) canonical: String,
    pub(crate) names: Vec<String>,
    pub(crate) de_id: Option<usize>,
}

pub(crate) struct StructModel {
    pub(crate) ident: Ident,
    pub(crate) container: ContainerAttrs,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) accepted: BTreeMap<String, usize>,
    pub(crate) type_name: String,
    pub(crate) expecting: String,
}

impl StructModel {
    pub(crate) fn from_input(input: DeriveInput) -> Result<Self> {
        if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
            return Err(syn::Error::new_spanned(
                &input.generics,
                "SonicDeserialize MVP supports only non-generic, non-borrowing structs",
            ));
        }

        let named = match &input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => fields,
                _ => {
                    return Err(syn::Error::new_spanned(
                        &data.fields,
                        "SonicDeserialize MVP supports only named structs",
                    ))
                }
            },
            Data::Enum(data) => {
                return Err(syn::Error::new_spanned(
                    data.enum_token,
                    "SonicDeserialize MVP does not yet support enums",
                ))
            }
            Data::Union(data) => {
                return Err(syn::Error::new_spanned(
                    data.union_token,
                    "SonicDeserialize does not support unions",
                ))
            }
        };

        let container = parse_container_attrs(&input.attrs)?;
        let mut fields = Vec::with_capacity(named.named.len());
        let mut accepted = BTreeMap::<String, usize>::new();
        let mut next_de_id = 0usize;

        for field in &named.named {
            let ident = field.ident.clone().expect("named field");
            let attrs = parse_field_attrs(&field.attrs)?;
            let canonical = attrs
                .deserialize_name
                .clone()
                .unwrap_or_else(|| ident.to_string());
            let mut names = BTreeSet::new();
            names.insert(canonical.clone());
            names.extend(attrs.aliases.iter().cloned());
            let names: Vec<_> = names.into_iter().collect();
            let de_id = if attrs.skip_deserializing {
                None
            } else {
                let id = next_de_id;
                next_de_id += 1;
                for name in &names {
                    if let Some(previous) = accepted.insert(name.clone(), id) {
                        if previous != id {
                            return Err(syn::Error::new_spanned(
                                field,
                                format!(
                                    "deserialize field name `{name}` is used by multiple fields"
                                ),
                            ));
                        }
                    }
                }
                Some(id)
            };
            fields.push(FieldInfo {
                ident,
                ty: field.ty.clone(),
                attrs,
                canonical,
                names,
                de_id,
            });
        }

        let type_name = container
            .name
            .clone()
            .unwrap_or_else(|| input.ident.to_string());
        let expecting = container
            .expecting
            .clone()
            .unwrap_or_else(|| format!("struct {type_name}"));

        Ok(Self {
            ident: input.ident,
            container,
            fields,
            accepted,
            type_name,
            expecting,
        })
    }
}

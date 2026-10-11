use syn::{meta::ParseNestedMeta, Attribute, LitStr, Path, Result, Token};

#[derive(Clone, Default)]
pub(crate) enum DefaultKind {
    #[default]
    None,
    Default,
    Path(Path),
}

#[derive(Default)]
pub(crate) struct ContainerAttrs {
    pub(crate) default: DefaultKind,
    pub(crate) deny_unknown_fields: bool,
    pub(crate) expecting: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) serde_path: Option<Path>,
    pub(crate) sonic_path: Option<Path>,
    pub(crate) force_phf: bool,
}

#[derive(Default)]
pub(crate) struct FieldAttrs {
    pub(crate) aliases: Vec<String>,
    pub(crate) default: DefaultKind,
    pub(crate) deserialize_name: Option<String>,
    pub(crate) deserialize_with: Option<Path>,
    pub(crate) skip_deserializing: bool,
}

pub(crate) fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs> {
    let mut out = ContainerAttrs::default();
    for attr in attrs {
        if attr.path().is_ident("sonic") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("force_phf") {
                    out.force_phf = true;
                    Ok(())
                } else if meta.path.is_ident("crate") {
                    out.sonic_path = Some(parse_lit_str(meta)?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("unsupported #[sonic(...)] option"))
                }
            })?;
        } else if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| parse_container_serde_meta(meta, &mut out))?;
        }
    }
    Ok(out)
}

pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if attr.path().is_ident("sonic") {
            return Err(syn::Error::new_spanned(
                attr,
                "SonicDeserialize has no field-level #[sonic(...)] options",
            ));
        }
        if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| parse_field_serde_meta(meta, &mut out))?;
        }
    }
    Ok(out)
}

fn parse_container_serde_meta(meta: ParseNestedMeta<'_>, out: &mut ContainerAttrs) -> Result<()> {
    if meta.path.is_ident("default") {
        out.default = parse_default(meta)?;
    } else if meta.path.is_ident("deny_unknown_fields") {
        out.deny_unknown_fields = true;
    } else if meta.path.is_ident("expecting") {
        out.expecting = Some(parse_lit_str(meta)?.value());
    } else if meta.path.is_ident("crate") {
        out.serde_path = Some(parse_lit_str(meta)?.parse()?);
    } else if meta.path.is_ident("rename") {
        parse_rename(meta, &mut out.name)?;
    } else {
        return Err(
            meta.error("SonicDeserialize MVP does not support this container-level Serde option")
        );
    }
    Ok(())
}

fn parse_field_serde_meta(meta: ParseNestedMeta<'_>, out: &mut FieldAttrs) -> Result<()> {
    if meta.path.is_ident("rename") {
        parse_rename(meta, &mut out.deserialize_name)?;
    } else if meta.path.is_ident("alias") {
        out.aliases.push(parse_lit_str(meta)?.value());
    } else if meta.path.is_ident("default") {
        out.default = parse_default(meta)?;
    } else if meta.path.is_ident("deserialize_with") {
        out.deserialize_with = Some(parse_lit_str(meta)?.parse()?);
    } else if meta.path.is_ident("skip") || meta.path.is_ident("skip_deserializing") {
        out.skip_deserializing = true;
    } else if meta.path.is_ident("skip_serializing") {
        // Serialization-only option; accepted because this derive does not own Serialize.
    } else if meta.path.is_ident("skip_serializing_if")
        || meta.path.is_ident("serialize_with")
        || meta.path.is_ident("getter")
    {
        let _ = parse_lit_str(meta)?;
    } else {
        return Err(
            meta.error("SonicDeserialize MVP does not support this field-level Serde option")
        );
    }
    Ok(())
}

fn parse_rename(meta: ParseNestedMeta<'_>, target: &mut Option<String>) -> Result<()> {
    if meta.input.peek(Token![=]) {
        *target = Some(parse_lit_str(meta)?.value());
        return Ok(());
    }
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("deserialize") {
            *target = Some(parse_lit_str(nested)?.value());
            Ok(())
        } else if nested.path.is_ident("serialize") {
            let _ = parse_lit_str(nested)?;
            Ok(())
        } else {
            Err(nested.error("expected `serialize` or `deserialize`"))
        }
    })
}

fn parse_default(meta: ParseNestedMeta<'_>) -> Result<DefaultKind> {
    if meta.input.peek(Token![=]) {
        Ok(DefaultKind::Path(parse_lit_str(meta)?.parse()?))
    } else {
        Ok(DefaultKind::Default)
    }
}

fn parse_lit_str(meta: ParseNestedMeta<'_>) -> Result<LitStr> {
    meta.value()?.parse()
}

use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{TypeDecl, VariantDecl};
use jisp_types::Type;
use proc_macro2::Ident;
use quote::format_ident;

use crate::CodegenError;

#[derive(Clone, Debug, Default)]
pub(crate) struct EnumTypes {
    pub(crate) names: BTreeMap<String, Ident>,
    pub(crate) enums: BTreeMap<String, EnumShape>,
    pub(crate) variants: BTreeMap<String, VariantShape>,
}

impl EnumTypes {
    pub(crate) fn from_declarations(declarations: &[TypeDecl]) -> Result<Self, CodegenError> {
        let mut names = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut variants = BTreeMap::new();
        for (index, declaration) in declarations.iter().enumerate() {
            let enum_ident = format_ident!("JispEnum{index}");
            names.insert(declaration.name.clone(), enum_ident.clone());
            let shape = enum_shape(declaration, enum_ident, &mut variants)?;
            enums.insert(declaration.name.clone(), shape);
        }
        Ok(Self {
            names,
            enums,
            variants,
        })
    }

    pub(crate) fn ident_for_name(&self, name: &str) -> Result<Ident, CodegenError> {
        self.names
            .get(name)
            .cloned()
            .ok_or(CodegenError::Unsupported("unregistered native enum type"))
    }

    pub(crate) fn variant(&self, name: &str) -> Result<&VariantShape, CodegenError> {
        self.variants.get(name).ok_or(CodegenError::Unsupported(
            "unregistered native enum variant",
        ))
    }

    pub(crate) fn zero_field_variant(&self, name: &str) -> Option<&VariantShape> {
        self.variants
            .get(name)
            .filter(|variant| variant.fields.is_empty())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EnumShape {
    pub(crate) ident: Ident,
    pub(crate) variants: Vec<VariantShape>,
}

#[derive(Clone, Debug)]
pub(crate) struct VariantShape {
    pub(crate) enum_ident: Ident,
    pub(crate) ident: Ident,
    pub(crate) fields: Vec<Type>,
}

fn enum_shape(
    declaration: &TypeDecl,
    enum_ident: Ident,
    variants: &mut BTreeMap<String, VariantShape>,
) -> Result<EnumShape, CodegenError> {
    let mut shapes = Vec::new();
    let mut used_variant_idents = BTreeSet::new();
    for variant in &declaration.variants {
        let shape = variant_shape(variant, &enum_ident)?;
        if !used_variant_idents.insert(shape.ident.to_string()) {
            return Err(CodegenError::Unsupported(
                "enum variants with colliding Rust identifiers",
            ));
        }
        if variants
            .insert(variant.name.clone(), shape.clone())
            .is_some()
        {
            return Err(CodegenError::Unsupported("duplicate native enum variant"));
        }
        shapes.push(shape);
    }
    Ok(EnumShape {
        ident: enum_ident,
        variants: shapes,
    })
}

fn variant_shape(variant: &VariantDecl, enum_ident: &Ident) -> Result<VariantShape, CodegenError> {
    let fields = variant
        .field_types
        .iter()
        .map(|field| parse_declared_type(field))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VariantShape {
        enum_ident: enum_ident.clone(),
        ident: rust_variant_ident(&variant.name),
        fields,
    })
}

fn parse_declared_type(text: &str) -> Result<Type, CodegenError> {
    let text = text.trim();
    Ok(match text {
        "never" => Type::Never,
        "null" => Type::Null,
        "bool" => Type::Bool,
        "int" => Type::Int,
        "float" => Type::Float,
        "str" | "string" => Type::Str,
        _ if is_parenthesized(text) => {
            let inner = &text[1..text.len() - 1];
            let items = split_type_items(inner)?;
            let Some((head, tail)) = items.split_first() else {
                return Err(CodegenError::Unsupported("empty declared native type form"));
            };
            if *head == "list" && tail.len() == 1 {
                Type::List(Box::new(parse_declared_type(tail[0])?))
            } else {
                Type::Named {
                    name: (*head).to_owned(),
                    arguments: tail
                        .iter()
                        .map(|item| parse_declared_type(item))
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
        }
        _ if is_type_parameter_name(text) => {
            return Err(CodegenError::Unsupported(
                "generic native enum declarations",
            ));
        }
        _ if is_type_name(text) => Type::Named {
            name: text.to_owned(),
            arguments: vec![],
        },
        _ => return Err(CodegenError::Unsupported("declared native type syntax")),
    })
}

fn is_parenthesized(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

fn split_type_items(text: &str) -> Result<Vec<&str>, CodegenError> {
    let mut items = vec![];
    let mut depth = 0usize;
    let mut start = None;

    for (index, ch) in text.char_indices() {
        match ch {
            '(' => {
                if start.is_none() {
                    start = Some(index);
                }
                depth += 1;
            }
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(CodegenError::Unsupported("declared native type syntax"))?;
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if let Some(item_start) = start.take() {
                    items.push(&text[item_start..index]);
                }
            }
            _ if start.is_none() => start = Some(index),
            _ => {}
        }
    }

    if depth != 0 {
        return Err(CodegenError::Unsupported("declared native type syntax"));
    }
    if let Some(item_start) = start {
        items.push(&text[item_start..]);
    }
    Ok(items)
}

fn is_type_parameter_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_type_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '/')
}

fn rust_variant_ident(name: &str) -> Ident {
    let mut output = String::new();
    let mut capitalize = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if output.is_empty() && ch.is_ascii_digit() {
                output.push('V');
            }
            if capitalize {
                output.push(ch.to_ascii_uppercase());
                capitalize = false;
            } else {
                output.push(ch);
            }
        } else {
            capitalize = true;
        }
    }
    if output.is_empty() || is_rust_keyword(&output) {
        output.push_str("Variant");
    }
    format_ident!("{output}")
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

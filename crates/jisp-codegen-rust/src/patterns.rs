use jisp_ir::Pattern;
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use crate::emit::{emit_literal, rust_ident};
use crate::enum_types::EnumTypes;
use crate::CodegenError;

#[derive(Clone, Debug)]
pub(crate) struct PatternEmission {
    pub(crate) condition: TokenStream,
    pub(crate) bindings: Vec<PatternBinding>,
}

impl PatternEmission {
    fn empty(condition: TokenStream) -> Self {
        Self {
            condition,
            bindings: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PatternBinding {
    pub(crate) name: String,
    pub(crate) tokens: TokenStream,
}

#[derive(Clone, Debug)]
pub(crate) struct PatternMatch {
    pub(crate) tokens: TokenStream,
    pub(crate) bindings: Vec<String>,
}

pub(crate) fn emit_pattern(
    pattern: &Pattern,
    value: TokenStream,
) -> Result<PatternEmission, CodegenError> {
    match pattern {
        Pattern::Wildcard => Ok(PatternEmission::empty(quote! { true })),
        Pattern::Bind(name) => emit_bind_pattern(name, value),
        Pattern::Literal(literal) => {
            let literal = emit_literal(literal)?;
            Ok(PatternEmission::empty(quote! { #value == #literal }))
        }
        Pattern::Variant { .. } => Err(CodegenError::Unsupported("variant case patterns")),
        Pattern::List { prefix, rest } => emit_list_pattern(prefix, rest.as_deref(), value),
        Pattern::Object(fields) => emit_object_pattern(fields, value),
    }
}

pub(crate) fn emit_variant_match_pattern(
    pattern: &Pattern,
    enum_types: &EnumTypes,
    subject_type: Option<&Type>,
) -> Result<PatternMatch, CodegenError> {
    match pattern {
        Pattern::Variant { tag, fields } => {
            let variant = match subject_type {
                Some(Type::Named { name, arguments })
                    if matches!(
                        (name.as_str(), arguments.len()),
                        ("result", 2) | ("option", 1)
                    ) =>
                {
                    enum_types.prelude_constructor(tag, subject_type)?.ok_or(
                        CodegenError::Unsupported("unregistered native enum variant"),
                    )?
                }
                _ => enum_types.variant(tag)?.clone(),
            };
            if fields.len() != variant.fields.len() {
                return Err(CodegenError::Unsupported(
                    "variant case pattern arity mismatch",
                ));
            }
            let enum_ident = &variant.enum_ident;
            let variant_ident = &variant.ident;
            let mut bindings = Vec::new();
            let fields = fields
                .iter()
                .map(|field| emit_variant_field_pattern(field, &mut bindings))
                .collect::<Result<Vec<_>, _>>()?;
            let tokens = if fields.is_empty() {
                quote! { #enum_ident::#variant_ident }
            } else {
                quote! { #enum_ident::#variant_ident(#(#fields),*) }
            };
            Ok(PatternMatch { tokens, bindings })
        }
        Pattern::Wildcard => Ok(PatternMatch {
            tokens: quote! { _ },
            bindings: vec![],
        }),
        Pattern::Bind(name) => {
            let ident = rust_ident(name);
            Ok(PatternMatch {
                tokens: quote! { #ident },
                bindings: vec![name.clone()],
            })
        }
        Pattern::Literal(_) => Err(CodegenError::Unsupported(
            "literal patterns in native variant case",
        )),
        Pattern::List { .. } => Err(CodegenError::Unsupported("list case patterns")),
        Pattern::Object(_) => Err(CodegenError::Unsupported("object case patterns")),
    }
}

fn emit_bind_pattern(name: &str, value: TokenStream) -> Result<PatternEmission, CodegenError> {
    let ident = rust_ident(name);
    Ok(PatternEmission {
        condition: quote! { true },
        bindings: vec![PatternBinding {
            name: name.to_owned(),
            tokens: quote! { let #ident = #value.clone(); },
        }],
    })
}

fn emit_list_pattern(
    prefix: &[Pattern],
    rest: Option<&str>,
    value: TokenStream,
) -> Result<PatternEmission, CodegenError> {
    let prefix_len = prefix.len();
    let mut condition = if rest.is_some() {
        quote! { #value.len() >= #prefix_len }
    } else {
        quote! { #value.len() == #prefix_len }
    };
    let mut bindings = Vec::new();
    for (index, pattern) in prefix.iter().enumerate() {
        let item = quote! { #value[#index] };
        let emitted = emit_pattern(pattern, item)?;
        let emitted_condition = emitted.condition;
        condition = quote! { #condition && (#emitted_condition) };
        bindings.extend(emitted.bindings);
    }
    if let Some(rest) = rest {
        let ident = rust_ident(rest);
        bindings.push(PatternBinding {
            name: rest.to_owned(),
            tokens: quote! { let #ident = #value[#prefix_len..].to_vec(); },
        });
    }
    Ok(PatternEmission {
        condition,
        bindings,
    })
}

fn emit_object_pattern(
    fields: &[(String, Pattern)],
    value: TokenStream,
) -> Result<PatternEmission, CodegenError> {
    let mut condition = quote! { true };
    let mut bindings = Vec::new();
    for (key, pattern) in fields {
        let field = rust_ident(key);
        let field_value = quote! { #value.#field };
        let emitted = emit_pattern(pattern, field_value)?;
        let emitted_condition = emitted.condition;
        condition = quote! { #condition && (#emitted_condition) };
        bindings.extend(emitted.bindings);
    }
    Ok(PatternEmission {
        condition,
        bindings,
    })
}

fn emit_variant_field_pattern(
    pattern: &Pattern,
    bindings: &mut Vec<String>,
) -> Result<TokenStream, CodegenError> {
    match pattern {
        Pattern::Wildcard => Ok(quote! { _ }),
        Pattern::Bind(name) => {
            bindings.push(name.clone());
            let ident = rust_ident(name);
            Ok(quote! { #ident })
        }
        Pattern::Literal(_) => Err(CodegenError::Unsupported("literal variant field patterns")),
        Pattern::Variant { .. } => Err(CodegenError::Unsupported("nested variant case patterns")),
        Pattern::List { .. } => Err(CodegenError::Unsupported("list case patterns")),
        Pattern::Object(_) => Err(CodegenError::Unsupported("object case patterns")),
    }
}

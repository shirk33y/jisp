use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{Definition, Expr, ExprKind, StringPart, TypeDecl, VariantDecl};
use jisp_types::{Scheme, Type};
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
    pub(crate) fn from_module(
        declarations: &[TypeDecl],
        definitions: &[Definition],
        schemes: &BTreeMap<String, Scheme>,
    ) -> Result<Self, CodegenError> {
        let mut names = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut variants = BTreeMap::new();
        for (index, declaration) in declarations.iter().enumerate() {
            let enum_ident = format_ident!("JispEnum{index}");
            names.insert(declaration.name.clone(), enum_ident.clone());
            let shape = enum_shape(declaration, enum_ident, &mut variants)?;
            enums.insert(declaration.name.clone(), shape);
        }
        let mut types = Self {
            names,
            enums,
            variants,
        }
        .with_prelude_instances(schemes);
        for definition in definitions {
            types.collect_static_obj_get_instances(&definition.value, schemes);
        }
        Ok(types)
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

    pub(crate) fn ident_for_type(&self, ty: &Type) -> Result<Ident, CodegenError> {
        match ty {
            Type::Named { name, arguments } if arguments.is_empty() => self.ident_for_name(name),
            Type::Named { .. } => self
                .enums
                .get(&type_key(ty))
                .map(|shape| shape.ident.clone())
                .ok_or(CodegenError::Unsupported("generic named type emission")),
            _ => Err(CodegenError::Unsupported("unregistered native enum type")),
        }
    }

    pub(crate) fn prelude_constructor(
        &self,
        name: &str,
        expected: Option<&Type>,
    ) -> Result<Option<VariantShape>, CodegenError> {
        let Some(expected) = expected else {
            return Ok(None);
        };
        let Type::Named {
            name: type_name,
            arguments,
        } = expected
        else {
            return Ok(None);
        };
        let fields = match (type_name.as_str(), name, arguments.as_slice()) {
            ("result", "ok", [ok, _]) => vec![ok.clone()],
            ("result", "err", [_, err]) => vec![err.clone()],
            ("option", "some", [item]) => vec![item.clone()],
            ("option", "none", [_]) => vec![],
            _ => return Ok(None),
        };
        let enum_ident = self.ident_for_type(expected)?;
        Ok(Some(VariantShape {
            enum_ident,
            ident: rust_variant_ident(name),
            fields,
        }))
    }

    fn with_prelude_instances(mut self, schemes: &BTreeMap<String, Scheme>) -> Self {
        for scheme in schemes.values() {
            collect_prelude_instances(&scheme.body, &mut self.enums);
        }
        self
    }

    fn collect_static_obj_get_instances(
        &mut self,
        expr: &Expr,
        schemes: &BTreeMap<String, Scheme>,
    ) {
        if let ExprKind::Call { callee, arguments } = &expr.kind {
            if let (ExprKind::Name(name), [object, key]) = (&callee.kind, arguments.as_slice()) {
                if let ("obj.get", ExprKind::Name(object), Some(key)) = (
                    name.as_str(),
                    &object.kind,
                    crate::emit::static_string_key(key),
                ) {
                    if let Some(Scheme {
                        body: Type::Object(row),
                        ..
                    }) = schemes.get(object)
                    {
                        if let Some(field) = row.fields.get(&key) {
                            collect_prelude_instances(
                                &result_type(field.clone(), Type::Str),
                                &mut self.enums,
                            );
                        }
                    }
                }
            }
            self.collect_static_obj_get_instances(callee, schemes);
            for argument in arguments {
                self.collect_static_obj_get_instances(argument, schemes);
            }
            return;
        }

        match &expr.kind {
            ExprKind::Lambda { body, .. } | ExprKind::Not(body) => {
                self.collect_static_obj_get_instances(body, schemes);
            }
            ExprKind::Let { bindings, body } => {
                for (_, value) in bindings {
                    self.collect_static_obj_get_instances(value, schemes);
                }
                self.collect_static_obj_get_instances(body, schemes);
            }
            ExprKind::Do(expressions) | ExprKind::And(expressions) | ExprKind::Or(expressions) => {
                for expression in expressions {
                    self.collect_static_obj_get_instances(expression, schemes);
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_static_obj_get_instances(condition, schemes);
                self.collect_static_obj_get_instances(then_branch, schemes);
                self.collect_static_obj_get_instances(else_branch, schemes);
            }
            ExprKind::List(items) => {
                for item in items {
                    self.collect_static_obj_get_instances(item, schemes);
                }
            }
            ExprKind::Object(fields) => {
                for (key, value) in fields {
                    self.collect_static_obj_get_instances(key, schemes);
                    self.collect_static_obj_get_instances(value, schemes);
                }
            }
            ExprKind::Field { object, key } => {
                self.collect_static_obj_get_instances(object, schemes);
                self.collect_static_obj_get_instances(key, schemes);
            }
            ExprKind::StringTemplate { parts, .. } => {
                for part in parts {
                    if let StringPart::Expr(expression) | StringPart::Splice(expression) = part {
                        self.collect_static_obj_get_instances(expression, schemes);
                    }
                }
            }
            ExprKind::Case { subject, branches } => {
                self.collect_static_obj_get_instances(subject, schemes);
                for branch in branches {
                    self.collect_static_obj_get_instances(&branch.body, schemes);
                }
            }
            ExprKind::Literal(_) | ExprKind::Name(_) => {}
            ExprKind::Call { .. } => unreachable!("calls return before recursive traversal"),
        }
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

fn collect_prelude_instances(ty: &Type, enums: &mut BTreeMap<String, EnumShape>) {
    match ty {
        Type::List(item) => collect_prelude_instances(item, enums),
        Type::Object(row) => {
            for ty in row.fields.values() {
                collect_prelude_instances(ty, enums);
            }
        }
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            for parameter in parameters {
                collect_prelude_instances(parameter, enums);
            }
            if let Some(rest) = rest {
                collect_prelude_instances(rest, enums);
            }
            collect_prelude_instances(result, enums);
        }
        Type::Named { name, arguments } => {
            for argument in arguments {
                collect_prelude_instances(argument, enums);
            }
            if let Some(shape) = prelude_enum_shape(name, arguments, enums.len()) {
                enums.entry(type_key(ty)).or_insert(shape);
            }
        }
        Type::Var(_)
        | Type::Never
        | Type::Null
        | Type::Bool
        | Type::Int
        | Type::BigInt
        | Type::Float
        | Type::Str => {}
    }
}

fn prelude_enum_shape(name: &str, arguments: &[Type], index: usize) -> Option<EnumShape> {
    let ident = format_ident!("JispEnum{index}");
    let variants = match (name, arguments) {
        ("result", [ok, err]) => vec![
            VariantShape {
                enum_ident: ident.clone(),
                ident: rust_variant_ident("ok"),
                fields: vec![ok.clone()],
            },
            VariantShape {
                enum_ident: ident.clone(),
                ident: rust_variant_ident("err"),
                fields: vec![err.clone()],
            },
        ],
        ("option", [item]) => vec![
            VariantShape {
                enum_ident: ident.clone(),
                ident: rust_variant_ident("none"),
                fields: vec![],
            },
            VariantShape {
                enum_ident: ident.clone(),
                ident: rust_variant_ident("some"),
                fields: vec![item.clone()],
            },
        ],
        _ => return None,
    };
    Some(EnumShape { ident, variants })
}

fn type_key(ty: &Type) -> String {
    ty.to_string()
}

fn result_type(ok: Type, err: Type) -> Type {
    Type::Named {
        name: "result".to_owned(),
        arguments: vec![ok, err],
    }
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
        "bigint" => Type::BigInt,
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

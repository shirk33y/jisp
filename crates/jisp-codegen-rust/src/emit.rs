use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{Definition, Expr, ExprKind, Literal};
use jisp_types::{ObjectRow, Scheme, Type, TypedModule};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::CodegenError;

pub(crate) fn emit_module(module: &TypedModule) -> Result<TokenStream, CodegenError> {
    let names = module
        .module
        .definitions
        .iter()
        .map(|definition| definition.name.clone())
        .collect::<BTreeSet<_>>();
    let object_types = ObjectTypes::from_module(module)?;
    let object_structs = emit_object_structs(&object_types)?;
    let definitions = module
        .module
        .definitions
        .iter()
        .map(|definition| emit_definition(module, definition, &names, &object_types))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#object_structs)* #(#definitions)* })
}

fn emit_definition(
    module: &TypedModule,
    definition: &Definition,
    top_level_names: &BTreeSet<String>,
    object_types: &ObjectTypes,
) -> Result<TokenStream, CodegenError> {
    let Some(scheme) = module.schemes.get(&definition.name) else {
        return Err(CodegenError::Unsupported(
            "definition without inferred scheme",
        ));
    };
    let name = rust_ident(&definition.name);
    let visibility = if definition.public || module.module.exports.contains(&definition.name) {
        quote! { pub }
    } else {
        quote! {}
    };

    match (&definition.value.kind, &scheme.body) {
        (
            ExprKind::Lambda { params, rest, body },
            Type::Function {
                parameters, result, ..
            },
        ) => {
            if rest.is_some() {
                return Err(CodegenError::Unsupported("native variadic functions"));
            }
            if params.len() != parameters.len() {
                return Err(CodegenError::Unsupported(
                    "function definitions with mismatched inferred arity",
                ));
            }
            let mut context = EmitContext::new(top_level_names, &module.schemes, object_types);
            let params = params
                .iter()
                .zip(parameters)
                .map(|(name, ty)| {
                    context.locals.insert(name.clone(), Some(ty.clone()));
                    let name = rust_ident(name);
                    let ty = emit_type(ty, object_types)?;
                    Ok(quote! { #name: #ty })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result_ty = result.as_ref();
            let result = emit_type(result_ty, object_types)?;
            let body = context.emit_expr(body, Some(result_ty))?;
            Ok(quote! {
                #visibility fn #name(#(#params),*) -> #result {
                    #body
                }
            })
        }
        (_, ty) => {
            let result = emit_type(ty, object_types)?;
            let body = EmitContext::new(top_level_names, &module.schemes, object_types)
                .emit_expr(&definition.value, Some(ty))?;
            Ok(quote! {
                #visibility fn #name() -> #result {
                    #body
                }
            })
        }
    }
}

struct EmitContext<'a> {
    top_level_names: &'a BTreeSet<String>,
    top_level_schemes: &'a BTreeMap<String, Scheme>,
    object_types: &'a ObjectTypes,
    locals: BTreeMap<String, Option<Type>>,
}

impl<'a> EmitContext<'a> {
    fn new(
        top_level_names: &'a BTreeSet<String>,
        top_level_schemes: &'a BTreeMap<String, Scheme>,
        object_types: &'a ObjectTypes,
    ) -> Self {
        Self {
            top_level_names,
            top_level_schemes,
            object_types,
            locals: BTreeMap::new(),
        }
    }

    fn emit_expr(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        match &expr.kind {
            ExprKind::Literal(literal) => emit_literal(literal),
            ExprKind::Name(name) => {
                let ident = rust_ident(name);
                if self.locals.contains_key(name) {
                    Ok(quote! { #ident })
                } else if self.top_level_names.contains(name) {
                    Ok(quote! { #ident() })
                } else {
                    Err(CodegenError::Unsupported("names outside native module"))
                }
            }
            ExprKind::Let { bindings, body } => self.emit_let(bindings, body, expected),
            ExprKind::Do(expressions) => self.emit_do(expressions, expected),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition, Some(&Type::Bool))?;
                let then_branch = self.emit_expr(then_branch, expected)?;
                let else_branch = self.emit_expr(else_branch, expected)?;
                Ok(quote! { if #condition { #then_branch } else { #else_branch } })
            }
            ExprKind::And(expressions) => self.emit_bool_chain(expressions, quote! { && }),
            ExprKind::Or(expressions) => self.emit_bool_chain(expressions, quote! { || }),
            ExprKind::Not(expression) => {
                let expression = self.emit_expr(expression, Some(&Type::Bool))?;
                Ok(quote! { !#expression })
            }
            ExprKind::Call { callee, arguments } => self.emit_call(callee, arguments),
            ExprKind::Lambda { .. } => Err(CodegenError::Unsupported("nested functions")),
            ExprKind::List(items) => self.emit_list(items, expected),
            ExprKind::Object(fields) => self.emit_object(fields, expected),
            ExprKind::Field { object, key } => self.emit_field(object, key),
            ExprKind::StringTemplate { .. } => Err(CodegenError::Unsupported("string templates")),
            ExprKind::Case { .. } => Err(CodegenError::Unsupported("case expressions")),
        }
    }

    fn emit_let(
        &mut self,
        bindings: &[(String, Expr)],
        body: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let mut emitted = Vec::new();
        let mut added: Vec<String> = Vec::new();
        for (name, value) in bindings {
            let ident = rust_ident(name);
            let value = self.emit_expr(value, None)?;
            self.locals.insert(name.clone(), None);
            added.push(name.clone());
            emitted.push(quote! { let #ident = #value; });
        }
        let body = self.emit_expr(body, expected)?;
        for name in added {
            self.locals.remove(name.as_str());
        }
        Ok(quote! {{ #(#emitted)* #body }})
    }

    fn emit_do(
        &mut self,
        expressions: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let Some((last, leading)) = expressions.split_last() else {
            return Ok(quote! { () });
        };
        let leading = leading
            .iter()
            .map(|expression| self.emit_expr(expression, None))
            .collect::<Result<Vec<_>, _>>()?;
        let last = self.emit_expr(last, expected)?;
        Ok(quote! {{ #(#leading;)* #last }})
    }

    fn emit_list(
        &mut self,
        items: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let item_type = match expected {
            Some(Type::List(item)) => Some(item.as_ref()),
            _ => None,
        };
        let items = items
            .iter()
            .map(|item| self.emit_expr(item, item_type))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { vec![#(#items),*] })
    }

    fn emit_object(
        &mut self,
        fields: &[(Expr, Expr)],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let Some(Type::Object(row)) = expected else {
            return Err(CodegenError::Unsupported(
                "object expressions without expected native type",
            ));
        };
        let ident = self.object_types.ident_for_row(row)?;
        let provided = fields
            .iter()
            .map(|(key, value)| {
                let Some(key) = static_string_key(key) else {
                    return Err(CodegenError::Unsupported("dynamic native object keys"));
                };
                Ok((key, value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        if provided.len() != fields.len() || provided.len() != row.fields.len() {
            return Err(CodegenError::Unsupported("native object field mismatch"));
        }
        let fields = row
            .fields
            .iter()
            .map(|(name, ty)| {
                let Some(value) = provided.get(name) else {
                    return Err(CodegenError::Unsupported("native object field mismatch"));
                };
                let field = rust_ident(name);
                let value = self.emit_expr(value, Some(ty))?;
                Ok(quote! { #field: #value })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { #ident { #(#fields),* } })
    }

    fn emit_field(&mut self, object: &Expr, key: &Expr) -> Result<TokenStream, CodegenError> {
        let Some(key) = static_string_key(key) else {
            return Err(CodegenError::Unsupported("dynamic native field access"));
        };
        let object = self.emit_expr(object, None)?;
        let key = rust_ident(&key);
        Ok(quote! { #object.#key })
    }

    fn emit_bool_chain(
        &mut self,
        expressions: &[Expr],
        operator: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let Some((first, rest)) = expressions.split_first() else {
            return Ok(quote! { true });
        };
        let mut output = self.emit_expr(first, Some(&Type::Bool))?;
        for expression in rest {
            let expression = self.emit_expr(expression, Some(&Type::Bool))?;
            output = quote! { (#output #operator #expression) };
        }
        Ok(output)
    }

    fn emit_call(
        &mut self,
        callee: &Expr,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let ExprKind::Name(name) = &callee.kind else {
            return Err(CodegenError::Unsupported("first-class function calls"));
        };
        if !self.locals.contains_key(name) && !self.top_level_names.contains(name) {
            if let Some(operator) = binary_intrinsic_operator(name) {
                return self.emit_binary_intrinsic(arguments, operator);
            }
            return Err(CodegenError::Unsupported("calls outside native module"));
        }
        let name = rust_ident(name);
        let parameter_types = self.callee_parameter_types(callee).map(Vec::from);
        let arguments = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                self.emit_expr(
                    argument,
                    parameter_types
                        .as_ref()
                        .and_then(|parameters| parameters.get(index)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { #name(#(#arguments),*) })
    }

    fn emit_binary_intrinsic(
        &mut self,
        arguments: &[Expr],
        operator: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, None)?;
        let right = self.emit_expr(right, None)?;
        Ok(quote! { (#left #operator #right) })
    }

    fn callee_parameter_types(&self, callee: &Expr) -> Option<&[Type]> {
        let ExprKind::Name(name) = &callee.kind else {
            return None;
        };
        let ty = self
            .locals
            .get(name)
            .and_then(Option::as_ref)
            .or_else(|| self.top_level_schemes.get(name).map(|scheme| &scheme.body))?;
        match ty {
            Type::Function { parameters, .. } => Some(parameters),
            _ => None,
        }
    }
}

fn binary_intrinsic_operator(name: &str) -> Option<TokenStream> {
    match name {
        "+" => Some(quote! { + }),
        "-" => Some(quote! { - }),
        "*" => Some(quote! { * }),
        "<" => Some(quote! { < }),
        ">" => Some(quote! { > }),
        "<=" => Some(quote! { <= }),
        ">=" => Some(quote! { >= }),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct ObjectShape {
    fields: BTreeMap<String, Type>,
}

#[derive(Clone, Debug, Default)]
struct ObjectTypes {
    names: BTreeMap<String, Ident>,
    shapes: BTreeMap<String, ObjectShape>,
}

impl ObjectTypes {
    fn from_module(module: &TypedModule) -> Result<Self, CodegenError> {
        let mut shapes = BTreeMap::new();
        for scheme in module.schemes.values() {
            collect_object_shapes(&scheme.body, &mut shapes)?;
        }
        let names = shapes
            .keys()
            .enumerate()
            .map(|(index, signature)| (signature.clone(), format_ident!("JispObject{index}")))
            .collect();
        Ok(Self { names, shapes })
    }

    fn ident_for_row(&self, row: &ObjectRow) -> Result<Ident, CodegenError> {
        let signature = object_signature(row)?;
        self.names
            .get(&signature)
            .cloned()
            .ok_or(CodegenError::Unsupported("unregistered native object type"))
    }
}

fn collect_object_shapes(
    ty: &Type,
    shapes: &mut BTreeMap<String, ObjectShape>,
) -> Result<(), CodegenError> {
    match ty {
        Type::Object(row) => {
            if row.rest.is_some() {
                return Err(CodegenError::Unsupported("open object row type emission"));
            }
            for ty in row.fields.values() {
                collect_object_shapes(ty, shapes)?;
            }
            shapes
                .entry(object_signature(row)?)
                .or_insert_with(|| ObjectShape {
                    fields: row.fields.clone(),
                });
        }
        Type::List(item) => collect_object_shapes(item, shapes)?,
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            for ty in parameters {
                collect_object_shapes(ty, shapes)?;
            }
            if let Some(rest) = rest {
                collect_object_shapes(rest, shapes)?;
            }
            collect_object_shapes(result, shapes)?;
        }
        Type::Named { arguments, .. } => {
            for ty in arguments {
                collect_object_shapes(ty, shapes)?;
            }
        }
        Type::Var(_)
        | Type::Never
        | Type::Null
        | Type::Bool
        | Type::Int
        | Type::Float
        | Type::Str => {}
    }
    Ok(())
}

fn emit_object_structs(object_types: &ObjectTypes) -> Result<Vec<TokenStream>, CodegenError> {
    object_types
        .shapes
        .iter()
        .map(|(signature, shape)| {
            let name = object_types
                .names
                .get(signature)
                .ok_or(CodegenError::Unsupported("unregistered native object type"))?;
            let mut field_idents = BTreeSet::new();
            let fields = shape
                .fields
                .iter()
                .map(|(field, ty)| {
                    let field = rust_ident(field);
                    if !field_idents.insert(field.to_string()) {
                        return Err(CodegenError::Unsupported(
                            "object fields with colliding Rust identifiers",
                        ));
                    }
                    let ty = emit_type(ty, object_types)?;
                    Ok(quote! { pub #field: #ty })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(quote! {
                #[derive(Clone, Debug, PartialEq)]
                pub struct #name {
                    #(#fields,)*
                }
            })
        })
        .collect()
}

fn object_signature(row: &ObjectRow) -> Result<String, CodegenError> {
    if row.rest.is_some() {
        return Err(CodegenError::Unsupported("open object row type emission"));
    }
    let fields = row
        .fields
        .iter()
        .map(|(name, ty)| Ok(format!("{name}:{}", type_signature(ty)?)))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(format!("{{{fields}}}"))
}

fn type_signature(ty: &Type) -> Result<String, CodegenError> {
    Ok(match ty {
        Type::Null => "null".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Int => "int".to_owned(),
        Type::Float => "float".to_owned(),
        Type::Str => "str".to_owned(),
        Type::List(item) => format!("list<{}>", type_signature(item)?),
        Type::Object(row) => object_signature(row)?,
        Type::Function { .. } => return Err(CodegenError::Unsupported("function value types")),
        Type::Never => return Err(CodegenError::Unsupported("never type emission")),
        Type::Var(_) => return Err(CodegenError::Unsupported("unresolved type variables")),
        Type::Named { .. } => return Err(CodegenError::Unsupported("named type emission")),
    })
}

fn emit_type(ty: &Type, object_types: &ObjectTypes) -> Result<TokenStream, CodegenError> {
    match ty {
        Type::Null => Ok(quote! { () }),
        Type::Bool => Ok(quote! { bool }),
        Type::Int => Ok(quote! { i64 }),
        Type::Float => Ok(quote! { f64 }),
        Type::Str => Ok(quote! { String }),
        Type::List(item) => {
            let item = emit_type(item, object_types)?;
            Ok(quote! { Vec<#item> })
        }
        Type::Object(row) => object_types
            .ident_for_row(row)
            .map(|ident| quote! { #ident }),
        Type::Never => Err(CodegenError::Unsupported("never type emission")),
        Type::Var(_) => Err(CodegenError::Unsupported("unresolved type variables")),
        Type::Function { .. } => Err(CodegenError::Unsupported("function value types")),
        Type::Named { .. } => Err(CodegenError::Unsupported("named type emission")),
    }
}

fn emit_literal(literal: &Literal) -> Result<TokenStream, CodegenError> {
    Ok(match literal {
        Literal::Null => quote! { () },
        Literal::Bool(value) => quote! { #value },
        Literal::Int(value) => quote! { #value },
        Literal::Float(value) => quote! { #value },
        Literal::String(value) => quote! { String::from(#value) },
    })
}

fn static_string_key(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value.clone()),
        ExprKind::StringTemplate { lines, parts } => {
            let mut fragments = Vec::with_capacity(parts.len());
            for part in parts {
                let jisp_ir::StringPart::Literal(value) = part else {
                    return None;
                };
                fragments.push(value.as_str());
            }
            if *lines {
                Some(fragments.join("\n"))
            } else {
                Some(fragments.concat())
            }
        }
        _ => None,
    }
}

fn rust_ident(name: &str) -> Ident {
    let mut output = String::new();
    for (index, ch) in name.chars().enumerate() {
        let valid = ch == '_' || ch.is_ascii_alphanumeric();
        if index == 0 && ch.is_ascii_digit() {
            output.push('_');
        }
        output.push(if valid { ch } else { '_' });
    }
    if output.is_empty() || is_rust_keyword(&output) {
        output.push('_');
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

use std::collections::BTreeSet;

use jisp_ir::{Definition, Expr, ExprKind, Literal};
use jisp_types::{Type, TypedModule};
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
    let definitions = module
        .module
        .definitions
        .iter()
        .map(|definition| emit_definition(module, definition, &names))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#definitions)* })
}

fn emit_definition(
    module: &TypedModule,
    definition: &Definition,
    top_level_names: &BTreeSet<String>,
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
            let mut context = EmitContext::new(top_level_names);
            let params = params
                .iter()
                .zip(parameters)
                .map(|(name, ty)| {
                    context.locals.insert(name.clone());
                    let name = rust_ident(name);
                    let ty = emit_type(ty)?;
                    Ok(quote! { #name: #ty })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = emit_type(result)?;
            let body = context.emit_expr(body)?;
            Ok(quote! {
                #visibility fn #name(#(#params),*) -> #result {
                    #body
                }
            })
        }
        (_, ty) => {
            let result = emit_type(ty)?;
            let body = EmitContext::new(top_level_names).emit_expr(&definition.value)?;
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
    locals: BTreeSet<String>,
}

impl<'a> EmitContext<'a> {
    fn new(top_level_names: &'a BTreeSet<String>) -> Self {
        Self {
            top_level_names,
            locals: BTreeSet::new(),
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<TokenStream, CodegenError> {
        match &expr.kind {
            ExprKind::Literal(literal) => emit_literal(literal),
            ExprKind::Name(name) => {
                let ident = rust_ident(name);
                if self.locals.contains(name) {
                    Ok(quote! { #ident })
                } else if self.top_level_names.contains(name) {
                    Ok(quote! { #ident() })
                } else {
                    Err(CodegenError::Unsupported("names outside native module"))
                }
            }
            ExprKind::Let { bindings, body } => self.emit_let(bindings, body),
            ExprKind::Do(expressions) => self.emit_do(expressions),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition)?;
                let then_branch = self.emit_expr(then_branch)?;
                let else_branch = self.emit_expr(else_branch)?;
                Ok(quote! { if #condition { #then_branch } else { #else_branch } })
            }
            ExprKind::And(expressions) => self.emit_bool_chain(expressions, quote! { && }),
            ExprKind::Or(expressions) => self.emit_bool_chain(expressions, quote! { || }),
            ExprKind::Not(expression) => {
                let expression = self.emit_expr(expression)?;
                Ok(quote! { !#expression })
            }
            ExprKind::Call { callee, arguments } => self.emit_call(callee, arguments),
            ExprKind::Lambda { .. } => Err(CodegenError::Unsupported("nested functions")),
            ExprKind::List(_) => Err(CodegenError::Unsupported("list expressions")),
            ExprKind::Object(_) => Err(CodegenError::Unsupported("object expressions")),
            ExprKind::Field { .. } => Err(CodegenError::Unsupported("field access")),
            ExprKind::StringTemplate { .. } => Err(CodegenError::Unsupported("string templates")),
            ExprKind::Case { .. } => Err(CodegenError::Unsupported("case expressions")),
        }
    }

    fn emit_let(
        &mut self,
        bindings: &[(String, Expr)],
        body: &Expr,
    ) -> Result<TokenStream, CodegenError> {
        let mut emitted = Vec::new();
        let mut added: Vec<String> = Vec::new();
        for (name, value) in bindings {
            let ident = rust_ident(name);
            let value = self.emit_expr(value)?;
            self.locals.insert(name.clone());
            added.push(name.clone());
            emitted.push(quote! { let #ident = #value; });
        }
        let body = self.emit_expr(body)?;
        for name in added {
            self.locals.remove(name.as_str());
        }
        Ok(quote! {{ #(#emitted)* #body }})
    }

    fn emit_do(&mut self, expressions: &[Expr]) -> Result<TokenStream, CodegenError> {
        let Some((last, leading)) = expressions.split_last() else {
            return Ok(quote! { () });
        };
        let leading = leading
            .iter()
            .map(|expression| self.emit_expr(expression))
            .collect::<Result<Vec<_>, _>>()?;
        let last = self.emit_expr(last)?;
        Ok(quote! {{ #(#leading;)* #last }})
    }

    fn emit_bool_chain(
        &mut self,
        expressions: &[Expr],
        operator: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let Some((first, rest)) = expressions.split_first() else {
            return Ok(quote! { true });
        };
        let mut output = self.emit_expr(first)?;
        for expression in rest {
            let expression = self.emit_expr(expression)?;
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
        if !self.locals.contains(name) && !self.top_level_names.contains(name) {
            if let Some(operator) = binary_intrinsic_operator(name) {
                return self.emit_binary_intrinsic(arguments, operator);
            }
            return Err(CodegenError::Unsupported("calls outside native module"));
        }
        let name = rust_ident(name);
        let arguments = arguments
            .iter()
            .map(|argument| self.emit_expr(argument))
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
        let left = self.emit_expr(left)?;
        let right = self.emit_expr(right)?;
        Ok(quote! { (#left #operator #right) })
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

fn emit_type(ty: &Type) -> Result<TokenStream, CodegenError> {
    match ty {
        Type::Null => Ok(quote! { () }),
        Type::Bool => Ok(quote! { bool }),
        Type::Int => Ok(quote! { i64 }),
        Type::Float => Ok(quote! { f64 }),
        Type::Str => Ok(quote! { String }),
        Type::List(item) => {
            let item = emit_type(item)?;
            Ok(quote! { Vec<#item> })
        }
        Type::Never => Err(CodegenError::Unsupported("never type emission")),
        Type::Var(_) => Err(CodegenError::Unsupported("unresolved type variables")),
        Type::Object(_) => Err(CodegenError::Unsupported("object type emission")),
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

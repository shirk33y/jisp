use std::collections::BTreeSet;

use jisp_ir::{Expr, ExprKind, Pattern, StringPart};
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use super::{emit_type, ensure_unique_rust_idents, rust_ident, EmitContext};
use crate::CodegenError;

impl<'a> EmitContext<'a> {
    pub(super) fn emit_lambda(
        &mut self,
        params: &[String],
        rest: Option<&str>,
        body: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        ensure_unique_rust_idents(params.iter().map(String::as_str), "lambda parameter")?;
        let Some(Type::Function {
            parameters,
            rest: inferred_rest,
            result,
        }) = expected
        else {
            return Err(CodegenError::Unsupported(
                "lambda expressions without native function type",
            ));
        };
        if rest.is_some() || inferred_rest.is_some() {
            return Err(CodegenError::Unsupported("native variadic functions"));
        }
        if params.len() != parameters.len() {
            return Err(CodegenError::Unsupported(
                "lambda expressions with mismatched inferred arity",
            ));
        }
        let mut bound = params.iter().cloned().collect::<BTreeSet<_>>();
        if let Some(rest) = rest {
            bound.insert(rest.to_owned());
        }
        let mut free_names = BTreeSet::new();
        collect_free_names(body, &bound, &mut free_names);
        let captures = free_names
            .into_iter()
            .filter(|name| self.locals.contains_key(name))
            .map(|name| {
                let ident = rust_ident(&name);
                quote! { let #ident = #ident.clone(); }
            })
            .collect::<Vec<_>>();
        let mut previous_locals = Vec::new();
        let parameters = params
            .iter()
            .zip(parameters)
            .map(|(name, ty)| {
                previous_locals.push((
                    name.clone(),
                    self.locals.insert(name.clone(), Some(ty.clone())),
                ));
                let name = rust_ident(name);
                let ty = emit_type(ty, self.object_types, self.enum_types)?;
                Ok(quote! { #name: #ty })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let result_type = emit_type(result, self.object_types, self.enum_types)?;
        let body = self.emit_expr(body, Some(result))?;
        for (name, previous) in previous_locals.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        Ok(quote! {{
            #(#captures)*
            ::std::rc::Rc::new(move |#(#parameters),*| -> #result_type { #body })
        }})
    }
}

fn collect_free_names(expr: &Expr, bound: &BTreeSet<String>, output: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Literal(_) => {}
        ExprKind::Name(name) if !bound.contains(name) => {
            output.insert(name.clone());
        }
        ExprKind::Name(_) => {}
        ExprKind::Lambda { params, rest, body } => {
            let mut scoped = bound.clone();
            scoped.extend(params.iter().cloned());
            if let Some(rest) = rest {
                scoped.insert(rest.clone());
            }
            collect_free_names(body, &scoped, output);
        }
        ExprKind::Let { bindings, body } => {
            let mut scoped = bound.clone();
            for (name, value) in bindings {
                collect_free_names(value, &scoped, output);
                scoped.insert(name.clone());
            }
            collect_free_names(body, &scoped, output);
        }
        ExprKind::Do(expressions) | ExprKind::And(expressions) | ExprKind::Or(expressions) => {
            for expression in expressions {
                collect_free_names(expression, bound, output);
            }
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_free_names(condition, bound, output);
            collect_free_names(then_branch, bound, output);
            collect_free_names(else_branch, bound, output);
        }
        ExprKind::Not(expression) => collect_free_names(expression, bound, output),
        ExprKind::Call { callee, arguments } => {
            collect_free_names(callee, bound, output);
            for argument in arguments {
                collect_free_names(argument, bound, output);
            }
        }
        ExprKind::List(items) => {
            for item in items {
                collect_free_names(item, bound, output);
            }
        }
        ExprKind::Object(fields) => {
            for (key, value) in fields {
                collect_free_names(key, bound, output);
                collect_free_names(value, bound, output);
            }
        }
        ExprKind::Field { object, key } => {
            collect_free_names(object, bound, output);
            collect_free_names(key, bound, output);
        }
        ExprKind::StringTemplate { parts, .. } => {
            for part in parts {
                if let StringPart::Expr(expression) | StringPart::Splice(expression) = part {
                    collect_free_names(expression, bound, output);
                }
            }
        }
        ExprKind::Case { subject, branches } => {
            collect_free_names(subject, bound, output);
            for branch in branches {
                let mut scoped = bound.clone();
                collect_pattern_bindings(&branch.pattern, &mut scoped);
                collect_free_names(&branch.body, &scoped, output);
            }
        }
    }
}

fn collect_pattern_bindings(pattern: &Pattern, output: &mut BTreeSet<String>) {
    match pattern {
        Pattern::Bind(name) => {
            output.insert(name.clone());
        }
        Pattern::Variant { fields, .. } => {
            for field in fields {
                collect_pattern_bindings(field, output);
            }
        }
        Pattern::List { prefix, rest } => {
            for field in prefix {
                collect_pattern_bindings(field, output);
            }
            if let Some(rest) = rest {
                output.insert(rest.clone());
            }
        }
        Pattern::Object(fields) => {
            for (_, value) in fields {
                collect_pattern_bindings(value, output);
            }
        }
        Pattern::Wildcard | Pattern::Literal(_) => {}
    }
}

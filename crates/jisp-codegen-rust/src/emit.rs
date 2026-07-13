use std::collections::{BTreeMap, BTreeSet, HashMap};

use jisp_core::Span;
use jisp_ir::{CaseBranch, Definition, Expr, ExprKind, Literal, Pattern, StringPart};
use jisp_types::{ObjectRow, Scheme, Type, TypedModule};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use self::object_types::ObjectTypes;
use crate::enum_types::EnumTypes;
use crate::patterns::{
    emit_pattern, emit_variant_match_pattern, expand_or_pattern, PatternEmission, PatternMatch,
};
use crate::{CodegenError, GeneratedRust, RustItemKind, RustSourceItem, RustSourceMap};

#[path = "intrinsics.rs"]
mod intrinsics;

#[path = "bigint.rs"]
mod bigint;

#[path = "closures.rs"]
mod closures;

#[path = "object_types.rs"]
mod object_types;

#[path = "dynamic_objects.rs"]
mod dynamic_objects;

#[path = "result.rs"]
mod result;

pub(crate) fn emit_module(module: &TypedModule) -> Result<GeneratedRust, CodegenError> {
    ensure_unique_rust_idents(
        module
            .module
            .definitions
            .iter()
            .map(|definition| definition.name.as_str()),
        "definition",
    )?;
    let names = module
        .module
        .definitions
        .iter()
        .map(|definition| definition.name.clone())
        .collect::<BTreeSet<_>>();
    let object_types = ObjectTypes::from_module(module)?;
    let enum_types = EnumTypes::from_module(
        &module.module.types,
        &module.schemes,
        &module.expression_types,
    )?;
    let object_structs = emit_object_structs(&object_types, &enum_types)?;
    let enum_definitions = emit_enum_definitions(&enum_types, &object_types)?;
    let definitions = module
        .module
        .definitions
        .iter()
        .map(|definition| emit_definition(module, definition, &names, &object_types, &enum_types))
        .collect::<Result<Vec<_>, _>>()?;
    let tokens = quote! { #(#object_structs)* #(#enum_definitions)* #(#definitions)* };
    let mut source_map = rust_source_map(module, &object_types, &enum_types);
    source_map.locate_generated_ranges(&tokens.to_string());
    Ok(GeneratedRust { tokens, source_map })
}

fn emit_definition(
    module: &TypedModule,
    definition: &Definition,
    top_level_names: &BTreeSet<String>,
    object_types: &ObjectTypes,
    enum_types: &EnumTypes,
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
                parameters,
                rest: inferred_rest,
                result,
            },
        ) => {
            if params.len() != parameters.len() {
                return Err(CodegenError::Unsupported(
                    "function definitions with mismatched inferred arity",
                ));
            }
            if rest.is_some() != inferred_rest.is_some() {
                return Err(CodegenError::Unsupported(
                    "function definitions with mismatched inferred rest parameter",
                ));
            }
            let mut parameter_names = params.clone();
            if let Some(rest) = rest {
                parameter_names.push(rest.clone());
            }
            ensure_unique_rust_idents(
                parameter_names.iter().map(String::as_str),
                "function parameter",
            )?;
            let mut context = EmitContext::new(
                top_level_names,
                &module.schemes,
                &module.expression_types,
                object_types,
                enum_types,
            );
            let mut emitted_params = params
                .iter()
                .zip(parameters)
                .map(|(name, ty)| {
                    context.locals.insert(name.clone(), Some(ty.clone()));
                    let name = rust_ident(name);
                    let ty = emit_type(ty, object_types, enum_types)?;
                    Ok(quote! { #name: #ty })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let (Some(rest_name), Some(rest_item)) = (rest, inferred_rest) {
                let rest_type = Type::List(rest_item.clone());
                context
                    .locals
                    .insert(rest_name.clone(), Some(rest_type.clone()));
                let rest_name = rust_ident(rest_name);
                let rest_type = emit_type(&rest_type, object_types, enum_types)?;
                emitted_params.push(quote! { #rest_name: #rest_type });
            }
            let result_ty = result.as_ref();
            let result = emit_type(result_ty, object_types, enum_types)?;
            let body = context.emit_expr(body, Some(result_ty))?;
            Ok(quote! {
                #visibility fn #name(#(#emitted_params),*) -> #result {
                    #body
                }
            })
        }
        (_, ty) => {
            let result = emit_type(ty, object_types, enum_types)?;
            let body = EmitContext::new(
                top_level_names,
                &module.schemes,
                &module.expression_types,
                object_types,
                enum_types,
            )
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
    expression_types: &'a HashMap<Span, Type>,
    object_types: &'a ObjectTypes,
    enum_types: &'a EnumTypes,
    locals: BTreeMap<String, Option<Type>>,
    closure_captures: BTreeSet<String>,
}

struct CaseFallbacks {
    condition: TokenStream,
    guard: TokenStream,
}

impl<'a> EmitContext<'a> {
    fn new(
        top_level_names: &'a BTreeSet<String>,
        top_level_schemes: &'a BTreeMap<String, Scheme>,
        expression_types: &'a HashMap<Span, Type>,
        object_types: &'a ObjectTypes,
        enum_types: &'a EnumTypes,
    ) -> Self {
        Self {
            top_level_names,
            top_level_schemes,
            expression_types,
            object_types,
            enum_types,
            locals: BTreeMap::new(),
            closure_captures: BTreeSet::new(),
        }
    }

    fn emit_expr(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let inferred = self.expression_type(expr).cloned();
        let expected = expected.or(inferred.as_ref());
        match &expr.kind {
            ExprKind::Literal(literal) => emit_literal(literal),
            ExprKind::Name(name) => {
                let ident = rust_ident(name);
                if self.locals.contains_key(name) {
                    if self.closure_captures.contains(name) {
                        Ok(quote! { #ident.clone() })
                    } else {
                        Ok(quote! { #ident })
                    }
                } else if self.top_level_names.contains(name) {
                    match expected {
                        Some(Type::Function {
                            parameters,
                            rest,
                            result,
                        }) => {
                            let mut parameters = parameters
                                .iter()
                                .map(|ty| emit_type(ty, self.object_types, self.enum_types))
                                .collect::<Result<Vec<_>, _>>()?;
                            if let Some(rest) = rest {
                                parameters.push(emit_type(
                                    &Type::List(rest.clone()),
                                    self.object_types,
                                    self.enum_types,
                                )?);
                            }
                            let result = emit_type(result, self.object_types, self.enum_types)?;
                            Ok(quote! {{
                                let __jisp_function: fn(#(#parameters),*) -> #result = #ident;
                                ::std::rc::Rc::new(__jisp_function)
                            }})
                        }
                        _ => Ok(quote! { #ident() }),
                    }
                } else if let Some(variant) = self.enum_types.prelude_constructor(name, expected)? {
                    if !variant.fields.is_empty() {
                        return Err(CodegenError::Unsupported(
                            "bare non-empty prelude enum constructor",
                        ));
                    }
                    let enum_ident = &variant.enum_ident;
                    let variant_ident = &variant.ident;
                    Ok(quote! { #enum_ident::#variant_ident })
                } else if let Some(variant) = self.enum_types.zero_field_variant(name) {
                    let enum_ident = &variant.enum_ident;
                    let variant_ident = &variant.ident;
                    Ok(quote! { #enum_ident::#variant_ident })
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
            ExprKind::Call { callee, arguments } => self.emit_call(callee, arguments, expected),
            ExprKind::Lambda { params, rest, body } => {
                self.emit_lambda(params, rest.as_deref(), body, expected)
            }
            ExprKind::List(items) => self.emit_list(items, expected),
            ExprKind::Object(fields) => self.emit_object(fields, expected),
            ExprKind::Field { object, key } => self.emit_field(object, key, expected),
            ExprKind::StringTemplate { lines, parts } => self.emit_string_template(*lines, parts),
            ExprKind::Case { subject, branches } => self.emit_case(subject, branches, expected),
        }
    }

    fn emit_let(
        &mut self,
        bindings: &[(String, Expr)],
        body: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        ensure_unique_rust_idents(
            bindings.iter().map(|(name, _)| name.as_str()),
            "let binding",
        )?;
        let mut emitted = Vec::new();
        let mut previous_locals = Vec::new();
        let previous_captures = self.closure_captures.clone();
        for (name, value) in bindings {
            let ident = rust_ident(name);
            let value_type = self.expression_type(value).cloned();
            let value = self.emit_expr(value, value_type.as_ref())?;
            previous_locals.push((name.clone(), self.locals.insert(name.clone(), value_type)));
            self.closure_captures.remove(name);
            emitted.push(quote! { let #ident = #value; });
        }
        let body = self.emit_expr(body, expected)?;
        for (name, previous) in previous_locals.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        self.closure_captures = previous_captures;
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

    fn emit_field(
        &mut self,
        object: &Expr,
        key: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        if let Some(key) = static_string_key(key) {
            let object = self.emit_expr(object, None)?;
            let key = rust_ident(&key);
            Ok(quote! { #object.#key })
        } else {
            self.emit_dynamic_field(object, key, expected)
        }
    }

    fn emit_string_template(
        &mut self,
        lines: bool,
        parts: &[StringPart],
    ) -> Result<TokenStream, CodegenError> {
        let mut statements = Vec::new();
        for part in parts {
            match part {
                StringPart::Literal(value) => {
                    statements.push(quote! { fragments.push(String::from(#value)); });
                }
                StringPart::Expr(expression) => {
                    let expression = self.emit_expr(expression, Some(&Type::Str))?;
                    statements.push(quote! { fragments.push(#expression); });
                }
                StringPart::Splice(expression) => {
                    let expected = Type::List(Box::new(Type::Str));
                    let expression = self.emit_expr(expression, Some(&expected))?;
                    statements.push(quote! { fragments.extend(#expression); });
                }
            }
        }
        let result = if lines {
            quote! { fragments.join("\n") }
        } else {
            quote! { fragments.concat() }
        };
        Ok(quote! {{
            let mut fragments: Vec<String> = Vec::new();
            #(#statements)*
            #result
        }})
    }

    fn emit_case(
        &mut self,
        subject: &Expr,
        branches: &[CaseBranch],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let branches = branches
            .iter()
            .map(|branch| {
                expand_or_pattern(&branch.pattern).map(|mut patterns| CaseBranch {
                    pattern: if patterns.len() == 1 {
                        patterns.pop().expect("one pattern")
                    } else {
                        Pattern::Or(patterns)
                    },
                    guard: branch.guard.clone(),
                    body: branch.body.clone(),
                    span: branch.span,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if branches
            .iter()
            .any(|branch| pattern_contains_variant(&branch.pattern))
        {
            return self.emit_variant_case(subject, &branches, expected);
        }
        let subject = self.emit_expr(subject, None)?;
        let subject_name = format_ident!("__jisp_case_subject");
        let mut output = quote! { unreachable!("typechecked Jisp case should be exhaustive") };
        for branch in branches.iter().rev() {
            if let Pattern::Or(alternatives) = &branch.pattern {
                let fallback = output.clone();
                let mut alternatives_output = fallback.clone();
                for alternative in alternatives.iter().rev() {
                    alternatives_output = self.emit_nonvariant_case_branch(
                        alternative,
                        branch.guard.as_ref(),
                        &branch.body,
                        expected,
                        CaseFallbacks {
                            condition: quote! { #alternatives_output },
                            guard: quote! { #fallback },
                        },
                        quote! { #subject_name },
                    )?;
                }
                output = alternatives_output;
            } else {
                output = self.emit_nonvariant_case_branch(
                    &branch.pattern,
                    branch.guard.as_ref(),
                    &branch.body,
                    expected,
                    CaseFallbacks {
                        condition: quote! { #output },
                        guard: quote! { #output },
                    },
                    quote! { #subject_name },
                )?;
            }
        }
        Ok(quote! {{
            let #subject_name = #subject;
            #output
        }})
    }

    fn emit_nonvariant_case_branch(
        &mut self,
        pattern: &Pattern,
        guard: Option<&Expr>,
        body: &Expr,
        expected: Option<&Type>,
        fallbacks: CaseFallbacks,
        subject: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let condition_fallback = fallbacks.condition;
        let guard_fallback = fallbacks.guard;
        let PatternEmission {
            condition,
            bindings,
        } = emit_pattern(pattern, subject)?;
        ensure_unique_rust_idents(
            bindings.iter().map(|binding| binding.name.as_str()),
            "case binding",
        )?;
        let mut previous_locals = Vec::new();
        let previous_captures = self.closure_captures.clone();
        for binding in &bindings {
            previous_locals.push((
                binding.name.clone(),
                self.locals.insert(binding.name.clone(), None),
            ));
            self.closure_captures.remove(&binding.name);
        }
        let guard = guard
            .as_ref()
            .map(|guard| self.emit_expr(guard, Some(&Type::Bool)))
            .transpose()?;
        let body = self.emit_expr(body, expected)?;
        for (name, previous) in previous_locals.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        self.closure_captures = previous_captures;
        let bindings = bindings.iter().map(|binding| &binding.tokens);
        let branch = match guard {
            Some(guard) => quote! {{ #(#bindings)* if #guard { #body } else { #guard_fallback } }},
            None => quote! {{ #(#bindings)* #body }},
        };
        Ok(quote! {
            if #condition {
                #branch
            } else {
                    #condition_fallback
            }
        })
    }

    fn emit_variant_case(
        &mut self,
        subject: &Expr,
        branches: &[CaseBranch],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let subject_type = self.known_expr_type(subject);
        let subject = self.emit_expr(subject, subject_type.as_ref())?;
        let subject_name = format_ident!("__jisp_case_subject");
        let arms = branches
            .iter()
            .map(|branch| {
                self.emit_variant_case_arm(
                    &branch.pattern,
                    branch.guard.as_ref(),
                    &branch.body,
                    expected,
                    subject_type.as_ref(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! {{
            let #subject_name = #subject;
            match #subject_name {
                #(#arms,)*
            }
        }})
    }

    fn emit_variant_case_arm(
        &mut self,
        pattern: &Pattern,
        guard: Option<&Expr>,
        body: &Expr,
        expected: Option<&Type>,
        subject_type: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let PatternMatch { tokens, bindings } =
            emit_variant_match_pattern(pattern, self.enum_types, subject_type)?;
        ensure_unique_rust_idents(bindings.iter().map(String::as_str), "case binding")?;
        let mut previous_locals = Vec::new();
        let previous_captures = self.closure_captures.clone();
        for binding in &bindings {
            previous_locals.push((binding.clone(), self.locals.insert(binding.clone(), None)));
            self.closure_captures.remove(binding);
        }
        let guard = guard
            .map(|guard| self.emit_expr(guard, Some(&Type::Bool)))
            .transpose()?;
        let body = self.emit_expr(body, expected)?;
        for (name, previous) in previous_locals.into_iter().rev() {
            if let Some(previous) = previous {
                self.locals.insert(name, previous);
            } else {
                self.locals.remove(&name);
            }
        }
        self.closure_captures = previous_captures;
        Ok(match guard {
            Some(guard) => quote! { #tokens if #guard => { #body } },
            None => quote! { #tokens => { #body } },
        })
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
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        if let ExprKind::Name(name) = &callee.kind {
            if let Some(variant) = self.enum_types.prelude_constructor(name, expected)? {
                if variant.fields.len() != arguments.len() {
                    return Err(CodegenError::Unsupported(
                        "prelude enum constructor arity mismatch",
                    ));
                }
                let enum_ident = &variant.enum_ident;
                let variant_ident = &variant.ident;
                let arguments = arguments
                    .iter()
                    .zip(&variant.fields)
                    .map(|(argument, ty)| self.emit_expr(argument, Some(ty)))
                    .collect::<Result<Vec<_>, _>>()?;
                return if arguments.is_empty() {
                    Ok(quote! { #enum_ident::#variant_ident })
                } else {
                    Ok(quote! { #enum_ident::#variant_ident(#(#arguments),*) })
                };
            }
            if let Some(variant) = self.enum_types.variants.get(name).cloned() {
                if variant.fields.len() != arguments.len() {
                    return Err(CodegenError::Unsupported(
                        "variant constructor arity mismatch",
                    ));
                }
                let enum_ident = &variant.enum_ident;
                let variant_ident = &variant.ident;
                let arguments = arguments
                    .iter()
                    .zip(&variant.fields)
                    .map(|(argument, ty)| self.emit_expr(argument, Some(ty)))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(quote! { #enum_ident::#variant_ident(#(#arguments),*) });
            }
            let is_local = self.locals.contains_key(name);
            if !is_local && !self.top_level_names.contains(name) {
                return self.emit_native_intrinsic(name, arguments, expected);
            }
            let name = rust_ident(name);
            let callee_type = self
                .expression_type(callee)
                .cloned()
                .ok_or(CodegenError::Unsupported("native callee type"))?;
            let arguments = self.emit_function_call_arguments(arguments, &callee_type)?;
            return if is_local {
                Ok(quote! { (&*#name)(#(#arguments),*) })
            } else {
                Ok(quote! { #name(#(#arguments),*) })
            };
        }
        let callee_type = self
            .expression_type(callee)
            .cloned()
            .ok_or(CodegenError::Unsupported("native callee type"))?;
        let callee = self.emit_expr(callee, Some(&callee_type))?;
        let arguments = self.emit_function_call_arguments(arguments, &callee_type)?;
        Ok(quote! { (#callee)(#(#arguments),*) })
    }

    fn emit_function_call_arguments(
        &mut self,
        arguments: &[Expr],
        callee_type: &Type,
    ) -> Result<Vec<TokenStream>, CodegenError> {
        let Type::Function {
            parameters, rest, ..
        } = callee_type
        else {
            return Err(CodegenError::Unsupported("native first-class callee"));
        };
        if arguments.len() < parameters.len() {
            return Err(CodegenError::Unsupported("native function call arity"));
        }
        if rest.is_none() && arguments.len() != parameters.len() {
            return Err(CodegenError::Unsupported("native function call arity"));
        }
        let mut emitted = arguments[..parameters.len()]
            .iter()
            .zip(parameters)
            .map(|(argument, ty)| self.emit_expr(argument, Some(ty)))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(rest) = rest {
            let rest = arguments[parameters.len()..]
                .iter()
                .map(|argument| self.emit_expr(argument, Some(rest.as_ref())))
                .collect::<Result<Vec<_>, _>>()?;
            emitted.push(quote! { vec![#(#rest),*] });
        }
        Ok(emitted)
    }

    pub(super) fn native_callback_type(&self, callback: &Expr) -> Result<Type, CodegenError> {
        let ty = self
            .expression_type(callback)
            .ok_or(CodegenError::Unsupported("native callback outside module"))?;
        match ty {
            Type::Function { rest: None, .. } => Ok(ty.clone()),
            Type::Function { rest: Some(_), .. } => {
                Err(CodegenError::Unsupported("native variadic function values"))
            }
            _ => Err(CodegenError::Unsupported(
                "native callback is not a function",
            )),
        }
    }

    fn known_expr_type(&self, expr: &Expr) -> Option<Type> {
        self.expression_type(expr).cloned()
    }

    pub(super) fn expression_type(&self, expr: &Expr) -> Option<&Type> {
        self.expression_types
            .get(&expr.span)
            .or_else(|| match &expr.kind {
                ExprKind::Name(name) => self
                    .locals
                    .get(name)
                    .and_then(Option::as_ref)
                    .or_else(|| self.top_level_schemes.get(name).map(|scheme| &scheme.body)),
                _ => None,
            })
    }

    pub(super) fn native_closed_object_row(&self, expr: &Expr) -> Result<ObjectRow, CodegenError> {
        match self.expression_type(expr) {
            Some(Type::Object(row)) if row.rest.is_none() => Ok(row.clone()),
            Some(Type::Object(_)) => {
                Err(CodegenError::Unsupported("open object row type emission"))
            }
            _ => Err(CodegenError::Unsupported(
                "native object helper arguments without known object rows",
            )),
        }
    }
}

fn pattern_contains_variant(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Alias { pattern, .. } => pattern_contains_variant(pattern),
        Pattern::Or(alternatives) => alternatives.iter().any(pattern_contains_variant),
        Pattern::Variant { .. } => true,
        Pattern::Wildcard
        | Pattern::Bind(_)
        | Pattern::Literal(_)
        | Pattern::List { .. }
        | Pattern::Object(_) => false,
    }
}

fn emit_enum_definitions(
    enum_types: &EnumTypes,
    object_types: &ObjectTypes,
) -> Result<Vec<TokenStream>, CodegenError> {
    enum_types
        .enums
        .values()
        .map(|shape| {
            let name = &shape.ident;
            let variants = shape
                .variants
                .iter()
                .map(|variant| {
                    let ident = &variant.ident;
                    if variant.fields.is_empty() {
                        return Ok(quote! { #ident });
                    }
                    let fields = variant
                        .fields
                        .iter()
                        .map(|ty| emit_type(ty, object_types, enum_types))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(quote! { #ident(#(#fields),*) })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(quote! {
                #[derive(Clone, Debug, PartialEq)]
                pub enum #name {
                    #(#variants,)*
                }
            })
        })
        .collect()
}

fn rust_source_map(
    module: &TypedModule,
    object_types: &ObjectTypes,
    enum_types: &EnumTypes,
) -> RustSourceMap {
    let mut items = Vec::new();
    for (signature, shape) in &object_types.shapes {
        if let Some(ident) = object_types.names.get(signature) {
            items.push(RustSourceItem {
                kind: RustItemKind::Struct,
                rust_name: ident.to_string(),
                source_span: shape.source_span,
                generated_range: None,
            });
        }
    }
    for declaration in &module.module.types {
        if let Some(ident) = enum_types.names.get(&declaration.name) {
            items.push(RustSourceItem {
                kind: RustItemKind::Enum,
                rust_name: ident.to_string(),
                source_span: declaration.span,
                generated_range: None,
            });
        }
    }
    for definition in &module.module.definitions {
        items.push(RustSourceItem {
            kind: RustItemKind::Function,
            rust_name: rust_ident(&definition.name).to_string(),
            source_span: definition.span,
            generated_range: None,
        });
    }
    RustSourceMap { items }
}

fn emit_object_structs(
    object_types: &ObjectTypes,
    enum_types: &EnumTypes,
) -> Result<Vec<TokenStream>, CodegenError> {
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
                    let ty = emit_type(ty, object_types, enum_types)?;
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

fn emit_type(
    ty: &Type,
    object_types: &ObjectTypes,
    enum_types: &EnumTypes,
) -> Result<TokenStream, CodegenError> {
    match ty {
        Type::Null => Ok(quote! { () }),
        Type::Bool => Ok(quote! { bool }),
        Type::Int => Ok(quote! { i64 }),
        Type::BigInt => Ok(quote! { ::num_bigint::BigInt }),
        Type::Float => Ok(quote! { f64 }),
        Type::Str => Ok(quote! { String }),
        Type::List(item) => {
            let item = emit_type(item, object_types, enum_types)?;
            Ok(quote! { Vec<#item> })
        }
        Type::Object(row) => object_types
            .ident_for_row(row)
            .map(|ident| quote! { #ident }),
        Type::Never => Err(CodegenError::Unsupported("never type emission")),
        Type::Var(_) => Err(CodegenError::Unsupported("unresolved type variables")),
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            let mut parameters = parameters
                .iter()
                .map(|parameter| emit_type(parameter, object_types, enum_types))
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(rest) = rest {
                parameters.push(emit_type(
                    &Type::List(rest.clone()),
                    object_types,
                    enum_types,
                )?);
            }
            let result = emit_type(result, object_types, enum_types)?;
            Ok(quote! { ::std::rc::Rc<dyn Fn(#(#parameters),*) -> #result> })
        }
        Type::Named { name, arguments } => {
            if !arguments.is_empty() {
                return enum_types.ident_for_type(ty).map(|ident| quote! { #ident });
            }
            enum_types
                .ident_for_name(name)
                .map(|ident| quote! { #ident })
        }
    }
}

pub(crate) fn emit_literal(literal: &Literal) -> Result<TokenStream, CodegenError> {
    Ok(match literal {
        Literal::Null => quote! { () },
        Literal::Bool(value) => quote! { #value },
        Literal::Int(value) => quote! { #value },
        Literal::Float(value) => quote! { #value },
        Literal::String(value) => quote! { String::from(#value) },
    })
}

pub(crate) fn static_string_key(expr: &Expr) -> Option<String> {
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

pub(super) fn result_arguments(expected: Option<&Type>) -> Option<(&Type, &Type)> {
    let Type::Named { name, arguments } = expected? else {
        return None;
    };
    if name == "result" && arguments.len() == 2 {
        Some((&arguments[0], &arguments[1]))
    } else {
        None
    }
}

pub(crate) fn rust_ident(name: &str) -> Ident {
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

fn ensure_unique_rust_idents<'a>(
    names: impl IntoIterator<Item = &'a str>,
    scope: &'static str,
) -> Result<(), CodegenError> {
    let mut emitted = BTreeMap::new();
    for name in names {
        let rust = rust_ident(name).to_string();
        if let Some(first) = emitted.insert(rust.clone(), name.to_owned()) {
            return Err(CodegenError::IdentifierCollision {
                scope,
                first,
                second: name.to_owned(),
                rust,
            });
        }
    }
    Ok(())
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

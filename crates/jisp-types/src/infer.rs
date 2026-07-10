use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{CaseBranch, Expr, ExprKind, Import, Literal, Module, Pattern, StringPart, TypeDecl};
use thiserror::Error;

use crate::top_level::definition_groups;
use crate::{ObjectRow, Scheme, Type, TypeVar, Unifier, UnifyError};

pub type ImportTypeEnvironments = BTreeMap<String, BTreeMap<String, Scheme>>;

#[derive(Clone, Debug, Error)]
pub enum InferError {
    #[error("unknown name `{0}`")]
    UnknownName(String),

    #[error(transparent)]
    Unify(#[from] UnifyError),

    #[error("full Core IR inference is not implemented yet: {0}")]
    NotImplemented(&'static str),

    #[error("unresolved import `{alias}` from `{path}`")]
    UnresolvedImport { alias: String, path: String },

    #[error("pattern binds `{0}` more than once")]
    DuplicatePatternBinding(String),

    #[error("non-exhaustive case for `{type_name}`, missing patterns: {missing:?}")]
    NonExhaustiveCase {
        type_name: String,
        missing: Vec<String>,
    },

    #[error("redundant case pattern `{0}`")]
    RedundantCasePattern(String),
}

/// Reusable state for Hindley–Milner-style inference.
///
/// Expressions are inferred against the current environment. Module inference
/// adds type constructors, gives top-level definitions recursive placeholders,
/// solves them monomorphically, and generalizes the resulting schemes.
#[derive(Clone, Debug, Default)]
pub struct Inferencer {
    next_var: u32,
    pub unifier: Unifier,
    pub environment: BTreeMap<String, Scheme>,
    overloads: BTreeMap<String, Vec<Scheme>>,
    type_variants: BTreeMap<String, BTreeSet<String>>,
}

impl Inferencer {
    pub fn with_prelude() -> Self {
        Self {
            environment: crate::prelude::environment(),
            overloads: crate::prelude::overloads(),
            type_variants: crate::prelude::variants(),
            ..Self::default()
        }
    }

    pub fn fresh_type(&mut self) -> Type {
        let var = TypeVar(self.next_var);
        self.next_var += 1;
        Type::Var(var)
    }

    pub fn define(&mut self, name: impl Into<String>, scheme: Scheme) {
        let name = name.into();
        self.overloads.remove(&name);
        self.environment.insert(name, scheme);
    }

    pub fn lookup(&mut self, name: &str) -> Result<Type, InferError> {
        let scheme = self
            .environment
            .get(name)
            .cloned()
            .ok_or_else(|| InferError::UnknownName(name.to_owned()))?;
        Ok(self.instantiate(&scheme))
    }

    pub fn infer_module(
        &mut self,
        module: &Module,
    ) -> Result<BTreeMap<String, Scheme>, InferError> {
        self.infer_module_with_imports(module, &BTreeMap::new())
    }

    pub fn infer_module_with_imports(
        &mut self,
        module: &Module,
        imports: &ImportTypeEnvironments,
    ) -> Result<BTreeMap<String, Scheme>, InferError> {
        self.install_imports(&module.imports, imports)?;
        self.install_type_constructors(&module.types)?;
        let mut schemes = BTreeMap::new();
        for group in definition_groups(&module.definitions) {
            let outer_environment = self.environment.clone();
            let mut placeholders = BTreeMap::new();

            for index in &group {
                let definition = &module.definitions[*index];
                let ty = self.fresh_type();
                self.define(&definition.name, Scheme::mono(ty.clone()));
                placeholders.insert(*index, ty);
            }

            for index in &group {
                let definition = &module.definitions[*index];
                let value = self.infer_expr(&definition.value)?;
                let placeholder = placeholders
                    .get(index)
                    .expect("definition placeholders are installed first")
                    .clone();
                self.unify(placeholder, value)?;
            }

            for index in &group {
                let definition = &module.definitions[*index];
                let ty = self.apply(
                    placeholders
                        .get(index)
                        .expect("definition placeholders are installed first"),
                );
                let scheme = generalize_with_environment(&ty, &outer_environment);
                self.define(definition.name.clone(), scheme.clone());
                schemes.insert(definition.name.clone(), scheme);
            }
        }

        Ok(schemes)
    }

    fn install_imports(
        &mut self,
        imports: &[Import],
        environments: &ImportTypeEnvironments,
    ) -> Result<(), InferError> {
        for import in imports {
            let environment =
                environments
                    .get(&import.path)
                    .ok_or_else(|| InferError::UnresolvedImport {
                        alias: import.alias.clone(),
                        path: import.path.clone(),
                    })?;
            for (name, scheme) in environment {
                self.define(format!("{}.{}", import.alias, name), scheme.clone());
            }
        }
        Ok(())
    }

    pub fn infer_expr(&mut self, expr: &Expr) -> Result<Type, InferError> {
        let ty = match &expr.kind {
            ExprKind::Literal(literal) => self.infer_literal(literal),
            ExprKind::Name(name) => self.lookup(name)?,
            ExprKind::Lambda { params, rest, body } => {
                if rest.is_some() {
                    return Err(InferError::NotImplemented("variadic function types"));
                }
                self.with_scope(|inferencer| {
                    let parameters = params
                        .iter()
                        .map(|name| {
                            let ty = inferencer.fresh_type();
                            inferencer.define(name, Scheme::mono(ty.clone()));
                            ty
                        })
                        .collect::<Vec<_>>();
                    let result = inferencer.infer_expr(body)?;
                    Ok(Type::Function {
                        parameters: parameters.iter().map(|ty| inferencer.apply(ty)).collect(),
                        result: Box::new(inferencer.apply(&result)),
                    })
                })?
            }
            ExprKind::Let { bindings, body } => self.infer_let(bindings, body)?,
            ExprKind::Do(expressions) => self.infer_do(expressions)?,
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.infer_expr(condition)?;
                let then_ty = self.infer_expr(then_branch)?;
                let else_ty = self.infer_expr(else_branch)?;
                self.unify(then_ty, else_ty)?
            }
            ExprKind::And(expressions) => self.infer_short_circuit(expressions, Type::Bool)?,
            ExprKind::Or(expressions) => self.infer_short_circuit(expressions, Type::Null)?,
            ExprKind::Not(expression) => {
                self.infer_expr(expression)?;
                Type::Bool
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Name(name) = &callee.kind {
                    if let Some(overloads) = self.overloads.get(name).cloned() {
                        return self.infer_overloaded_call(&overloads, arguments);
                    }
                }
                let callee_ty = self.infer_expr(callee)?;
                let parameters = arguments
                    .iter()
                    .map(|argument| self.infer_expr(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.fresh_type();
                self.unify(
                    callee_ty,
                    Type::Function {
                        parameters,
                        result: Box::new(result.clone()),
                    },
                )?;
                result
            }
            ExprKind::List(expressions) => {
                let item = self.fresh_type();
                for expression in expressions {
                    let value = self.infer_expr(expression)?;
                    self.unify(item.clone(), value)?;
                }
                Type::List(Box::new(self.apply(&item)))
            }
            ExprKind::Object(fields) => self.infer_object(fields)?,
            ExprKind::Field { object, key } => self.infer_field(object, key)?,
            ExprKind::StringTemplate { parts, .. } => {
                for part in parts {
                    match part {
                        StringPart::Literal(_) => {}
                        StringPart::Expr(expression) => {
                            let ty = self.infer_expr(expression)?;
                            self.unify(ty, Type::Str)?;
                        }
                        StringPart::Splice(expression) => {
                            let ty = self.infer_expr(expression)?;
                            self.unify(ty, Type::List(Box::new(Type::Str)))?;
                        }
                    }
                }
                Type::Str
            }
            ExprKind::Case { subject, branches } => self.infer_case(subject, branches)?,
        };
        Ok(self.apply(&ty))
    }

    pub fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let replacements: BTreeMap<_, _> = scheme
            .variables
            .iter()
            .copied()
            .map(|var| (var, self.fresh_type()))
            .collect();
        replace(&scheme.body, &replacements)
    }

    pub fn generalize(&self, ty: &Type) -> Scheme {
        let ty = self.apply(ty);
        generalize_with_environment(&ty, &self.environment)
    }

    fn install_type_constructors(&mut self, declarations: &[TypeDecl]) -> Result<(), InferError> {
        for declaration in declarations {
            self.type_variants.insert(
                declaration.name.clone(),
                declaration
                    .variants
                    .iter()
                    .map(|variant| variant.name.clone())
                    .collect(),
            );

            let mut parameters = TypeParameters::default();
            for variant in &declaration.variants {
                for field in &variant.field_types {
                    self.declared_type(field, &mut parameters)?;
                }
            }

            for variant in &declaration.variants {
                let fields = variant
                    .field_types
                    .iter()
                    .map(|field| self.declared_type(field, &mut parameters))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = Type::Named {
                    name: declaration.name.clone(),
                    arguments: parameters.types(),
                };
                let body = if fields.is_empty() {
                    result
                } else {
                    Type::Function {
                        parameters: fields,
                        result: Box::new(result),
                    }
                };
                self.define(
                    variant.name.clone(),
                    Scheme {
                        variables: parameters.vars(),
                        body,
                    },
                );
            }
        }
        Ok(())
    }

    fn declared_type(
        &mut self,
        text: &str,
        parameters: &mut TypeParameters,
    ) -> Result<Type, InferError> {
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
                    return Err(InferError::NotImplemented("empty declared type form"));
                };
                if *head == "list" && tail.len() == 1 {
                    Type::List(Box::new(self.declared_type(tail[0], parameters)?))
                } else {
                    Type::Named {
                        name: (*head).to_owned(),
                        arguments: tail
                            .iter()
                            .map(|item| self.declared_type(item, parameters))
                            .collect::<Result<Vec<_>, _>>()?,
                    }
                }
            }
            _ if is_type_parameter_name(text) => {
                Type::Var(parameters.get_or_insert(text, || self.fresh_var()))
            }
            _ if is_type_name(text) => Type::Named {
                name: text.to_owned(),
                arguments: vec![],
            },
            _ => return Err(InferError::NotImplemented("declared type syntax")),
        })
    }

    fn infer_literal(&self, literal: &Literal) -> Type {
        match literal {
            Literal::Null => Type::Null,
            Literal::Bool(_) => Type::Bool,
            Literal::Int(_) => Type::Int,
            Literal::Float(_) => Type::Float,
            Literal::String(_) => Type::Str,
        }
    }

    fn infer_let(&mut self, bindings: &[(String, Expr)], body: &Expr) -> Result<Type, InferError> {
        self.with_scope(|inferencer| {
            for (name, value) in bindings {
                let ty = inferencer.infer_expr(value)?;
                let scheme = inferencer.generalize(&ty);
                inferencer.define(name, scheme);
            }
            inferencer.infer_expr(body)
        })
    }

    fn infer_do(&mut self, expressions: &[Expr]) -> Result<Type, InferError> {
        let mut ty = Type::Null;
        for expression in expressions {
            ty = self.infer_expr(expression)?;
        }
        Ok(ty)
    }

    fn infer_short_circuit(
        &mut self,
        expressions: &[Expr],
        empty: Type,
    ) -> Result<Type, InferError> {
        let Some((first, rest)) = expressions.split_first() else {
            return Ok(empty);
        };
        let ty = self.infer_expr(first)?;
        for expression in rest {
            let next = self.infer_expr(expression)?;
            self.unify(ty.clone(), next)?;
        }
        Ok(ty)
    }

    fn infer_object(&mut self, fields: &[(Expr, Expr)]) -> Result<Type, InferError> {
        let mut typed_fields = BTreeMap::new();
        for (key, value) in fields {
            let key_ty = self.infer_expr(key)?;
            self.unify(key_ty, Type::Str)?;
            let Some(name) = static_string_key(key) else {
                return Err(InferError::NotImplemented("dynamic object key types"));
            };
            typed_fields.insert(name, self.infer_expr(value)?);
        }
        Ok(Type::Object(ObjectRow {
            fields: typed_fields,
            rest: None,
        }))
    }

    fn infer_field(&mut self, object: &Expr, key: &Expr) -> Result<Type, InferError> {
        let key_ty = self.infer_expr(key)?;
        self.unify(key_ty, Type::Str)?;
        let Some(name) = static_string_key(key) else {
            return Err(InferError::NotImplemented("dynamic field key types"));
        };

        let object_ty = self.infer_expr(object)?;
        let field_ty = self.fresh_type();
        let rest = self.fresh_var();
        self.unify(
            object_ty,
            Type::Object(ObjectRow {
                fields: BTreeMap::from([(name, field_ty.clone())]),
                rest: Some(rest),
            }),
        )?;
        Ok(field_ty)
    }

    fn infer_case(&mut self, subject: &Expr, branches: &[CaseBranch]) -> Result<Type, InferError> {
        let subject_ty = self.infer_expr(subject)?;
        let result_ty = self.fresh_type();

        for branch in branches {
            let body_ty = self.with_scope(|inferencer| {
                let mut bindings = BTreeSet::new();
                inferencer.infer_pattern(&branch.pattern, subject_ty.clone(), &mut bindings)?;
                inferencer.infer_expr(&branch.body)
            })?;
            self.unify(result_ty.clone(), body_ty)?;
        }

        self.check_case_exhaustive(&subject_ty, branches)?;
        Ok(result_ty)
    }

    fn check_case_exhaustive(
        &self,
        subject_ty: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let subject_ty = self.apply(subject_ty);
        match &subject_ty {
            Type::Named { name, .. } => {
                let Some(variants) = self.type_variants.get(name) else {
                    return Ok(());
                };
                self.check_finite_case_exhaustive(name, variants.clone(), branches)
            }
            Type::Bool => self.check_finite_case_exhaustive(
                "bool",
                BTreeSet::from(["false".to_owned(), "true".to_owned()]),
                branches,
            ),
            Type::Null => self.check_finite_case_exhaustive(
                "null",
                BTreeSet::from(["null".to_owned()]),
                branches,
            ),
            Type::List(item) => self.check_list_case_exhaustive(item, branches),
            Type::Object(_) => self.check_object_case_exhaustive(&subject_ty, branches),
            _ => Ok(()),
        }
    }

    fn check_finite_case_exhaustive(
        &self,
        type_name: &str,
        expected: BTreeSet<String>,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut covered = BTreeSet::new();
        let mut has_catch_all = false;
        for branch in branches {
            if has_catch_all {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }
            match &branch.pattern {
                Pattern::Wildcard | Pattern::Bind(_) => has_catch_all = true,
                Pattern::Variant { tag, .. } => {
                    if !covered.insert(tag.clone()) {
                        return Err(InferError::RedundantCasePattern(tag.clone()));
                    }
                }
                Pattern::Literal(Literal::Bool(value)) if type_name == "bool" => {
                    let name = value.to_string();
                    if !covered.insert(name.clone()) {
                        return Err(InferError::RedundantCasePattern(name));
                    }
                }
                Pattern::Literal(Literal::Null) if type_name == "null" => {
                    if !covered.insert("null".to_owned()) {
                        return Err(InferError::RedundantCasePattern("null".to_owned()));
                    }
                }
                Pattern::Literal(_) | Pattern::List { .. } | Pattern::Object(_) => {}
            }
        }

        if has_catch_all {
            return Ok(());
        }

        let missing = expected.difference(&covered).cloned().collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(InferError::NonExhaustiveCase {
                type_name: type_name.to_owned(),
                missing,
            })
        }
    }

    fn check_list_case_exhaustive(
        &self,
        item: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut exact_lengths = BTreeSet::new();
        let mut rest_lengths = BTreeSet::new();
        let mut has_catch_all = false;

        for branch in branches {
            if has_catch_all {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }

            match &branch.pattern {
                Pattern::Wildcard | Pattern::Bind(_) => has_catch_all = true,
                Pattern::List { prefix, rest } => {
                    let irrefutable_prefix = prefix
                        .iter()
                        .all(|pattern| self.pattern_is_irrefutable_for_type(pattern, item));
                    if !irrefutable_prefix {
                        continue;
                    }

                    let length = prefix.len();
                    if rest_lengths.iter().any(|covered| *covered <= length) {
                        return Err(InferError::RedundantCasePattern(pattern_name(
                            &branch.pattern,
                        )));
                    }

                    if rest.is_some() {
                        rest_lengths.insert(length);
                        if length == 0 {
                            has_catch_all = true;
                        }
                    } else if !exact_lengths.insert(length) {
                        return Err(InferError::RedundantCasePattern(pattern_name(
                            &branch.pattern,
                        )));
                    }
                }
                _ => {}
            }
        }

        if has_catch_all || list_coverage_is_exhaustive(&exact_lengths, &rest_lengths) {
            Ok(())
        } else {
            Err(InferError::NonExhaustiveCase {
                type_name: "list".to_owned(),
                missing: missing_list_patterns(&exact_lengths, &rest_lengths),
            })
        }
    }

    fn check_object_case_exhaustive(
        &self,
        subject_ty: &Type,
        branches: &[CaseBranch],
    ) -> Result<(), InferError> {
        let mut has_catch_all = false;
        for branch in branches {
            if has_catch_all {
                return Err(InferError::RedundantCasePattern(pattern_name(
                    &branch.pattern,
                )));
            }
            if self.pattern_is_irrefutable_for_type(&branch.pattern, subject_ty) {
                has_catch_all = true;
            }
        }

        if has_catch_all {
            Ok(())
        } else {
            Err(InferError::NonExhaustiveCase {
                type_name: "object".to_owned(),
                missing: vec!["object pattern".to_owned()],
            })
        }
    }

    fn pattern_is_irrefutable_for_type(&self, pattern: &Pattern, ty: &Type) -> bool {
        match pattern {
            Pattern::Wildcard | Pattern::Bind(_) => true,
            Pattern::Literal(Literal::Null) => matches!(self.apply(ty), Type::Null),
            Pattern::List {
                prefix,
                rest: Some(_),
            } => {
                let Type::List(item) = self.apply(ty) else {
                    return false;
                };
                prefix
                    .iter()
                    .all(|pattern| self.pattern_is_irrefutable_for_type(pattern, &item))
            }
            Pattern::Object(fields) => {
                let Type::Object(row) = self.apply(ty) else {
                    return false;
                };
                fields.iter().all(|(name, pattern)| {
                    row.fields
                        .get(name)
                        .is_some_and(|ty| self.pattern_is_irrefutable_for_type(pattern, ty))
                })
            }
            Pattern::Literal(_) | Pattern::Variant { .. } | Pattern::List { rest: None, .. } => {
                false
            }
        }
    }

    fn infer_pattern(
        &mut self,
        pattern: &Pattern,
        expected: Type,
        bindings: &mut BTreeSet<String>,
    ) -> Result<(), InferError> {
        match pattern {
            Pattern::Wildcard => {}
            Pattern::Bind(name) => self.bind_pattern_name(name, expected, bindings)?,
            Pattern::Literal(literal) => {
                let literal_ty = self.infer_literal(literal);
                self.unify(expected, literal_ty)?;
            }
            Pattern::Variant { tag, fields } => {
                let constructor_ty = self.lookup(tag)?;
                match (fields.as_slice(), self.apply(&constructor_ty)) {
                    ([], constructor_ty @ Type::Named { .. }) => {
                        self.unify(expected, constructor_ty)?;
                    }
                    (fields, Type::Function { parameters, result }) => {
                        if fields.len() != parameters.len() {
                            return Err(InferError::Unify(UnifyError::Arity {
                                left: parameters.len(),
                                right: fields.len(),
                            }));
                        }
                        self.unify(expected, *result)?;
                        for (field, parameter) in fields.iter().zip(parameters) {
                            self.infer_pattern(field, parameter, bindings)?;
                        }
                    }
                    ([], other) => {
                        self.unify(expected, other)?;
                    }
                    (_, other) => {
                        return Err(InferError::Unify(UnifyError::Mismatch {
                            left: other,
                            right: Type::Function {
                                parameters: fields.iter().map(|_| self.fresh_type()).collect(),
                                result: Box::new(expected),
                            },
                        }));
                    }
                }
            }
            Pattern::List { prefix, rest } => {
                let item = self.fresh_type();
                self.unify(expected, Type::List(Box::new(item.clone())))?;
                for pattern in prefix {
                    self.infer_pattern(pattern, item.clone(), bindings)?;
                }
                if let Some(name) = rest {
                    self.bind_pattern_name(name, Type::List(Box::new(item)), bindings)?;
                }
            }
            Pattern::Object(fields) => {
                let mut row_fields = BTreeMap::new();
                for (name, pattern) in fields {
                    let field_ty = self.fresh_type();
                    row_fields.insert(name.clone(), field_ty.clone());
                    self.infer_pattern(pattern, field_ty, bindings)?;
                }
                let rest = self.fresh_var();
                self.unify(
                    expected,
                    Type::Object(ObjectRow {
                        fields: row_fields,
                        rest: Some(rest),
                    }),
                )?;
            }
        }
        Ok(())
    }

    fn bind_pattern_name(
        &mut self,
        name: &str,
        ty: Type,
        bindings: &mut BTreeSet<String>,
    ) -> Result<(), InferError> {
        if !bindings.insert(name.to_owned()) {
            return Err(InferError::DuplicatePatternBinding(name.to_owned()));
        }
        self.define(name, Scheme::mono(self.apply(&ty)));
        Ok(())
    }

    fn apply(&self, ty: &Type) -> Type {
        self.unifier.substitution.apply(ty)
    }

    fn unify(&mut self, left: Type, right: Type) -> Result<Type, InferError> {
        Ok(self.unifier.unify(left, right)?)
    }

    fn fresh_var(&mut self) -> TypeVar {
        let Type::Var(var) = self.fresh_type() else {
            unreachable!("fresh_type always returns a type variable");
        };
        var
    }

    fn with_scope<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, InferError>,
    ) -> Result<T, InferError> {
        let environment = self.environment.clone();
        let overloads = self.overloads.clone();
        let result = f(self);
        self.environment = environment;
        self.overloads = overloads;
        result
    }

    fn infer_overloaded_call(
        &mut self,
        overloads: &[Scheme],
        arguments: &[Expr],
    ) -> Result<Type, InferError> {
        let mut last_error = None;

        for overload in overloads {
            let mut candidate = self.clone();
            match candidate.infer_call_with_scheme(overload, arguments) {
                Ok(result) => {
                    *self = candidate;
                    return Ok(self.apply(&result));
                }
                Err(error) => last_error = Some(error),
            }
        }

        Err(last_error.expect("overloaded call has at least one candidate"))
    }

    fn infer_call_with_scheme(
        &mut self,
        scheme: &Scheme,
        arguments: &[Expr],
    ) -> Result<Type, InferError> {
        let callee_ty = self.instantiate(scheme);
        let parameters = arguments
            .iter()
            .map(|argument| self.infer_expr(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.fresh_type();
        self.unify(
            callee_ty,
            Type::Function {
                parameters,
                result: Box::new(result.clone()),
            },
        )?;
        Ok(result)
    }
}

fn replace(ty: &Type, replacements: &BTreeMap<TypeVar, Type>) -> Type {
    match ty {
        Type::Var(var) => replacements.get(var).cloned().unwrap_or(Type::Var(*var)),
        Type::List(item) => Type::List(Box::new(replace(item, replacements))),
        Type::Object(row) => Type::Object(crate::ObjectRow {
            fields: row
                .fields
                .iter()
                .map(|(name, ty)| (name.clone(), replace(ty, replacements)))
                .collect(),
            rest: row.rest,
        }),
        Type::Function { parameters, result } => Type::Function {
            parameters: parameters
                .iter()
                .map(|ty| replace(ty, replacements))
                .collect(),
            result: Box::new(replace(result, replacements)),
        },
        Type::Named { name, arguments } => Type::Named {
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(|ty| replace(ty, replacements))
                .collect(),
        },
        other => other.clone(),
    }
}

fn generalize_with_environment(ty: &Type, environment: &BTreeMap<String, Scheme>) -> Scheme {
    let mut variables = free_type_vars(ty);
    for var in environment.values().flat_map(free_scheme_vars) {
        variables.remove(&var);
    }
    Scheme {
        variables: variables.into_iter().collect(),
        body: ty.clone(),
    }
}

fn static_string_key(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value.clone()),
        ExprKind::StringTemplate { lines, parts } => {
            let mut fragments = Vec::with_capacity(parts.len());
            for part in parts {
                let StringPart::Literal(value) = part else {
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

fn pattern_name(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_owned(),
        Pattern::Bind(name) => name.clone(),
        Pattern::Literal(Literal::Null) => "null".to_owned(),
        Pattern::Literal(Literal::Bool(value)) => value.to_string(),
        Pattern::Literal(Literal::Int(value)) => value.to_string(),
        Pattern::Literal(Literal::Float(value)) => value.to_string(),
        Pattern::Literal(Literal::String(value)) => format!("{value:?}"),
        Pattern::Variant { tag, .. } => tag.clone(),
        Pattern::List { .. } => "list pattern".to_owned(),
        Pattern::Object(_) => "object pattern".to_owned(),
    }
}

fn list_coverage_is_exhaustive(
    exact_lengths: &BTreeSet<usize>,
    rest_lengths: &BTreeSet<usize>,
) -> bool {
    let Some(rest_start) = rest_lengths.first().copied() else {
        return false;
    };
    (0..rest_start).all(|length| exact_lengths.contains(&length))
}

fn missing_list_patterns(
    exact_lengths: &BTreeSet<usize>,
    rest_lengths: &BTreeSet<usize>,
) -> Vec<String> {
    if let Some(rest_start) = rest_lengths.first().copied() {
        return (0..rest_start)
            .filter(|length| !exact_lengths.contains(length))
            .map(list_length_pattern)
            .collect();
    }

    let max_exact = exact_lengths.last().copied().unwrap_or(0);
    let mut missing = (0..=max_exact)
        .filter(|length| !exact_lengths.contains(length))
        .map(list_length_pattern)
        .collect::<Vec<_>>();
    missing.push(format!("list length >= {}", max_exact + 1));
    missing
}

fn list_length_pattern(length: usize) -> String {
    if length == 0 {
        "[]".to_owned()
    } else {
        format!("list length {length}")
    }
}

#[derive(Default)]
struct TypeParameters {
    variables: BTreeMap<String, TypeVar>,
    order: Vec<String>,
}

impl TypeParameters {
    fn get_or_insert(&mut self, name: &str, fresh: impl FnOnce() -> TypeVar) -> TypeVar {
        if let Some(var) = self.variables.get(name) {
            return *var;
        }
        let var = fresh();
        self.variables.insert(name.to_owned(), var);
        self.order.push(name.to_owned());
        var
    }

    fn vars(&self) -> Vec<TypeVar> {
        self.order
            .iter()
            .filter_map(|name| self.variables.get(name).copied())
            .collect()
    }

    fn types(&self) -> Vec<Type> {
        self.vars().into_iter().map(Type::Var).collect()
    }
}

fn is_parenthesized(text: &str) -> bool {
    text.starts_with('(') && text.ends_with(')')
}

fn split_type_items(text: &str) -> Result<Vec<&str>, InferError> {
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
                    .ok_or(InferError::NotImplemented("declared type syntax"))?;
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
        return Err(InferError::NotImplemented("declared type syntax"));
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

fn free_type_vars(ty: &Type) -> BTreeSet<TypeVar> {
    let mut vars = BTreeSet::new();
    collect_type_vars(ty, &mut vars);
    vars
}

fn free_scheme_vars(scheme: &Scheme) -> BTreeSet<TypeVar> {
    let mut vars = free_type_vars(&scheme.body);
    for var in &scheme.variables {
        vars.remove(var);
    }
    vars
}

fn collect_type_vars(ty: &Type, vars: &mut BTreeSet<TypeVar>) {
    match ty {
        Type::Var(var) => {
            vars.insert(*var);
        }
        Type::List(item) => collect_type_vars(item, vars),
        Type::Object(row) => {
            if let Some(rest) = row.rest {
                vars.insert(rest);
            }
            for ty in row.fields.values() {
                collect_type_vars(ty, vars);
            }
        }
        Type::Function { parameters, result } => {
            for parameter in parameters {
                collect_type_vars(parameter, vars);
            }
            collect_type_vars(result, vars);
        }
        Type::Named { arguments, .. } => {
            for argument in arguments {
                collect_type_vars(argument, vars);
            }
        }
        Type::Never | Type::Null | Type::Bool | Type::Int | Type::Float | Type::Str => {}
    }
}

#[cfg(test)]
#[path = "infer_test.rs"]
mod infer_test;

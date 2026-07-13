use std::collections::{BTreeMap, BTreeSet, HashMap};

use jisp_core::Span;
use jisp_ir::{Expr, ExprKind, Import, Literal, Module, StringPart, TypeDecl};
use thiserror::Error;

use crate::top_level::definition_groups;
use crate::{ObjectRow, Scheme, Type, TypeVar, TypedModule, Unifier, UnifyError};

#[path = "infer_case.rs"]
mod infer_case;

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

    #[error("all alternatives in an or pattern must bind the same names")]
    InconsistentAlternativeBindings,

    #[error("non-exhaustive case for `{type_name}`, missing patterns: {missing:?}")]
    NonExhaustiveCase {
        type_name: String,
        missing: Vec<String>,
    },

    #[error("redundant case pattern `{0}`")]
    RedundantCasePattern(String),

    #[error("no overload of `{name}` matches the arguments; expected {expected}")]
    NoMatchingOverload { name: String, expected: String },

    #[error("{error}")]
    Located {
        #[source]
        error: Box<InferError>,
        span: Span,
    },
}

impl InferError {
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::Located { span, .. } => Some(*span),
            _ => None,
        }
    }

    fn locate(self, span: Span) -> Self {
        match self {
            Self::Located { .. } => self,
            error => Self::Located {
                error: Box::new(error),
                span,
            },
        }
    }

    fn into_unlocated(self) -> Self {
        match self {
            Self::Located { error, .. } => error.into_unlocated(),
            error => error,
        }
    }
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
    expression_types: HashMap<Span, Type>,
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
            .map_err(InferError::into_unlocated)
    }

    pub fn infer_typed_module(&mut self, module: Module) -> Result<TypedModule, InferError> {
        self.infer_typed_module_with_imports(module, &BTreeMap::new())
            .map_err(InferError::into_unlocated)
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
                let value = self.infer_expr_located(&definition.value)?;
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

    pub fn infer_typed_module_with_imports(
        &mut self,
        module: Module,
        imports: &ImportTypeEnvironments,
    ) -> Result<TypedModule, InferError> {
        self.expression_types.clear();
        let schemes = self.infer_module_with_imports(&module, imports)?;
        let expression_types = self
            .expression_types
            .iter()
            .map(|(span, ty)| (*span, self.apply(ty)))
            .collect();
        Ok(TypedModule {
            module,
            schemes,
            expression_types,
        })
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
        self.infer_expr_located(expr)
            .map_err(InferError::into_unlocated)
    }

    fn infer_expr_located(&mut self, expr: &Expr) -> Result<Type, InferError> {
        let result: Result<Type, InferError> = (|| {
            let ty = match &expr.kind {
                ExprKind::Literal(literal) => self.infer_literal(literal),
                ExprKind::Name(name) => self.lookup(name)?,
                ExprKind::Lambda { params, rest, body } => self.with_scope(|inferencer| {
                    let parameters = params
                        .iter()
                        .map(|name| {
                            let ty = inferencer.fresh_type();
                            inferencer.define(name, Scheme::mono(ty.clone()));
                            ty
                        })
                        .collect::<Vec<_>>();
                    let rest_item = rest.as_ref().map(|name| {
                        let ty = inferencer.fresh_type();
                        inferencer.define(name, Scheme::mono(Type::List(Box::new(ty.clone()))));
                        ty
                    });
                    let result = inferencer.infer_expr_located(body)?;
                    Ok(Type::Function {
                        parameters: parameters.iter().map(|ty| inferencer.apply(ty)).collect(),
                        rest: rest_item.map(|ty| Box::new(inferencer.apply(&ty))),
                        result: Box::new(inferencer.apply(&result)),
                    })
                })?,
                ExprKind::Let { bindings, body } => self.infer_let(bindings, body)?,
                ExprKind::Do(expressions) => self.infer_do(expressions)?,
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    self.infer_expr_located(condition)?;
                    let then_ty = self.infer_expr_located(then_branch)?;
                    let else_ty = self.infer_expr_located(else_branch)?;
                    self.unify(then_ty, else_ty)?
                }
                ExprKind::And(expressions) => self.infer_short_circuit(expressions, Type::Bool)?,
                ExprKind::Or(expressions) => self.infer_short_circuit(expressions, Type::Null)?,
                ExprKind::Not(expression) => {
                    self.infer_expr_located(expression)?;
                    Type::Bool
                }
                ExprKind::Call { callee, arguments } => {
                    if let ExprKind::Name(name) = &callee.kind {
                        if name == "map" {
                            self.infer_map(arguments)?
                        } else if let Some(overloads) = self.overloads.get(name).cloned() {
                            self.infer_overloaded_call(name, &overloads, arguments)?
                        } else if self.can_specialize_object_builtin(name) {
                            let mut candidate = self.clone();
                            if let Some(result) = candidate.infer_object_builtin(name, arguments)? {
                                *self = candidate;
                                self.apply(&result)
                            } else {
                                self.infer_call(callee, arguments)?
                            }
                        } else {
                            self.infer_call(callee, arguments)?
                        }
                    } else {
                        self.infer_call(callee, arguments)?
                    }
                }
                ExprKind::List(expressions) => {
                    let item = self.fresh_type();
                    for expression in expressions {
                        let value = self.infer_expr_located(expression)?;
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
                                let ty = self.infer_expr_located(expression)?;
                                self.unify(ty, Type::Str)?;
                            }
                            StringPart::Splice(expression) => {
                                let ty = self.infer_expr_located(expression)?;
                                self.unify(ty, Type::List(Box::new(Type::Str)))?;
                            }
                        }
                    }
                    Type::Str
                }
                ExprKind::Case { subject, branches } => self.infer_case(subject, branches)?,
            };
            Ok(self.apply(&ty))
        })();
        match result {
            Ok(ty) => {
                self.expression_types.insert(expr.span, ty.clone());
                Ok(ty)
            }
            Err(error) => Err(error.locate(expr.span)),
        }
    }

    fn infer_call(&mut self, callee: &Expr, arguments: &[Expr]) -> Result<Type, InferError> {
        let callee_ty = self.infer_expr_located(callee)?;
        let parameters = arguments
            .iter()
            .map(|argument| self.infer_expr_located(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.fresh_type();
        self.unify(
            callee_ty,
            Type::Function {
                parameters,
                rest: None,
                result: Box::new(result.clone()),
            },
        )?;
        Ok(result)
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
                        rest: None,
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
            "bigint" => Type::BigInt,
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
                } else if *head == "map" && tail.len() == 2 && tail[0] == "str" {
                    Type::Map(Box::new(self.declared_type(tail[1], parameters)?))
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
            _ if is_type_name(text) && self.type_variants.contains_key(text) => Type::Named {
                name: text.to_owned(),
                arguments: vec![],
            },
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
            // Plain integer literals intentionally stay checked i64. Use
            // `(bigint "...")` when a value must exceed that range.
            Literal::Float(_) => Type::Float,
            Literal::String(_) => Type::Str,
        }
    }

    fn infer_let(&mut self, bindings: &[(String, Expr)], body: &Expr) -> Result<Type, InferError> {
        self.with_scope(|inferencer| {
            for (name, value) in bindings {
                let ty = inferencer.infer_expr_located(value)?;
                let scheme = inferencer.generalize(&ty);
                inferencer.define(name, scheme);
            }
            inferencer.infer_expr_located(body)
        })
    }

    fn infer_do(&mut self, expressions: &[Expr]) -> Result<Type, InferError> {
        let mut ty = Type::Null;
        for expression in expressions {
            ty = self.infer_expr_located(expression)?;
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
        let ty = self.infer_expr_located(first)?;
        for expression in rest {
            let next = self.infer_expr_located(expression)?;
            self.unify(ty.clone(), next)?;
        }
        Ok(ty)
    }

    fn infer_object(&mut self, fields: &[(Expr, Expr)]) -> Result<Type, InferError> {
        let mut typed_fields = BTreeMap::new();
        let mut dynamic = false;
        for (key, value) in fields {
            let key_ty = self.infer_expr_located(key)?;
            self.unify(key_ty, Type::Str)?;
            let value_ty = self.infer_expr_located(value)?;
            if let Some(name) = static_string_key(key) {
                typed_fields.insert(name, value_ty);
            } else {
                dynamic = true;
            }
        }
        Ok(Type::Object(ObjectRow {
            fields: typed_fields,
            rest: dynamic.then(|| self.fresh_var()),
        }))
    }

    fn infer_map(&mut self, arguments: &[Expr]) -> Result<Type, InferError> {
        if !arguments.len().is_multiple_of(2) {
            return Err(InferError::NotImplemented(
                "map expects alternating key and value expressions",
            ));
        }
        let value = self.fresh_type();
        for pair in arguments.chunks_exact(2) {
            let key_ty = self.infer_expr_located(&pair[0])?;
            self.unify(key_ty, Type::Str)?;
            let value_ty = self.infer_expr_located(&pair[1])?;
            self.unify(value.clone(), value_ty)?;
        }
        Ok(Type::Map(Box::new(self.apply(&value))))
    }

    fn infer_field(&mut self, object: &Expr, key: &Expr) -> Result<Type, InferError> {
        let key_ty = self.infer_expr_located(key)?;
        self.unify(key_ty, Type::Str)?;
        let object_ty = self.infer_expr_located(object)?;
        if static_string_key(key).is_none() {
            if let Type::Object(row) = self.apply(&object_ty) {
                if let Some(field) = homogeneous_closed_field_type(&row) {
                    return Ok(field);
                }
                if row.rest.is_none() {
                    return Err(InferError::NoMatchingOverload {
                        name: ".".to_owned(),
                        expected: "static field or homogeneous closed object".to_owned(),
                    });
                }
            }
        }
        let field_ty = self.fresh_type();
        let rest = self.fresh_var();
        let fields = static_string_key(key)
            .map(|name| BTreeMap::from([(name, field_ty.clone())]))
            .unwrap_or_default();
        self.unify(
            object_ty,
            Type::Object(ObjectRow {
                fields,
                rest: Some(rest),
            }),
        )?;
        Ok(field_ty)
    }

    fn infer_object_builtin(
        &mut self,
        name: &str,
        arguments: &[Expr],
    ) -> Result<Option<Type>, InferError> {
        match name {
            "obj.get" => self.infer_obj_get(arguments),
            "obj.set" => self.infer_obj_set(arguments),
            "obj.del" => self.infer_obj_del(arguments),
            "obj.values" => self.infer_obj_values(arguments),
            "obj.to-map" => self.infer_obj_to_map(arguments),
            "obj.cat" => self.infer_obj_cat(arguments),
            _ => Ok(None),
        }
    }

    fn infer_obj_get(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        require_arity(arguments, 2)?;
        let key_ty = self.infer_expr_located(&arguments[1])?;
        self.unify(key_ty, Type::Str)?;
        let Some(row) = self.infer_static_object_row(&arguments[0])? else {
            return Ok(None);
        };
        let field = if let Some(key) = static_string_key(&arguments[1]) {
            row.fields.get(&key).cloned()
        } else {
            homogeneous_closed_field_type(&row)
        };
        let Some(field) = field else {
            if static_string_key(&arguments[1]).is_none() && row.rest.is_none() {
                return Err(InferError::NoMatchingOverload {
                    name: "obj.get".to_owned(),
                    expected: "static field or homogeneous closed object".to_owned(),
                });
            }
            return Ok(None);
        };
        Ok(Some(result_type(field, Type::Str)))
    }

    fn infer_obj_set(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        require_arity(arguments, 3)?;
        let key_ty = self.infer_expr_located(&arguments[1])?;
        self.unify(key_ty, Type::Str)?;
        let Some(mut row) = self.infer_static_object_row(&arguments[0])? else {
            return Ok(None);
        };
        let value = self.infer_expr_located(&arguments[2])?;
        if let Some(key) = static_string_key(&arguments[1]) {
            row.fields.insert(key, self.apply(&value));
        } else {
            let Some(field) = homogeneous_closed_field_type(&row) else {
                return Ok(None);
            };
            let mut candidate = self.clone();
            if candidate.unify(value, field).is_err() {
                return Ok(None);
            }
            *self = candidate;
        }
        Ok(Some(Type::Object(row)))
    }

    fn infer_obj_del(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        require_arity(arguments, 2)?;
        let Some(key) = static_string_key(&arguments[1]) else {
            return Ok(None);
        };
        let key_ty = self.infer_expr_located(&arguments[1])?;
        self.unify(key_ty, Type::Str)?;
        let Some(mut row) = self.infer_static_object_row(&arguments[0])? else {
            return Ok(None);
        };
        row.fields.remove(&key);
        Ok(Some(Type::Object(row)))
    }

    fn infer_obj_values(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        require_arity(arguments, 1)?;
        let Some(row) = self.infer_static_object_row(&arguments[0])? else {
            return Ok(None);
        };
        if row.rest.is_some() || row.fields.is_empty() {
            return Ok(None);
        }
        let item = self.fresh_type();
        for value in row.fields.values() {
            if self.unify(item.clone(), value.clone()).is_err() {
                return Ok(None);
            }
        }
        Ok(Some(Type::List(Box::new(self.apply(&item)))))
    }

    fn infer_obj_to_map(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        require_arity(arguments, 1)?;
        let ty = self.infer_expr_located(&arguments[0])?;
        let Type::Object(row) = self.apply(&ty) else {
            return Ok(None);
        };
        if row.rest.is_some() {
            return Err(InferError::NoMatchingOverload {
                name: "obj.to-map".to_owned(),
                expected: "homogeneous closed object".to_owned(),
            });
        }
        let Some(field) = homogeneous_closed_field_type(&row) else {
            return Err(InferError::NoMatchingOverload {
                name: "obj.to-map".to_owned(),
                expected: "homogeneous closed object".to_owned(),
            });
        };
        Ok(Some(Type::Map(Box::new(field))))
    }

    fn infer_obj_cat(&mut self, arguments: &[Expr]) -> Result<Option<Type>, InferError> {
        let mut fields = BTreeMap::new();
        for argument in arguments {
            let Some(row) = self.infer_static_object_row(argument)? else {
                return Ok(None);
            };
            if row.rest.is_some() {
                return Ok(None);
            }
            fields.extend(row.fields);
        }
        Ok(Some(Type::Object(ObjectRow { fields, rest: None })))
    }

    fn infer_static_object_row(&mut self, object: &Expr) -> Result<Option<ObjectRow>, InferError> {
        let ty = self.infer_expr_located(object)?;
        match self.apply(&ty) {
            Type::Object(row) => Ok(Some(row)),
            _ => Ok(None),
        }
    }

    fn can_specialize_object_builtin(&self, name: &str) -> bool {
        matches!(
            name,
            "obj.get" | "obj.set" | "obj.del" | "obj.values" | "obj.to-map" | "obj.cat"
        ) && crate::prelude::environment()
            .get(name)
            .is_some_and(|scheme| self.environment.get(name) == Some(scheme))
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
        name: &str,
        overloads: &[Scheme],
        arguments: &[Expr],
    ) -> Result<Type, InferError> {
        for overload in overloads {
            let mut candidate = self.clone();
            if let Ok(result) = candidate.infer_call_with_scheme(overload, arguments) {
                *self = candidate;
                return Ok(self.apply(&result));
            }
        }

        Err(InferError::NoMatchingOverload {
            name: name.to_owned(),
            expected: overloads
                .iter()
                .map(|overload| overload.body.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        })
    }

    fn infer_call_with_scheme(
        &mut self,
        scheme: &Scheme,
        arguments: &[Expr],
    ) -> Result<Type, InferError> {
        let callee_ty = self.instantiate(scheme);
        let parameters = arguments
            .iter()
            .map(|argument| self.infer_expr_located(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.fresh_type();
        self.unify(
            callee_ty,
            Type::Function {
                parameters,
                rest: None,
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
        Type::Map(value) => Type::Map(Box::new(replace(value, replacements))),
        Type::Object(row) => Type::Object(crate::ObjectRow {
            fields: row
                .fields
                .iter()
                .map(|(name, ty)| (name.clone(), replace(ty, replacements)))
                .collect(),
            rest: row.rest,
        }),
        Type::Function {
            parameters,
            rest,
            result,
        } => Type::Function {
            parameters: parameters
                .iter()
                .map(|ty| replace(ty, replacements))
                .collect(),
            rest: rest.as_ref().map(|ty| Box::new(replace(ty, replacements))),
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

fn homogeneous_closed_field_type(row: &ObjectRow) -> Option<Type> {
    if row.rest.is_some() || row.fields.is_empty() {
        return None;
    }
    let field = row.fields.values().next()?.clone();
    row.fields
        .values()
        .all(|candidate| candidate == &field)
        .then_some(field)
}

fn require_arity(arguments: &[Expr], expected: usize) -> Result<(), InferError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(UnifyError::Arity {
            left: expected,
            right: arguments.len(),
        }
        .into())
    }
}

fn result_type(ok: Type, err: Type) -> Type {
    Type::Named {
        name: "result".to_owned(),
        arguments: vec![ok, err],
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
        Type::Map(value) => collect_type_vars(value, vars),
        Type::Object(row) => {
            if let Some(rest) = row.rest {
                vars.insert(rest);
            }
            for ty in row.fields.values() {
                collect_type_vars(ty, vars);
            }
        }
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            for parameter in parameters {
                collect_type_vars(parameter, vars);
            }
            if let Some(rest) = rest {
                collect_type_vars(rest, vars);
            }
            collect_type_vars(result, vars);
        }
        Type::Named { arguments, .. } => {
            for argument in arguments {
                collect_type_vars(argument, vars);
            }
        }
        Type::Never
        | Type::Null
        | Type::Bool
        | Type::Int
        | Type::BigInt
        | Type::Float
        | Type::Str => {}
    }
}

#[cfg(test)]
#[path = "infer_test.rs"]
mod infer_test;

use std::collections::{BTreeMap, BTreeSet};

use jisp_ir::{Expr, ExprKind, Literal, StringPart};
use thiserror::Error;

use crate::{ObjectRow, Scheme, Type, TypeVar, Unifier, UnifyError};

#[derive(Clone, Debug, Error)]
pub enum InferError {
    #[error("unknown name `{0}`")]
    UnknownName(String),

    #[error(transparent)]
    Unify(#[from] UnifyError),

    #[error("full Core IR inference is not implemented yet: {0}")]
    NotImplemented(&'static str),
}

/// Reusable state for Hindley–Milner-style inference.
///
/// The unification engine is implemented and tested. The next implementation
/// step is to add `infer_expr` over `jisp_ir::Expr`, generalize `let` bindings,
/// instantiate schemes at use sites, and infer recursive SCCs monomorphically
/// before generalization.
#[derive(Clone, Debug, Default)]
pub struct Inferencer {
    next_var: u32,
    pub unifier: Unifier,
    pub environment: BTreeMap<String, Scheme>,
}

impl Inferencer {
    pub fn fresh_type(&mut self) -> Type {
        let var = TypeVar(self.next_var);
        self.next_var += 1;
        Type::Var(var)
    }

    pub fn define(&mut self, name: impl Into<String>, scheme: Scheme) {
        self.environment.insert(name.into(), scheme);
    }

    pub fn lookup(&mut self, name: &str) -> Result<Type, InferError> {
        let scheme = self
            .environment
            .get(name)
            .cloned()
            .ok_or_else(|| InferError::UnknownName(name.to_owned()))?;
        Ok(self.instantiate(&scheme))
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
            ExprKind::Case { .. } => {
                return Err(InferError::NotImplemented(
                    "case exhaustiveness and pattern types",
                ));
            }
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
        let mut variables = free_type_vars(&ty);
        for var in self.environment.values().flat_map(free_scheme_vars) {
            variables.remove(&var);
        }
        Scheme {
            variables: variables.into_iter().collect(),
            body: ty,
        }
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
        let result = f(self);
        self.environment = environment;
        result
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
mod tests {
    use super::*;
    use jisp_core::{SourceId, Span};

    fn span() -> Span {
        Span::empty(SourceId(0), 0)
    }

    fn expr(kind: ExprKind) -> Expr {
        Expr::new(kind, span())
    }

    fn name(value: &str) -> Expr {
        expr(ExprKind::Name(value.to_owned()))
    }

    fn int(value: i64) -> Expr {
        expr(ExprKind::Literal(Literal::Int(value)))
    }

    fn string(value: &str) -> Expr {
        expr(ExprKind::Literal(Literal::String(value.to_owned())))
    }

    #[test]
    fn instantiation_creates_fresh_variables() {
        let mut inferencer = Inferencer::default();
        let scheme = Scheme {
            variables: vec![TypeVar(99)],
            body: Type::Function {
                parameters: vec![Type::Var(TypeVar(99))],
                result: Box::new(Type::Var(TypeVar(99))),
            },
        };
        let first = inferencer.instantiate(&scheme);
        let second = inferencer.instantiate(&scheme);
        assert_ne!(first, second);
    }

    #[test]
    fn infers_function_calls() {
        let mut inferencer = Inferencer::default();
        let expression = expr(ExprKind::Call {
            callee: Box::new(expr(ExprKind::Lambda {
                params: vec!["value".to_owned()],
                rest: None,
                body: Box::new(name("value")),
            })),
            arguments: vec![int(1)],
        });

        assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
    }

    #[test]
    fn generalizes_let_bindings() {
        let mut inferencer = Inferencer::default();
        let identity = expr(ExprKind::Lambda {
            params: vec!["value".to_owned()],
            rest: None,
            body: Box::new(name("value")),
        });
        let expression = expr(ExprKind::Let {
            bindings: vec![("id".to_owned(), identity)],
            body: Box::new(expr(ExprKind::Do(vec![
                expr(ExprKind::Call {
                    callee: Box::new(name("id")),
                    arguments: vec![int(1)],
                }),
                expr(ExprKind::Call {
                    callee: Box::new(name("id")),
                    arguments: vec![string("ok")],
                }),
            ]))),
        });

        assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
    }

    #[test]
    fn infers_static_object_fields() {
        let mut inferencer = Inferencer::default();
        let expression = expr(ExprKind::Field {
            object: Box::new(expr(ExprKind::Object(vec![(
                string("name"),
                string("Ada"),
            )]))),
            key: Box::new(string("name")),
        });

        assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
    }
}

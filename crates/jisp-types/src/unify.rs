use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{ObjectRow, Type, TypeVar};

#[derive(Clone, Debug, Default)]
pub struct Substitution {
    bindings: BTreeMap<TypeVar, Type>,
}

impl Substitution {
    pub fn get(&self, var: TypeVar) -> Option<&Type> {
        self.bindings.get(&var)
    }

    pub fn insert(&mut self, var: TypeVar, ty: Type) {
        self.bindings.insert(var, ty);
    }

    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(var) => self
                .bindings
                .get(var)
                .map(|bound| self.apply(bound))
                .unwrap_or(Type::Var(*var)),
            Type::List(item) => Type::List(Box::new(self.apply(item))),
            Type::Map(value) => Type::Map(Box::new(self.apply(value))),
            Type::Object(row) => Type::Object(ObjectRow {
                fields: row
                    .fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.apply(ty)))
                    .collect(),
                rest: row.rest,
            }),
            Type::Function {
                parameters,
                rest,
                result,
            } => Type::Function {
                parameters: parameters.iter().map(|ty| self.apply(ty)).collect(),
                rest: rest.as_ref().map(|ty| Box::new(self.apply(ty))),
                result: Box::new(self.apply(result)),
            },
            Type::Named { name, arguments } => Type::Named {
                name: name.clone(),
                arguments: arguments.iter().map(|ty| self.apply(ty)).collect(),
            },
            other => other.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Unifier {
    pub substitution: Substitution,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum UnifyError {
    #[error("cannot unify {left} with {right}")]
    Mismatch { left: Type, right: Type },

    #[error("recursive type: {var:?} occurs in {ty}")]
    Occurs { var: TypeVar, ty: Type },

    #[error("function arity mismatch: {left} versus {right}")]
    Arity { left: usize, right: usize },

    #[error("object field `{0}` is missing")]
    MissingField(String),
}

impl Unifier {
    pub fn unify(&mut self, left: Type, right: Type) -> Result<Type, UnifyError> {
        let left = self.substitution.apply(&left);
        let right = self.substitution.apply(&right);

        match (left, right) {
            (Type::Var(a), Type::Var(b)) if a == b => Ok(Type::Var(a)),
            (Type::Var(var), ty) | (ty, Type::Var(var)) => self.bind(var, ty),
            (Type::Never, ty) | (ty, Type::Never) => Ok(ty),
            (Type::Null, Type::Null) => Ok(Type::Null),
            (Type::Bool, Type::Bool) => Ok(Type::Bool),
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::BigInt, Type::BigInt) => Ok(Type::BigInt),
            (Type::Float, Type::Float) => Ok(Type::Float),
            (Type::Str, Type::Str) => Ok(Type::Str),
            (Type::List(a), Type::List(b)) => Ok(Type::List(Box::new(self.unify(*a, *b)?))),
            (Type::Map(a), Type::Map(b)) => Ok(Type::Map(Box::new(self.unify(*a, *b)?))),
            (
                Type::Function {
                    parameters: left_parameters,
                    rest: left_rest,
                    result: left_result,
                },
                Type::Function {
                    parameters: right_parameters,
                    rest: right_rest,
                    result: right_result,
                },
            ) => {
                if !function_arities_overlap(
                    left_parameters.len(),
                    left_rest.is_some(),
                    right_parameters.len(),
                    right_rest.is_some(),
                ) {
                    return Err(UnifyError::Arity {
                        left: left_parameters.len(),
                        right: right_parameters.len(),
                    });
                }
                let parameters = self.unify_function_parameters(
                    &left_parameters,
                    &left_rest,
                    &right_parameters,
                )?;
                if let Some(right_rest) = right_rest.as_deref() {
                    if let Some(left_rest) = left_rest.as_deref() {
                        self.unify(left_rest.clone(), right_rest.clone())?;
                    }
                    for left in left_parameters.iter().skip(right_parameters.len()) {
                        self.unify(left.clone(), right_rest.clone())?;
                    }
                }
                let rest = match (left_rest, right_rest) {
                    (Some(left), Some(right)) => Some(Box::new(self.unify(*left, *right)?)),
                    (Some(left), None) => Some(left),
                    (None, Some(right)) => Some(right),
                    (None, None) => None,
                };
                let result = self.unify(*left_result, *right_result)?;
                Ok(Type::Function {
                    parameters,
                    rest,
                    result: Box::new(result),
                })
            }
            (Type::Object(left), Type::Object(right)) => self.unify_objects(left, right),
            (
                Type::Named {
                    name: left_name,
                    arguments: left_arguments,
                },
                Type::Named {
                    name: right_name,
                    arguments: right_arguments,
                },
            ) if left_name == right_name && left_arguments.len() == right_arguments.len() => {
                let arguments = left_arguments
                    .into_iter()
                    .zip(right_arguments)
                    .map(|(left, right)| self.unify(left, right))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Type::Named {
                    name: left_name,
                    arguments,
                })
            }
            (left, right) => Err(UnifyError::Mismatch { left, right }),
        }
    }

    fn bind(&mut self, var: TypeVar, ty: Type) -> Result<Type, UnifyError> {
        if ty == Type::Var(var) {
            return Ok(ty);
        }
        if occurs(var, &ty, &self.substitution) {
            return Err(UnifyError::Occurs { var, ty });
        }
        self.substitution.insert(var, ty.clone());
        Ok(ty)
    }

    fn unify_objects(&mut self, left: ObjectRow, right: ObjectRow) -> Result<Type, UnifyError> {
        let names: BTreeSet<_> = left
            .fields
            .keys()
            .chain(right.fields.keys())
            .cloned()
            .collect();

        let mut fields = BTreeMap::new();
        for name in names {
            match (left.fields.get(&name), right.fields.get(&name)) {
                (Some(left), Some(right)) => {
                    fields.insert(name, self.unify(left.clone(), right.clone())?);
                }
                (Some(ty), None) if right.rest.is_some() => {
                    fields.insert(name, ty.clone());
                }
                (None, Some(ty)) if left.rest.is_some() => {
                    fields.insert(name, ty.clone());
                }
                _ => return Err(UnifyError::MissingField(name)),
            }
        }

        Ok(Type::Object(ObjectRow {
            fields,
            rest: left.rest.or(right.rest),
        }))
    }
}

fn occurs(var: TypeVar, ty: &Type, substitution: &Substitution) -> bool {
    match substitution.apply(ty) {
        Type::Var(other) => var == other,
        Type::List(item) => occurs(var, &item, substitution),
        Type::Map(value) => occurs(var, &value, substitution),
        Type::Object(row) => {
            row.rest == Some(var) || row.fields.values().any(|ty| occurs(var, ty, substitution))
        }
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            parameters.iter().any(|ty| occurs(var, ty, substitution))
                || rest
                    .as_ref()
                    .is_some_and(|ty| occurs(var, ty, substitution))
                || occurs(var, &result, substitution)
        }
        Type::Named { arguments, .. } => arguments.iter().any(|ty| occurs(var, ty, substitution)),
        _ => false,
    }
}

impl Unifier {
    fn unify_function_parameters(
        &mut self,
        left_parameters: &[Type],
        left_rest: &Option<Box<Type>>,
        right_parameters: &[Type],
    ) -> Result<Vec<Type>, UnifyError> {
        let mut parameters = Vec::with_capacity(left_parameters.len().max(right_parameters.len()));
        let shared = left_parameters.len().min(right_parameters.len());

        for index in 0..shared {
            parameters.push(self.unify(
                left_parameters[index].clone(),
                right_parameters[index].clone(),
            )?);
        }

        if left_parameters.len() > right_parameters.len() {
            parameters.extend_from_slice(&left_parameters[shared..]);
        } else if let Some(left_rest) = left_rest.as_deref() {
            for right in &right_parameters[shared..] {
                parameters.push(self.unify(left_rest.clone(), right.clone())?);
            }
        }

        Ok(parameters)
    }
}

fn function_arities_overlap(
    left_fixed: usize,
    left_variadic: bool,
    right_fixed: usize,
    right_variadic: bool,
) -> bool {
    match (left_variadic, right_variadic) {
        (false, false) => left_fixed == right_fixed,
        (true, false) => left_fixed <= right_fixed,
        (false, true) => right_fixed <= left_fixed,
        (true, true) => true,
    }
}

#[cfg(test)]
#[path = "unify_test.rs"]
mod unify_test;

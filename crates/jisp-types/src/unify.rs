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
            Type::Object(row) => Type::Object(ObjectRow {
                fields: row
                    .fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.apply(ty)))
                    .collect(),
                rest: row.rest,
            }),
            Type::Function { parameters, result } => Type::Function {
                parameters: parameters.iter().map(|ty| self.apply(ty)).collect(),
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
            (Type::Float, Type::Float) => Ok(Type::Float),
            (Type::Str, Type::Str) => Ok(Type::Str),
            (Type::List(a), Type::List(b)) => {
                Ok(Type::List(Box::new(self.unify(*a, *b)?)))
            }
            (
                Type::Function {
                    parameters: left_parameters,
                    result: left_result,
                },
                Type::Function {
                    parameters: right_parameters,
                    result: right_result,
                },
            ) => {
                if left_parameters.len() != right_parameters.len() {
                    return Err(UnifyError::Arity {
                        left: left_parameters.len(),
                        right: right_parameters.len(),
                    });
                }
                let parameters = left_parameters
                    .into_iter()
                    .zip(right_parameters)
                    .map(|(left, right)| self.unify(left, right))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.unify(*left_result, *right_result)?;
                Ok(Type::Function {
                    parameters,
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
        Type::Object(row) => {
            row.rest == Some(var)
                || row
                    .fields
                    .values()
                    .any(|ty| occurs(var, ty, substitution))
        }
        Type::Function { parameters, result } => {
            parameters
                .iter()
                .any(|ty| occurs(var, ty, substitution))
                || occurs(var, &result, substitution)
        }
        Type::Named { arguments, .. } => arguments
            .iter()
            .any(|ty| occurs(var, ty, substitution)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_a_variable_inside_a_list() {
        let var = TypeVar(0);
        let mut unifier = Unifier::default();
        let result = unifier
            .unify(
                Type::List(Box::new(Type::Var(var))),
                Type::List(Box::new(Type::Int)),
            )
            .unwrap();
        assert_eq!(result, Type::List(Box::new(Type::Int)));
        assert_eq!(unifier.substitution.get(var), Some(&Type::Int));
    }

    #[test]
    fn rejects_recursive_types() {
        let var = TypeVar(0);
        let mut unifier = Unifier::default();
        assert!(matches!(
            unifier.unify(Type::Var(var), Type::List(Box::new(Type::Var(var)))),
            Err(UnifyError::Occurs { .. })
        ));
    }

    #[test]
    fn unifies_function_types() {
        let mut unifier = Unifier::default();
        let variable = Type::Var(TypeVar(0));
        let result = unifier
            .unify(
                Type::Function {
                    parameters: vec![variable.clone()],
                    result: Box::new(variable),
                },
                Type::Function {
                    parameters: vec![Type::Str],
                    result: Box::new(Type::Str),
                },
            )
            .unwrap();
        assert_eq!(
            result,
            Type::Function {
                parameters: vec![Type::Str],
                result: Box::new(Type::Str)
            }
        );
    }
}

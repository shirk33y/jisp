use std::collections::BTreeMap;

use thiserror::Error;

use crate::{Scheme, Type, TypeVar, Unifier};

#[derive(Clone, Debug, Error)]
pub enum InferError {
    #[error("unknown name `{0}`")]
    UnknownName(String),

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

    pub fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let replacements: BTreeMap<_, _> = scheme
            .variables
            .iter()
            .copied()
            .map(|var| (var, self.fresh_type()))
            .collect();
        replace(&scheme.body, &replacements)
    }
}

fn replace(ty: &Type, replacements: &BTreeMap<TypeVar, Type>) -> Type {
    match ty {
        Type::Var(var) => replacements
            .get(var)
            .cloned()
            .unwrap_or(Type::Var(*var)),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

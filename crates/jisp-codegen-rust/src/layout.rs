use std::collections::BTreeMap;

use jisp_types::{ObjectRow, Scheme, Type, TypeVar, TypedModule};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModuleLayout {
    pub definitions: BTreeMap<String, Layout>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    Null,
    Bool,
    Int,
    BigInt,
    Float,
    Str,
    List(Box<Layout>),
    ClosedObject(ClosedObjectLayout),
    Function {
        parameters: Vec<Layout>,
        rest: Option<Box<Layout>>,
        result: Box<Layout>,
    },
    Named {
        name: String,
        arguments: Vec<Layout>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClosedObjectLayout {
    pub fields: BTreeMap<String, Layout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub(crate) enum LayoutError {
    #[error("native codegen does not support `never` values yet")]
    Never,

    #[error("native codegen does not support unresolved type variable `t{0}`")]
    UnresolvedTypeVariable(u32),

    #[error("native codegen does not support polymorphic definition `{name}` yet")]
    PolymorphicDefinition {
        name: String,
        variables: Vec<TypeVar>,
    },

    #[error("native codegen does not support open object rows yet")]
    OpenObjectRow { rest: TypeVar },
}

pub(crate) fn classify_module(module: &TypedModule) -> Result<ModuleLayout, LayoutError> {
    let definitions = module
        .schemes
        .iter()
        .map(|(name, scheme)| Ok((name.clone(), classify_scheme(name, scheme)?)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(ModuleLayout { definitions })
}

fn classify_scheme(name: &str, scheme: &Scheme) -> Result<Layout, LayoutError> {
    if !scheme.variables.is_empty() {
        return Err(LayoutError::PolymorphicDefinition {
            name: name.to_owned(),
            variables: scheme.variables.clone(),
        });
    }
    classify_type(&scheme.body)
}

pub(crate) fn classify_type(ty: &Type) -> Result<Layout, LayoutError> {
    match ty {
        Type::Var(var) => Err(LayoutError::UnresolvedTypeVariable(var.0)),
        Type::Never => Err(LayoutError::Never),
        Type::Null => Ok(Layout::Null),
        Type::Bool => Ok(Layout::Bool),
        Type::Int => Ok(Layout::Int),
        Type::BigInt => Ok(Layout::BigInt),
        Type::Float => Ok(Layout::Float),
        Type::Str => Ok(Layout::Str),
        Type::List(item) => Ok(Layout::List(Box::new(classify_type(item)?))),
        Type::Object(row) => classify_object(row),
        Type::Function {
            parameters,
            rest,
            result,
        } => Ok(Layout::Function {
            parameters: parameters
                .iter()
                .map(classify_type)
                .collect::<Result<_, _>>()?,
            rest: rest
                .as_deref()
                .map(classify_type)
                .transpose()?
                .map(Box::new),
            result: Box::new(classify_type(result)?),
        }),
        Type::Named { name, arguments } => Ok(Layout::Named {
            name: name.clone(),
            arguments: arguments
                .iter()
                .map(classify_type)
                .collect::<Result<_, _>>()?,
        }),
    }
}

fn classify_object(row: &ObjectRow) -> Result<Layout, LayoutError> {
    if let Some(rest) = row.rest {
        return Err(LayoutError::OpenObjectRow { rest });
    }
    let fields = row
        .fields
        .iter()
        .map(|(name, ty)| Ok((name.clone(), classify_type(ty)?)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(Layout::ClosedObject(ClosedObjectLayout { fields }))
}

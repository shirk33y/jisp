use std::collections::BTreeMap;

use jisp_core::{SourceId, Span};
use jisp_types::{ObjectRow, Type, TypedModule};
use proc_macro2::Ident;
use quote::format_ident;

use crate::CodegenError;

#[derive(Clone, Debug)]
pub(super) struct ObjectShape {
    pub(super) fields: BTreeMap<String, Type>,
    pub(super) source_span: Span,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ObjectTypes {
    pub(super) names: BTreeMap<String, Ident>,
    pub(super) shapes: BTreeMap<String, ObjectShape>,
}

impl ObjectTypes {
    pub(super) fn from_module(module: &TypedModule) -> Result<Self, CodegenError> {
        let mut shapes = BTreeMap::new();
        for definition in &module.module.definitions {
            if let Some(scheme) = module.schemes.get(&definition.name) {
                collect_object_shapes(&scheme.body, definition.span, &mut shapes, true)?;
            }
        }
        for scheme in module.schemes.values() {
            collect_object_shapes(&scheme.body, Span::empty(SourceId(0), 0), &mut shapes, true)?;
        }
        let mut expression_types = module.expression_types.iter().collect::<Vec<_>>();
        expression_types.sort_by_key(|(span, _)| (span.source.0, span.start, span.end));
        for (span, ty) in expression_types {
            collect_object_shapes(ty, *span, &mut shapes, false)?;
        }
        let names = shapes
            .keys()
            .enumerate()
            .map(|(index, signature)| (signature.clone(), format_ident!("JispObject{index}")))
            .collect();
        Ok(Self { names, shapes })
    }

    pub(super) fn ident_for_row(&self, row: &ObjectRow) -> Result<Ident, CodegenError> {
        let signature = object_signature(row)?;
        self.names
            .get(&signature)
            .cloned()
            .ok_or(CodegenError::Unsupported("unregistered native object type"))
    }
}

fn collect_object_shapes(
    ty: &Type,
    source_span: Span,
    shapes: &mut BTreeMap<String, ObjectShape>,
    reject_open_rows: bool,
) -> Result<(), CodegenError> {
    match ty {
        Type::Object(row) => {
            if row.rest.is_some() {
                return if reject_open_rows {
                    Err(CodegenError::Unsupported("open object row type emission"))
                } else {
                    Ok(())
                };
            }
            for ty in row.fields.values() {
                collect_object_shapes(ty, source_span, shapes, reject_open_rows)?;
            }
            shapes
                .entry(object_signature(row)?)
                .or_insert_with(|| ObjectShape {
                    fields: row.fields.clone(),
                    source_span,
                });
        }
        Type::List(item) => collect_object_shapes(item, source_span, shapes, reject_open_rows)?,
        Type::Function {
            parameters,
            rest,
            result,
        } => {
            for ty in parameters {
                collect_object_shapes(ty, source_span, shapes, reject_open_rows)?;
            }
            if let Some(rest) = rest {
                collect_object_shapes(rest, source_span, shapes, reject_open_rows)?;
            }
            collect_object_shapes(result, source_span, shapes, reject_open_rows)?;
        }
        Type::Named { arguments, .. } => {
            for ty in arguments {
                collect_object_shapes(ty, source_span, shapes, reject_open_rows)?;
            }
        }
        Type::Var(_)
        | Type::Never
        | Type::Null
        | Type::Bool
        | Type::Int
        | Type::BigInt
        | Type::Float
        | Type::Str => {}
    }
    Ok(())
}

fn object_signature(row: &ObjectRow) -> Result<String, CodegenError> {
    if row.rest.is_some() {
        return Err(CodegenError::Unsupported("open object row type emission"));
    }
    let fields = row
        .fields
        .iter()
        .map(|(name, ty)| Ok(format!("{name}:{}", type_signature(ty)?)))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(format!("{{{fields}}}"))
}

fn type_signature(ty: &Type) -> Result<String, CodegenError> {
    Ok(match ty {
        Type::Null => "null".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::Int => "int".to_owned(),
        Type::BigInt => "bigint".to_owned(),
        Type::Float => "float".to_owned(),
        Type::Str => "str".to_owned(),
        Type::List(item) => format!("list<{}>", type_signature(item)?),
        Type::Object(row) => object_signature(row)?,
        Type::Function { .. } => return Err(CodegenError::Unsupported("function value types")),
        Type::Never => return Err(CodegenError::Unsupported("never type emission")),
        Type::Var(_) => return Err(CodegenError::Unsupported("unresolved type variables")),
        Type::Named { name, arguments } => {
            if arguments.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    arguments
                        .iter()
                        .map(type_signature)
                        .collect::<Result<Vec<_>, _>>()?
                        .join(",")
                )
            }
        }
    })
}

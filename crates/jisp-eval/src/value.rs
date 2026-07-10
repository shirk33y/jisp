use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;
use jisp_core::Span;
use jisp_ir::Expr;
use num_bigint::BigInt;

use crate::{Env, Evaluator, RuntimeError};

pub type BuiltinFn = fn(&mut Evaluator, &[Value], Span) -> Result<Value, RuntimeError>;

#[derive(Clone)]
pub struct Builtin {
    pub name: &'static str,
    pub function: BuiltinFn,
}

#[derive(Clone)]
pub struct Closure {
    pub params: Vec<String>,
    pub rest: Option<String>,
    pub body: Expr,
    pub env: Env,
}

#[derive(Clone, Debug)]
pub struct Constructor {
    pub name: String,
    pub arity: usize,
}

#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    BigInt(BigInt),
    Float(f64),
    Str(Rc<str>),
    List(Vec<Value>),
    Obj(IndexMap<String, Value>),
    Variant { tag: String, fields: Vec<Value> },
    Builtin(Builtin),
    Closure(Closure),
    Constructor(Constructor),
    Uninitialized(String),
}

impl Value {
    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::Str(value.into())
    }

    pub fn truthy(&self) -> bool {
        !matches!(self, Self::Null | Self::Bool(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::BigInt(_) => "bigint",
            Self::Float(_) => "float",
            Self::Str(_) => "str",
            Self::List(_) => "list",
            Self::Obj(_) => "obj",
            Self::Variant { .. } => "enum",
            Self::Builtin(_) | Self::Closure(_) => "fn",
            Self::Constructor(_) => "constructor",
            Self::Uninitialized(_) => "uninitialized",
        }
    }

    pub fn display_string(&self) -> String {
        match self {
            Self::Null => "null".to_owned(),
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::BigInt(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Str(value) => value.to_string(),
            Self::List(values) => {
                let body = values
                    .iter()
                    .map(Value::display_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{body}]")
            }
            Self::Obj(values) => {
                let body = values
                    .iter()
                    .map(|(key, value)| format!("{key:?}: {}", value.display_string()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{body}}}")
            }
            Self::Variant { tag, fields } if fields.is_empty() => format!("[{tag}]"),
            Self::Variant { tag, fields } => {
                let body = fields
                    .iter()
                    .map(Value::display_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{tag}, {body}]")
            }
            Self::Builtin(builtin) => format!("<builtin {}>", builtin.name),
            Self::Closure(_) => "<fn>".to_owned(),
            Self::Constructor(constructor) => format!("<constructor {}>", constructor.name),
            Self::Uninitialized(name) => format!("<uninitialized {name}>"),
        }
    }

    pub fn structurally_equal(&self, other: &Self) -> Result<bool, RuntimeError> {
        match (self, other) {
            (Self::Null, Self::Null) => Ok(true),
            (Self::Bool(a), Self::Bool(b)) => Ok(a == b),
            (Self::Int(a), Self::Int(b)) => Ok(a == b),
            (Self::BigInt(a), Self::BigInt(b)) => Ok(a == b),
            (Self::Float(a), Self::Float(b)) => Ok(a == b),
            (Self::Str(a), Self::Str(b)) => Ok(a == b),
            (Self::List(a), Self::List(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (a, b) in a.iter().zip(b) {
                    if !a.structurally_equal(b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Self::Obj(a), Self::Obj(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (key, a) in a {
                    let Some(b) = b.get(key) else {
                        return Ok(false);
                    };
                    if !a.structurally_equal(b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (
                Self::Variant {
                    tag: a_tag,
                    fields: a_fields,
                },
                Self::Variant {
                    tag: b_tag,
                    fields: b_fields,
                },
            ) => {
                if a_tag != b_tag || a_fields.len() != b_fields.len() {
                    return Ok(false);
                }
                for (a, b) in a_fields.iter().zip(b_fields) {
                    if !a.structurally_equal(b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Self::Builtin(_) | Self::Closure(_), _) | (_, Self::Builtin(_) | Self::Closure(_)) => {
                Err(RuntimeError::message(
                    "functions do not support structural equality",
                ))
            }
            _ => Ok(false),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_string())
    }
}

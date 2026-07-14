//! Decoder for the stable source-data shape of reducer-declared resources.

use indexmap::IndexMap;
use jisp_eval::Value as JispValue;
use serde_json::{Map, Number, Value};

use super::{Capability, Command, DesiredResources, Owner, Subscription};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Invalid { kind: &'static str, message: String },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { kind, message } => {
                write!(formatter, "invalid ui.{kind} descriptor: {message}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug)]
pub enum ReconcileError {
    Decode(DecodeError),
    Host(super::Error),
}

impl std::fmt::Display for ReconcileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::Host(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReconcileError {}

impl From<DecodeError> for ReconcileError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

impl From<super::Error> for ReconcileError {
    fn from(error: super::Error) -> Self {
        Self::Host(error)
    }
}

/// Decode the values produced by `ui.command` and `ui.subscription`. Source
/// descriptors intentionally omit `owner`: source-level resources are app
/// owned until keyed component ownership is implemented.
pub fn decode_resources(
    commands: &[JispValue],
    subscriptions: &[JispValue],
) -> Result<DesiredResources, DecodeError> {
    Ok(DesiredResources {
        commands: commands
            .iter()
            .map(|value| decode_command(value, "command"))
            .collect::<Result<_, _>>()?,
        subscriptions: subscriptions
            .iter()
            .map(|value| decode_subscription(value, "subscription"))
            .collect::<Result<_, _>>()?,
    })
}

fn decode_command(value: &JispValue, kind: &'static str) -> Result<Command, DecodeError> {
    let descriptor = decode_descriptor(value, kind)?;
    Ok(Command {
        id: descriptor.id,
        owner: Owner::App,
        capability: descriptor.capability,
        request: descriptor.request,
        replace: descriptor.replace,
        on_ok: None,
        on_error: None,
    })
}

fn decode_subscription(value: &JispValue, kind: &'static str) -> Result<Subscription, DecodeError> {
    let descriptor = decode_descriptor(value, kind)?;
    Ok(Subscription {
        id: descriptor.id,
        owner: Owner::App,
        capability: descriptor.capability,
        request: descriptor.request,
        replace: descriptor.replace,
        on_ok: None,
        on_error: None,
    })
}

struct Descriptor {
    id: String,
    capability: Capability,
    request: Value,
    replace: bool,
}

fn decode_descriptor(value: &JispValue, kind: &'static str) -> Result<Descriptor, DecodeError> {
    let fields = expect_object(value, kind)?;
    require_exact_fields(fields, kind)?;
    if expect_string(required(fields, "kind", kind)?, "kind", kind)? != kind {
        return invalid(kind, "resource kind does not match its ui.result list");
    }
    let id = expect_string(required(fields, "id", kind)?, "id", kind)?;
    if id.is_empty() {
        return invalid(kind, "id must not be empty");
    }
    let capability_fields = expect_object(required(fields, "capability", kind)?, kind)?;
    if capability_fields.len() != 2
        || !capability_fields.contains_key("name")
        || !capability_fields.contains_key("version")
    {
        return invalid(kind, "capability must contain exactly name and version");
    }
    let name = expect_string(
        required(capability_fields, "name", kind)?,
        "capability.name",
        kind,
    )?;
    if name.is_empty() {
        return invalid(kind, "capability.name must not be empty");
    }
    let version = expect_version(required(capability_fields, "version", kind)?, kind)?;
    let replace = expect_bool(required(fields, "replace", kind)?, "replace", kind)?;
    Ok(Descriptor {
        id,
        capability: Capability { name, version },
        request: json_value(required(fields, "request", kind)?, kind)?,
        replace,
    })
}

fn expect_object<'a>(
    value: &'a JispValue,
    kind: &'static str,
) -> Result<&'a IndexMap<String, JispValue>, DecodeError> {
    let JispValue::Obj(fields) = value else {
        return invalid(kind, "descriptor must be an object");
    };
    Ok(fields)
}

fn require_exact_fields(
    fields: &IndexMap<String, JispValue>,
    kind: &'static str,
) -> Result<(), DecodeError> {
    const FIELDS: [&str; 5] = ["kind", "id", "capability", "request", "replace"];
    if fields.len() != FIELDS.len() || FIELDS.iter().any(|field| !fields.contains_key(*field)) {
        return invalid(
            kind,
            "descriptor must contain exactly kind, id, capability, request, and replace",
        );
    }
    Ok(())
}

fn required<'a>(
    fields: &'a IndexMap<String, JispValue>,
    name: &str,
    kind: &'static str,
) -> Result<&'a JispValue, DecodeError> {
    fields.get(name).ok_or_else(|| DecodeError::Invalid {
        kind,
        message: format!("missing `{name}`"),
    })
}

fn expect_string(
    value: &JispValue,
    field: &str,
    kind: &'static str,
) -> Result<String, DecodeError> {
    let JispValue::Str(value) = value else {
        return invalid(kind, format!("{field} must be a string"));
    };
    Ok(value.to_string())
}

fn expect_bool(value: &JispValue, field: &str, kind: &'static str) -> Result<bool, DecodeError> {
    let JispValue::Bool(value) = value else {
        return invalid(kind, format!("{field} must be a bool"));
    };
    Ok(*value)
}

fn expect_version(value: &JispValue, kind: &'static str) -> Result<u32, DecodeError> {
    let JispValue::Int(value) = value else {
        return invalid(kind, "capability.version must be a positive int");
    };
    u32::try_from(*value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| DecodeError::Invalid {
            kind,
            message: "capability.version must be a positive u32".to_owned(),
        })
}

fn json_value(value: &JispValue, kind: &'static str) -> Result<Value, DecodeError> {
    match value {
        JispValue::Null => Ok(Value::Null),
        JispValue::Bool(value) => Ok(Value::Bool(*value)),
        JispValue::Int(value) => Ok(Value::Number(Number::from(*value))),
        JispValue::Float(value) => {
            Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| DecodeError::Invalid {
                    kind,
                    message: "request must not contain NaN or infinity".to_owned(),
                })
        }
        JispValue::Str(value) => Ok(Value::String(value.to_string())),
        JispValue::List(values) => values
            .iter()
            .map(|value| json_value(value, kind))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        JispValue::Obj(fields) => fields
            .iter()
            .map(|(key, value)| json_value(value, kind).map(|value| (key.clone(), value)))
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
        JispValue::Variant { tag, fields } => Ok(serde_json::json!({
            "tag": tag,
            "fields": fields
                .iter()
                .map(|value| json_value(value, kind))
                .collect::<Result<Vec<_>, _>>()?,
        })),
        JispValue::BigInt(_) => invalid(kind, "request must not contain bigint"),
        JispValue::Builtin(_)
        | JispValue::Closure(_)
        | JispValue::Constructor(_)
        | JispValue::Uninitialized(_) => invalid(kind, "request must be portable data"),
    }
}

fn invalid<T>(kind: &'static str, message: impl Into<String>) -> Result<T, DecodeError> {
    Err(DecodeError::Invalid {
        kind,
        message: message.into(),
    })
}

#[cfg(test)]
#[path = "effects_decode_test.rs"]
mod effects_decode_test;

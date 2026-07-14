//! A deliberately small, in-memory native-widget adapter prototype.
//!
//! This module consumes the renderer-neutral UI value produced by JUIR. It is
//! not a GUI toolkit and must not become one: platform adapters can translate
//! [`NativeWidget`] into their own retained widget objects. Its purpose is to
//! prove that the portable tree maps to semantic widgets rather than DOM APIs,
//! and that an unsupported host feature is a deterministic diagnostic.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use jisp_eval::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeWidgetKind {
    Container,
    Text,
    Button,
    TextInput,
    List,
    ListItem,
    Form,
    Label,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeWidget {
    pub kind: NativeWidgetKind,
    pub key: Option<NativeScalar>,
    pub attributes: BTreeMap<String, NativeScalar>,
    pub properties: BTreeMap<String, NativeScalar>,
    /// Structured utility tokens are retained for a platform style-token map;
    /// they are not interpreted as browser CSS.
    pub style_tokens: BTreeSet<String>,
    pub events: BTreeSet<String>,
    pub children: Vec<NativeWidget>,
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NativeScalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Clone, Debug, Default)]
pub struct NativeRegistry {
    widgets: BTreeMap<String, NativeWidgetSpec>,
}

#[derive(Clone, Debug)]
pub struct NativeWidgetSpec {
    pub kind: NativeWidgetKind,
    pub attributes: BTreeSet<String>,
    pub properties: BTreeSet<String>,
    pub events: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeError {
    InvalidNode(String),
    UnsupportedElement { tag: String },
    UnsupportedAttribute { tag: String, name: String },
    UnsupportedProperty { tag: String, name: String },
    UnsupportedEvent { tag: String, name: String },
    InvalidScalar { field: String },
}

impl std::fmt::Display for NativeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNode(message) => formatter.write_str(message),
            Self::UnsupportedElement { tag } => {
                write!(
                    formatter,
                    "native host does not support Jisp element `{tag}`"
                )
            }
            Self::UnsupportedAttribute { tag, name } => {
                write!(
                    formatter,
                    "native `{tag}` does not support attribute `{name}`"
                )
            }
            Self::UnsupportedProperty { tag, name } => {
                write!(
                    formatter,
                    "native `{tag}` does not support property `{name}`"
                )
            }
            Self::UnsupportedEvent { tag, name } => {
                write!(formatter, "native `{tag}` does not support event `{name}`")
            }
            Self::InvalidScalar { field } => {
                write!(formatter, "native UI `{field}` must be a scalar")
            }
        }
    }
}

impl std::error::Error for NativeError {}

impl NativeRegistry {
    /// A small semantic widget set suitable for an initial desktop/mobile
    /// adapter. Deliberately unsupported web-only elements (for example `img`,
    /// `a`, and `select`) fail explicitly instead of receiving a fake DOM map.
    pub fn portable_baseline() -> Self {
        let mut registry = Self::default();
        for tag in [
            "article", "aside", "div", "footer", "header", "main", "nav", "section",
        ] {
            registry.insert(tag, NativeWidgetKind::Container);
        }
        for tag in ["h1", "h2", "h3", "p", "span", "strong"] {
            registry.insert(tag, NativeWidgetKind::Text);
        }
        registry.insert("button", NativeWidgetKind::Button);
        registry.insert("input", NativeWidgetKind::TextInput);
        registry.insert("textarea", NativeWidgetKind::TextInput);
        registry.insert("ul", NativeWidgetKind::List);
        registry.insert("ol", NativeWidgetKind::List);
        registry.insert("li", NativeWidgetKind::ListItem);
        registry.insert("form", NativeWidgetKind::Form);
        registry.insert("label", NativeWidgetKind::Label);
        registry
    }

    pub fn insert(&mut self, tag: impl Into<String>, kind: NativeWidgetKind) {
        self.widgets.insert(
            tag.into(),
            NativeWidgetSpec {
                kind,
                attributes: BTreeSet::from([
                    "aria-label".to_owned(),
                    "placeholder".to_owned(),
                    "role".to_owned(),
                    "type".to_owned(),
                ]),
                properties: BTreeSet::from([
                    "checked".to_owned(),
                    "disabled".to_owned(),
                    "hidden".to_owned(),
                    "value".to_owned(),
                ]),
                events: BTreeSet::from([
                    "blur".to_owned(),
                    "change".to_owned(),
                    "click".to_owned(),
                    "focus".to_owned(),
                    "input".to_owned(),
                    "keydown".to_owned(),
                    "keyup".to_owned(),
                    "submit".to_owned(),
                ]),
            },
        );
    }

    fn widget(&self, tag: &str) -> Result<&NativeWidgetSpec, NativeError> {
        self.widgets
            .get(tag)
            .ok_or_else(|| NativeError::UnsupportedElement {
                tag: tag.to_owned(),
            })
    }
}

/// Materialize a portable JUIR structural value as semantic native widgets.
pub fn render(value: &Value, registry: &NativeRegistry) -> Result<NativeWidget, NativeError> {
    render_node(value, registry)
}

fn render_node(value: &Value, registry: &NativeRegistry) -> Result<NativeWidget, NativeError> {
    let fields = object(value, "native UI node must be an object")?;
    let tag = string(fields.get("tag"), "native UI node is missing string `tag`")?;
    if tag == "text" {
        return Ok(NativeWidget {
            kind: NativeWidgetKind::Text,
            key: None,
            attributes: BTreeMap::new(),
            properties: BTreeMap::new(),
            style_tokens: BTreeSet::new(),
            events: BTreeSet::new(),
            children: vec![],
            text: Some(scalar(fields.get("value"), "text value")?.display()),
        });
    }
    let spec = registry.widget(tag)?;
    let attrs = native_map(fields.get("attrs"), tag, &spec.attributes, true)?;
    let props = native_map(fields.get("props"), tag, &spec.properties, false)?;
    let events = event_names(fields.get("events"), tag, &spec.events)?;
    let classes = style_tokens(fields.get("classes"))?;
    let children = match fields.get("children") {
        None => vec![],
        Some(Value::List(children)) => children
            .iter()
            .map(|child| render_node(child, registry))
            .collect::<Result<_, _>>()?,
        Some(_) => {
            return Err(NativeError::InvalidNode(
                "native UI children must be a list".to_owned(),
            ))
        }
    };
    Ok(NativeWidget {
        kind: spec.kind.clone(),
        key: fields
            .get("key")
            .map(|value| scalar(Some(value), "key"))
            .transpose()?,
        attributes: attrs,
        properties: props,
        style_tokens: classes,
        events,
        children,
        text: None,
    })
}

fn native_map(
    value: Option<&Value>,
    tag: &str,
    allowed: &BTreeSet<String>,
    attribute: bool,
) -> Result<BTreeMap<String, NativeScalar>, NativeError> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let fields = object(value, "native UI metadata must be an object")?;
    fields
        .iter()
        .map(|(name, value)| {
            if !allowed.contains(name) {
                return Err(if attribute {
                    NativeError::UnsupportedAttribute {
                        tag: tag.to_owned(),
                        name: name.clone(),
                    }
                } else {
                    NativeError::UnsupportedProperty {
                        tag: tag.to_owned(),
                        name: name.clone(),
                    }
                });
            }
            Ok((name.clone(), scalar(Some(value), name)?))
        })
        .collect()
}

fn event_names(
    value: Option<&Value>,
    tag: &str,
    allowed: &BTreeSet<String>,
) -> Result<BTreeSet<String>, NativeError> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let fields = object(value, "native UI events must be an object")?;
    fields
        .keys()
        .map(|name| {
            if allowed.contains(name) {
                Ok(name.clone())
            } else {
                Err(NativeError::UnsupportedEvent {
                    tag: tag.to_owned(),
                    name: name.clone(),
                })
            }
        })
        .collect()
}

fn style_tokens(value: Option<&Value>) -> Result<BTreeSet<String>, NativeError> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let fields = object(value, "native UI classes must be an object")?;
    fields
        .iter()
        .filter_map(|(name, enabled)| enabled.truthy().then_some(Ok(name.clone())))
        .collect()
}

fn object<'a>(value: &'a Value, message: &str) -> Result<&'a IndexMap<String, Value>, NativeError> {
    match value {
        Value::Obj(fields) => Ok(fields),
        _ => Err(NativeError::InvalidNode(message.to_owned())),
    }
}

fn string<'a>(value: Option<&'a Value>, message: &str) -> Result<&'a str, NativeError> {
    match value {
        Some(Value::Str(value)) => Ok(value),
        _ => Err(NativeError::InvalidNode(message.to_owned())),
    }
}

fn scalar(value: Option<&Value>, field: &str) -> Result<NativeScalar, NativeError> {
    match value {
        Some(Value::Null) | None => Ok(NativeScalar::Null),
        Some(Value::Bool(value)) => Ok(NativeScalar::Bool(*value)),
        Some(Value::Int(value)) => Ok(NativeScalar::Int(*value)),
        Some(Value::Float(value)) => Ok(NativeScalar::Float(*value)),
        Some(Value::Str(value)) => Ok(NativeScalar::Str(value.to_string())),
        Some(Value::BigInt(value)) => Ok(NativeScalar::Str(value.to_string())),
        Some(_) => Err(NativeError::InvalidScalar {
            field: field.to_owned(),
        }),
    }
}

impl NativeScalar {
    fn display(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Str(value) => value.clone(),
        }
    }
}

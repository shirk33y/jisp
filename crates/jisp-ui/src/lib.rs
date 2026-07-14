//! Typed, renderer-neutral intermediate representation for Jisp UI components.
//!
//! JUIR is deliberately an internal compiler artifact. It retains the source
//! expression for each dynamic slot, while separating static template shape
//! from host execution. Browser and native executors will consume this contract;
//! the current structural-tree renderer remains the semantic reference.

use std::collections::BTreeMap;

use jisp_core::Span;
use jisp_ir::{Definition, Expr, ExprKind, Literal};
use jisp_types::{Type, TypedModule};

#[derive(Clone, Debug)]
pub struct Program {
    pub components: BTreeMap<String, Component>,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub name: String,
    pub params: Vec<String>,
    pub rest: Option<String>,
    pub root: Node,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Node {
    Text(Slot),
    Element(Element),
    If {
        condition: Expr,
        then_branch: Box<Node>,
        else_branch: Box<Node>,
        span: Span,
    },
    Each {
        binding: String,
        collection: Expr,
        body: Box<Node>,
        span: Span,
    },
    ComponentCall {
        name: String,
        arguments: Vec<Expr>,
        span: Span,
    },
    Dynamic {
        expression: Expr,
        ty: Type,
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct Element {
    pub tag: String,
    pub attrs: BTreeMap<String, Slot>,
    pub props: BTreeMap<String, Slot>,
    pub classes: BTreeMap<String, Slot>,
    pub events: BTreeMap<String, Event>,
    pub key: Option<Slot>,
    pub children: Vec<Node>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Slot {
    Static(Scalar),
    Dynamic {
        expression: Expr,
        ty: Type,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

#[derive(Clone, Debug)]
pub struct Event {
    pub handler: Expr,
    pub policy: EventPolicy,
    pub span: Span,
}

/// Host-local event policy. The source syntax currently emits the default;
/// explicit modifiers will lower here before JUIR event execution is enabled.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventPolicy {
    pub prevent_default: bool,
    pub stop_propagation: bool,
    pub capture: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    InvalidUiNode { span: Span, message: String },
    UnknownComponent { name: String },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUiNode { message, .. } => formatter.write_str(message),
            Self::UnknownComponent { name } => {
                write!(formatter, "JUIR component `{name}` does not exist")
            }
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile(module: &TypedModule) -> Result<Program, CompileError> {
    let component_names = module
        .module
        .definitions
        .iter()
        .filter(|definition| component_parts(definition).is_some())
        .map(|definition| definition.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let compiler = Compiler {
        expression_types: &module.expression_types,
        component_names: &component_names,
    };
    let mut components = BTreeMap::new();
    for definition in &module.module.definitions {
        let Some((params, rest, root)) = component_parts(definition) else {
            continue;
        };
        components.insert(
            definition.name.clone(),
            Component {
                name: definition.name.clone(),
                params: params.to_vec(),
                rest: rest.clone(),
                root: compiler.node(root)?,
                span: definition.span,
            },
        );
    }
    Ok(Program { components })
}

pub fn render_static_html(program: &Program, component: &str) -> Result<String, CompileError> {
    let component =
        program
            .components
            .get(component)
            .ok_or_else(|| CompileError::UnknownComponent {
                name: component.to_owned(),
            })?;
    if !component.params.is_empty() || component.rest.is_some() {
        return Err(dynamic_error(
            component.span,
            "static rendering needs a component without parameters",
        ));
    }
    let mut output = String::new();
    render_static_node(&component.root, &mut output)?;
    Ok(output)
}

struct Compiler<'a> {
    expression_types: &'a std::collections::HashMap<Span, Type>,
    component_names: &'a std::collections::BTreeSet<String>,
}

impl Compiler<'_> {
    fn node(&self, expr: &Expr) -> Result<Node, CompileError> {
        if let ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &expr.kind
        {
            return Ok(Node::If {
                condition: (**condition).clone(),
                then_branch: Box::new(self.node(then_branch)?),
                else_branch: Box::new(self.node(else_branch)?),
                span: expr.span,
            });
        }
        if let Some((binding, collection, body)) = each_parts(expr) {
            return Ok(Node::Each {
                binding: binding.to_owned(),
                collection: collection.clone(),
                body: Box::new(self.node(body)?),
                span: expr.span,
            });
        }
        if let Some((name, arguments)) = component_call(expr, self.component_names) {
            return Ok(Node::ComponentCall {
                name: name.to_owned(),
                arguments: arguments.to_vec(),
                span: expr.span,
            });
        }
        let Some(object) = ui_node_object(expr) else {
            return Ok(self.dynamic(expr));
        };
        self.object_node(object, expr.span)
    }

    fn object_node(&self, fields: &[(Expr, Expr)], span: Span) -> Result<Node, CompileError> {
        let fields = object_fields(fields)?;
        let tag = static_string(required_field(&fields, "tag", span)?)?;
        if tag == "text" {
            return Ok(Node::Text(
                self.slot(required_field(&fields, "value", span)?)?,
            ));
        }
        Ok(Node::Element(Element {
            tag,
            attrs: self.slots(fields.get("attrs"))?,
            props: self.slots(fields.get("props"))?,
            classes: self.slots(fields.get("classes"))?,
            events: self.events(fields.get("events"))?,
            key: fields.get("key").map(|expr| self.slot(expr)).transpose()?,
            children: fields
                .get("children")
                .map(|children| self.children(children))
                .transpose()?
                .unwrap_or_default(),
            span,
        }))
    }

    fn children(&self, expr: &Expr) -> Result<Vec<Node>, CompileError> {
        match &expr.kind {
            ExprKind::List(children) => children.iter().map(|child| self.node(child)).collect(),
            ExprKind::Call { callee, arguments } if is_name(callee, "list.cat") => arguments
                .iter()
                .map(|argument| self.children(argument))
                .collect::<Result<Vec<_>, _>>()
                .map(|groups| groups.into_iter().flatten().collect()),
            _ => Ok(vec![self.node(expr)?]),
        }
    }

    fn slots(&self, expr: Option<&&Expr>) -> Result<BTreeMap<String, Slot>, CompileError> {
        let Some(expr) = expr else {
            return Ok(BTreeMap::new());
        };
        let ExprKind::Object(fields) = &expr.kind else {
            return Err(invalid(expr.span, "JUIR metadata must be an object"));
        };
        object_fields(fields)?
            .into_iter()
            .map(|(name, value)| Ok((name, self.slot(value)?)))
            .collect()
    }

    fn events(&self, expr: Option<&&Expr>) -> Result<BTreeMap<String, Event>, CompileError> {
        let Some(expr) = expr else {
            return Ok(BTreeMap::new());
        };
        let ExprKind::Object(fields) = &expr.kind else {
            return Err(invalid(expr.span, "JUIR events must be an object"));
        };
        object_fields(fields)?
            .into_iter()
            .map(|(name, handler)| {
                Ok((
                    name,
                    Event {
                        span: handler.span,
                        handler: handler.clone(),
                        policy: EventPolicy::default(),
                    },
                ))
            })
            .collect()
    }

    fn slot(&self, expr: &Expr) -> Result<Slot, CompileError> {
        match &expr.kind {
            ExprKind::Literal(Literal::Null) => Ok(Slot::Static(Scalar::Null)),
            ExprKind::Literal(Literal::Bool(value)) => Ok(Slot::Static(Scalar::Bool(*value))),
            ExprKind::Literal(Literal::Int(value)) => Ok(Slot::Static(Scalar::Int(*value))),
            ExprKind::Literal(Literal::Float(value)) => Ok(Slot::Static(Scalar::Float(*value))),
            ExprKind::Literal(Literal::String(value)) => {
                Ok(Slot::Static(Scalar::Str(value.clone())))
            }
            _ => Ok(self.dynamic(expr).into_slot()),
        }
    }

    fn dynamic(&self, expr: &Expr) -> Node {
        Node::Dynamic {
            expression: expr.clone(),
            ty: self
                .expression_types
                .get(&expr.span)
                .cloned()
                .unwrap_or(Type::Never),
            span: expr.span,
        }
    }
}

impl Node {
    fn into_slot(self) -> Slot {
        let Self::Dynamic {
            expression,
            ty,
            span,
        } = self
        else {
            unreachable!("only compiler dynamic nodes become slots")
        };
        Slot::Dynamic {
            expression,
            ty,
            span,
        }
    }
}

fn component_parts(definition: &Definition) -> Option<(&[String], &Option<String>, &Expr)> {
    let ExprKind::Lambda { params, rest, body } = &definition.value.kind else {
        return None;
    };
    ui_node_object(body).map(|_| (params.as_slice(), rest, body.as_ref()))
}

fn ui_node_object(expr: &Expr) -> Option<&[(Expr, Expr)]> {
    let ExprKind::Call { callee, arguments } = &expr.kind else {
        return None;
    };
    if !is_name(callee, "ui.node") || arguments.len() != 1 {
        return None;
    }
    let ExprKind::Object(fields) = &arguments[0].kind else {
        return None;
    };
    Some(fields)
}

fn each_parts(expr: &Expr) -> Option<(&str, &Expr, &Expr)> {
    let ExprKind::Call { callee, arguments } = &expr.kind else {
        return None;
    };
    if !is_name(callee, "list.map") || arguments.len() != 2 {
        return None;
    }
    let ExprKind::Lambda { params, rest, body } = &arguments[0].kind else {
        return None;
    };
    if params.len() != 1 || rest.is_some() {
        return None;
    }
    Some((&params[0], &arguments[1], body))
}

fn component_call<'a>(
    expr: &'a Expr,
    component_names: &std::collections::BTreeSet<String>,
) -> Option<(&'a str, &'a [Expr])> {
    let ExprKind::Call { callee, arguments } = &expr.kind else {
        return None;
    };
    let ExprKind::Name(name) = &callee.kind else {
        return None;
    };
    component_names.contains(name).then_some((name, arguments))
}

fn object_fields<'a>(
    fields: &'a [(Expr, Expr)],
) -> Result<BTreeMap<String, &'a Expr>, CompileError> {
    fields
        .iter()
        .map(|(key, value)| Ok((static_string(key)?, value)))
        .collect()
}

fn required_field<'a>(
    fields: &'a BTreeMap<String, &'a Expr>,
    name: &str,
    span: Span,
) -> Result<&'a Expr, CompileError> {
    fields
        .get(name)
        .copied()
        .ok_or_else(|| invalid(span, format!("JUIR node is missing `{name}`")))
}

fn static_string(expr: &Expr) -> Result<String, CompileError> {
    let ExprKind::Literal(Literal::String(value)) = &expr.kind else {
        return Err(invalid(
            expr.span,
            "JUIR object keys must be static strings",
        ));
    };
    Ok(value.clone())
}

fn is_name(expr: &Expr, name: &str) -> bool {
    matches!(&expr.kind, ExprKind::Name(value) if value == name)
}

fn invalid(span: Span, message: impl Into<String>) -> CompileError {
    CompileError::InvalidUiNode {
        span,
        message: message.into(),
    }
}

fn dynamic_error(span: Span, message: impl Into<String>) -> CompileError {
    invalid(span, message)
}

fn render_static_node(node: &Node, output: &mut String) -> Result<(), CompileError> {
    match node {
        Node::Text(slot) => output.push_str(&escape_text(&static_slot(slot)?)),
        Node::Element(element) => {
            output.push('<');
            output.push_str(&element.tag);
            let mut classes = Vec::new();
            for (name, slot) in &element.classes {
                if matches!(static_slot(slot)?, Scalar::Bool(true)) {
                    classes.push(name.as_str());
                }
            }
            if !classes.is_empty() {
                output.push_str(" class=\"");
                output.push_str(&escape_attr(&classes.join(" ")));
                output.push('"');
            }
            for (name, slot) in element.attrs.iter().chain(element.props.iter()) {
                render_attribute(name, static_slot(slot)?, output);
            }
            output.push('>');
            for child in &element.children {
                render_static_node(child, output)?;
            }
            output.push_str("</");
            output.push_str(&element.tag);
            output.push('>');
        }
        Node::If { span, .. }
        | Node::Each { span, .. }
        | Node::ComponentCall { span, .. }
        | Node::Dynamic { span, .. } => return Err(dynamic_error(*span, "JUIR node is dynamic")),
    }
    Ok(())
}

fn render_attribute(name: &str, value: Scalar, output: &mut String) {
    match value {
        Scalar::Null | Scalar::Bool(false) => {}
        Scalar::Bool(true) => {
            output.push(' ');
            output.push_str(name);
        }
        Scalar::Int(value) => render_string_attribute(name, &value.to_string(), output),
        Scalar::Float(value) => render_string_attribute(name, &value.to_string(), output),
        Scalar::Str(value) => render_string_attribute(name, &value, output),
    }
}

fn render_string_attribute(name: &str, value: &str, output: &mut String) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_attr(value));
    output.push('"');
}

fn static_slot(slot: &Slot) -> Result<Scalar, CompileError> {
    match slot {
        Slot::Static(value) => Ok(value.clone()),
        Slot::Dynamic { span, .. } => Err(dynamic_error(*span, "JUIR slot is dynamic")),
    }
}

fn escape_text(value: &Scalar) -> String {
    scalar_text(value)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn scalar_text(value: &Scalar) -> String {
    match value {
        Scalar::Null => "null".to_owned(),
        Scalar::Bool(value) => value.to_string(),
        Scalar::Int(value) => value.to_string(),
        Scalar::Float(value) => value.to_string(),
        Scalar::Str(value) => value.clone(),
    }
}

#[cfg(test)]
mod lib_test;

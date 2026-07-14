//! Typed, renderer-neutral intermediate representation for Jisp UI components.
//!
//! JUIR is deliberately an internal compiler artifact. It retains the source
//! expression for each dynamic slot, while separating static template shape
//! from host execution. Browser and native executors will consume this contract;
//! the current structural-tree renderer remains the semantic reference.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use jisp_core::Span;
use jisp_eval::{Env, Evaluator, RuntimeError, Value};
use jisp_ir::{Definition, Expr, ExprKind, Literal, StringPart};
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
        dependencies: Vec<Dependency>,
        then_branch: Box<Node>,
        else_branch: Box<Node>,
        span: Span,
    },
    Each {
        binding: String,
        collection: Expr,
        dependencies: Vec<Dependency>,
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
        dependencies: Vec<Dependency>,
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct Element {
    pub tag: String,
    pub attrs: IndexMap<String, Slot>,
    pub props: IndexMap<String, Slot>,
    pub classes: IndexMap<String, Slot>,
    pub events: IndexMap<String, Event>,
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
        dependencies: Vec<Dependency>,
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

/// Conservative static dependencies of a dynamic JUIR expression.
///
/// `Path` is a static field-read chain rooted at a component parameter. Every
/// expression that cannot be proven to be only such reads carries `Unknown`;
/// hosts must then re-evaluate it rather than risk a stale value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dependency {
    Path { root: String, fields: Vec<String> },
    Unknown,
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

#[derive(Debug)]
pub enum ExecuteError {
    UnknownComponent {
        name: String,
    },
    InvalidArguments {
        component: String,
        expected: String,
        actual: usize,
    },
    InvalidValue {
        span: Span,
        message: String,
    },
    Runtime(RuntimeError),
}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownComponent { name } => {
                write!(formatter, "JUIR component `{name}` does not exist")
            }
            Self::InvalidArguments {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "JUIR component `{component}` expects {expected} argument(s), got {actual}"
            ),
            Self::InvalidValue { message, .. } => formatter.write_str(message),
            Self::Runtime(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExecuteError {}

impl From<RuntimeError> for ExecuteError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(error)
    }
}

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
                root: compiler.node(root, params)?,
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
    render_static_node(program, &component.root, &mut output)?;
    Ok(output)
}

/// Execute a compiled UI component to the existing renderer-neutral Jisp UI
/// value. Dynamic expressions run in the supplied Jisp evaluator and lexical
/// module environment; a host never needs to interpret a Jisp expression.
pub fn execute(
    program: &Program,
    evaluator: &mut Evaluator,
    module_env: &Env,
    component: &str,
    arguments: &[Value],
) -> Result<Value, ExecuteError> {
    Executor {
        program,
        evaluator,
        module_env,
    }
    .component(component, arguments)
}

struct Compiler<'a> {
    expression_types: &'a std::collections::HashMap<Span, Type>,
    component_names: &'a std::collections::BTreeSet<String>,
}

impl Compiler<'_> {
    fn node(&self, expr: &Expr, parameters: &[String]) -> Result<Node, CompileError> {
        if let ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &expr.kind
        {
            return Ok(Node::If {
                condition: (**condition).clone(),
                dependencies: expression_dependencies(condition, parameters),
                then_branch: Box::new(self.node(then_branch, parameters)?),
                else_branch: Box::new(self.node(else_branch, parameters)?),
                span: expr.span,
            });
        }
        if let Some((binding, collection, body)) = each_parts(expr) {
            return Ok(Node::Each {
                binding: binding.to_owned(),
                collection: collection.clone(),
                dependencies: expression_dependencies(collection, parameters),
                body: Box::new(self.node(body, parameters)?),
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
            return Ok(self.dynamic(expr, parameters));
        };
        self.object_node(object, expr.span, parameters)
    }

    fn object_node(
        &self,
        fields: &[(Expr, Expr)],
        span: Span,
        parameters: &[String],
    ) -> Result<Node, CompileError> {
        let fields = object_fields(fields)?;
        let tag = static_string(required_field(&fields, "tag", span)?)?;
        if tag == "text" {
            return Ok(Node::Text(
                self.slot(required_field(&fields, "value", span)?, parameters)?,
            ));
        }
        Ok(Node::Element(Element {
            tag,
            attrs: self.slots(fields.get("attrs"), parameters)?,
            props: self.slots(fields.get("props"), parameters)?,
            classes: self.slots(fields.get("classes"), parameters)?,
            events: self.events(fields.get("events"))?,
            key: fields
                .get("key")
                .map(|expr| self.slot(expr, parameters))
                .transpose()?,
            children: fields
                .get("children")
                .map(|children| self.children(children, parameters))
                .transpose()?
                .unwrap_or_default(),
            span,
        }))
    }

    fn children(&self, expr: &Expr, parameters: &[String]) -> Result<Vec<Node>, CompileError> {
        match &expr.kind {
            ExprKind::List(children) => children
                .iter()
                .map(|child| self.node(child, parameters))
                .collect(),
            ExprKind::Call { callee, arguments } if is_name(callee, "list.cat") => arguments
                .iter()
                .map(|argument| self.children(argument, parameters))
                .collect::<Result<Vec<_>, _>>()
                .map(|groups| groups.into_iter().flatten().collect()),
            _ => Ok(vec![self.node(expr, parameters)?]),
        }
    }

    fn slots(
        &self,
        expr: Option<&&Expr>,
        parameters: &[String],
    ) -> Result<IndexMap<String, Slot>, CompileError> {
        let Some(expr) = expr else {
            return Ok(IndexMap::new());
        };
        let ExprKind::Object(fields) = &expr.kind else {
            return Err(invalid(expr.span, "JUIR metadata must be an object"));
        };
        fields
            .iter()
            .map(|(name, value)| Ok((static_string(name)?, self.slot(value, parameters)?)))
            .collect()
    }

    fn events(&self, expr: Option<&&Expr>) -> Result<IndexMap<String, Event>, CompileError> {
        let Some(expr) = expr else {
            return Ok(IndexMap::new());
        };
        let ExprKind::Object(fields) = &expr.kind else {
            return Err(invalid(expr.span, "JUIR events must be an object"));
        };
        fields
            .iter()
            .map(|(name, handler)| {
                Ok((
                    static_string(name)?,
                    Event {
                        span: handler.span,
                        handler: handler.clone(),
                        policy: EventPolicy::default(),
                    },
                ))
            })
            .collect()
    }

    fn slot(&self, expr: &Expr, parameters: &[String]) -> Result<Slot, CompileError> {
        match &expr.kind {
            ExprKind::Literal(Literal::Null) => Ok(Slot::Static(Scalar::Null)),
            ExprKind::Literal(Literal::Bool(value)) => Ok(Slot::Static(Scalar::Bool(*value))),
            ExprKind::Literal(Literal::Int(value)) => Ok(Slot::Static(Scalar::Int(*value))),
            ExprKind::Literal(Literal::Float(value)) => Ok(Slot::Static(Scalar::Float(*value))),
            ExprKind::Literal(Literal::String(value)) => {
                Ok(Slot::Static(Scalar::Str(value.clone())))
            }
            _ => Ok(self.dynamic(expr, parameters).into_slot()),
        }
    }

    fn dynamic(&self, expr: &Expr, parameters: &[String]) -> Node {
        Node::Dynamic {
            expression: expr.clone(),
            ty: self
                .expression_types
                .get(&expr.span)
                .cloned()
                .unwrap_or(Type::Never),
            dependencies: expression_dependencies(expr, parameters),
            span: expr.span,
        }
    }
}

struct Executor<'a> {
    program: &'a Program,
    evaluator: &'a mut Evaluator,
    module_env: &'a Env,
}

impl Executor<'_> {
    fn component(&mut self, name: &str, arguments: &[Value]) -> Result<Value, ExecuteError> {
        let component = self.program.components.get(name).cloned().ok_or_else(|| {
            ExecuteError::UnknownComponent {
                name: name.to_owned(),
            }
        })?;
        let expected = component.params.len();
        if arguments.len() < expected || (component.rest.is_none() && arguments.len() != expected) {
            return Err(ExecuteError::InvalidArguments {
                component: name.to_owned(),
                expected: format!(
                    "{}{}",
                    expected,
                    if component.rest.is_some() { "+" } else { "" }
                ),
                actual: arguments.len(),
            });
        }

        let env = self.module_env.child();
        for (parameter, argument) in component.params.iter().zip(arguments) {
            env.define(parameter.clone(), argument.clone());
        }
        if let Some(rest) = &component.rest {
            env.define(rest.clone(), Value::List(arguments[expected..].to_vec()));
        }
        self.node(&component.root, &env)
    }

    fn node(&mut self, node: &Node, env: &Env) -> Result<Value, ExecuteError> {
        match node {
            Node::Text(slot) => Ok(Value::Obj(IndexMap::from([
                ("tag".to_owned(), Value::string("text")),
                ("value".to_owned(), self.slot(slot, env)?),
            ]))),
            Node::Element(element) => self.element(element, env),
            Node::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if self.evaluator.eval_in(condition, env)?.truthy() {
                    self.node(then_branch, env)
                } else {
                    self.node(else_branch, env)
                }
            }
            Node::Each {
                binding,
                collection,
                body,
                span,
                ..
            } => {
                let values = self.evaluator.eval_in(collection, env)?;
                let Value::List(values) = values else {
                    return Err(invalid_value(
                        *span,
                        format!(
                            "JUIR each collection must be a list, got {}",
                            values.type_name()
                        ),
                    ));
                };
                values
                    .into_iter()
                    .map(|value| {
                        let item_env = env.child();
                        item_env.define(binding.clone(), value);
                        self.node(body, &item_env)
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::List)
            }
            Node::ComponentCall {
                name, arguments, ..
            } => {
                let values = arguments
                    .iter()
                    .map(|argument| self.evaluator.eval_in(argument, env).map_err(Into::into))
                    .collect::<Result<Vec<_>, ExecuteError>>()?;
                self.component(name, &values)
            }
            Node::Dynamic { expression, .. } => {
                self.evaluator.eval_in(expression, env).map_err(Into::into)
            }
        }
    }

    fn element(&mut self, element: &Element, env: &Env) -> Result<Value, ExecuteError> {
        let mut fields = IndexMap::new();
        fields.insert("tag".to_owned(), Value::string(element.tag.clone()));
        self.insert_slots(&mut fields, "attrs", &element.attrs, env)?;
        self.insert_slots(&mut fields, "props", &element.props, env)?;
        self.insert_slots(&mut fields, "classes", &element.classes, env)?;
        if !element.events.is_empty() {
            let mut events = IndexMap::new();
            for (name, event) in &element.events {
                events.insert(name.clone(), self.evaluator.eval_in(&event.handler, env)?);
            }
            fields.insert("events".to_owned(), Value::Obj(events));
        }
        if let Some(key) = &element.key {
            fields.insert("key".to_owned(), self.slot(key, env)?);
        }
        if !element.children.is_empty() {
            fields.insert(
                "children".to_owned(),
                element
                    .children
                    .iter()
                    .map(|child| self.node(child, env))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::List)?,
            );
        }
        Ok(Value::Obj(fields))
    }

    fn insert_slots(
        &mut self,
        fields: &mut IndexMap<String, Value>,
        name: &str,
        slots: &IndexMap<String, Slot>,
        env: &Env,
    ) -> Result<(), ExecuteError> {
        if slots.is_empty() {
            return Ok(());
        }
        let mut values = IndexMap::new();
        for (name, slot) in slots {
            values.insert(name.clone(), self.slot(slot, env)?);
        }
        fields.insert(name.to_owned(), Value::Obj(values));
        Ok(())
    }

    fn slot(&mut self, slot: &Slot, env: &Env) -> Result<Value, ExecuteError> {
        match slot {
            Slot::Static(value) => Ok(scalar_value(value)),
            Slot::Dynamic { expression, .. } => {
                self.evaluator.eval_in(expression, env).map_err(Into::into)
            }
        }
    }
}

fn scalar_value(value: &Scalar) -> Value {
    match value {
        Scalar::Null => Value::Null,
        Scalar::Bool(value) => Value::Bool(*value),
        Scalar::Int(value) => Value::Int(*value),
        Scalar::Float(value) => Value::Float(*value),
        Scalar::Str(value) => Value::string(value.clone()),
    }
}

fn invalid_value(span: Span, message: impl Into<String>) -> ExecuteError {
    ExecuteError::InvalidValue {
        span,
        message: message.into(),
    }
}

impl Node {
    fn into_slot(self) -> Slot {
        let Self::Dynamic {
            expression,
            ty,
            dependencies,
            span,
        } = self
        else {
            unreachable!("only compiler dynamic nodes become slots")
        };
        Slot::Dynamic {
            expression,
            ty,
            dependencies,
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

fn expression_dependencies(expr: &Expr, parameters: &[String]) -> Vec<Dependency> {
    let mut paths = std::collections::BTreeSet::new();
    let mut unknown = false;
    collect_dependencies(expr, parameters, &mut paths, &mut unknown);
    let mut dependencies = paths
        .into_iter()
        .map(|(root, fields)| Dependency::Path { root, fields })
        .collect::<Vec<_>>();
    if unknown {
        dependencies.push(Dependency::Unknown);
    }
    dependencies
}

fn collect_dependencies(
    expr: &Expr,
    parameters: &[String],
    paths: &mut std::collections::BTreeSet<(String, Vec<String>)>,
    unknown: &mut bool,
) {
    match &expr.kind {
        ExprKind::Literal(_) => {}
        ExprKind::Name(name) => {
            if parameters.contains(name) {
                paths.insert((name.clone(), vec![]));
            }
        }
        ExprKind::Field { .. } => match dependency_path(expr, parameters) {
            Some((root, fields)) => {
                paths.insert((root, fields));
            }
            None => *unknown = true,
        },
        ExprKind::Lambda { .. } => *unknown = true,
        ExprKind::Let { bindings, body } => {
            for (_, value) in bindings {
                collect_dependencies(value, parameters, paths, unknown);
            }
            // The body can refer to local bindings, for which a static component
            // parameter path is not generally recoverable.
            *unknown = true;
            collect_dependencies(body, parameters, paths, unknown);
        }
        ExprKind::Do(expressions)
        | ExprKind::And(expressions)
        | ExprKind::Or(expressions)
        | ExprKind::List(expressions) => {
            for expression in expressions {
                collect_dependencies(expression, parameters, paths, unknown);
            }
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_dependencies(condition, parameters, paths, unknown);
            collect_dependencies(then_branch, parameters, paths, unknown);
            collect_dependencies(else_branch, parameters, paths, unknown);
        }
        ExprKind::Not(expression) => collect_dependencies(expression, parameters, paths, unknown),
        ExprKind::Call { callee, arguments } => {
            collect_dependencies(callee, parameters, paths, unknown);
            for argument in arguments {
                collect_dependencies(argument, parameters, paths, unknown);
            }
        }
        ExprKind::Object(fields) => {
            for (key, value) in fields {
                collect_dependencies(key, parameters, paths, unknown);
                collect_dependencies(value, parameters, paths, unknown);
            }
        }
        ExprKind::StringTemplate { parts, .. } => {
            for part in parts {
                if let StringPart::Expr(expression) | StringPart::Splice(expression) = part {
                    collect_dependencies(expression, parameters, paths, unknown);
                }
            }
        }
        ExprKind::Case {
            subject, branches, ..
        } => {
            collect_dependencies(subject, parameters, paths, unknown);
            // Pattern bindings can feed guards and bodies, so retain the safe
            // fallback while still recording any direct parameter reads.
            *unknown = true;
            for branch in branches {
                if let Some(guard) = &branch.guard {
                    collect_dependencies(guard, parameters, paths, unknown);
                }
                collect_dependencies(&branch.body, parameters, paths, unknown);
            }
        }
    }
}

fn dependency_path(expr: &Expr, parameters: &[String]) -> Option<(String, Vec<String>)> {
    match &expr.kind {
        ExprKind::Name(name) if parameters.contains(name) => Some((name.clone(), vec![])),
        ExprKind::Field { object, key } => {
            let (root, mut fields) = dependency_path(object, parameters)?;
            let ExprKind::Literal(Literal::String(key)) = &key.kind else {
                return None;
            };
            fields.push(key.clone());
            Some((root, fields))
        }
        _ => None,
    }
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

fn render_static_node(
    program: &Program,
    node: &Node,
    output: &mut String,
) -> Result<(), CompileError> {
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
                render_static_node(program, child, output)?;
            }
            output.push_str("</");
            output.push_str(&element.tag);
            output.push('>');
        }
        Node::ComponentCall {
            name,
            arguments,
            span,
        } => {
            if !arguments.is_empty() {
                return Err(dynamic_error(*span, "JUIR node is dynamic"));
            }
            let component = program
                .components
                .get(name)
                .ok_or_else(|| CompileError::UnknownComponent { name: name.clone() })?;
            if !component.params.is_empty() || component.rest.is_some() {
                return Err(dynamic_error(*span, "JUIR node is dynamic"));
            }
            render_static_node(program, &component.root, output)?;
        }
        Node::If { span, .. } | Node::Each { span, .. } | Node::Dynamic { span, .. } => {
            return Err(dynamic_error(*span, "JUIR node is dynamic"))
        }
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

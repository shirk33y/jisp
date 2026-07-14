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
use serde_json::{json, Map as JsonMap, Value as JsonValue};

pub mod effects;
pub mod native;
pub mod testing;

#[derive(Clone, Debug)]
pub struct Program {
    pub components: BTreeMap<String, Component>,
    /// Stable source locations for compiled templates, slots, events, and
    /// block expressions. Hosts may use these for diagnostics and developer
    /// tooling, but they never participate in execution semantics.
    pub source_map: Vec<SourceMapEntry>,
}

/// One source location in the compiled JUIR plan.
///
/// `component` and `path` identify the generated plan location without
/// leaking a browser- or toolkit-specific host identity. `path` always starts
/// at `root`; child indices and metadata names are appended with `.`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMapEntry {
    pub component: String,
    pub path: String,
    pub kind: SourceMapKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMapKind {
    Element,
    Text,
    If,
    Each,
    ComponentCall,
    Dynamic,
    Slot,
    Event,
    Condition,
    Collection,
    Argument,
}

impl SourceMapKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Element => "element",
            Self::Text => "text",
            Self::If => "if",
            Self::Each => "each",
            Self::ComponentCall => "component-call",
            Self::Dynamic => "dynamic",
            Self::Slot => "slot",
            Self::Event => "event",
            Self::Condition => "condition",
            Self::Collection => "collection",
            Self::Argument => "argument",
        }
    }
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
    Text(Text),
    Element(Box<Element>),
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
        dependencies: Vec<Dependency>,
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
pub struct Text {
    pub value: Slot,
    pub span: Span,
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

/// A conservative set of state paths changed by one reducer turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeSet {
    pub paths: std::collections::BTreeSet<DependencyPath>,
    pub unknown: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyPath {
    pub root: String,
    pub fields: Vec<String>,
}

impl ChangeSet {
    /// Returns whether reevaluating an expression with `dependencies` is
    /// necessary. `Unknown` on either side deliberately disables skipping.
    pub fn affects(&self, dependencies: &[Dependency]) -> bool {
        self.unknown
            || dependencies.iter().any(|dependency| match dependency {
                Dependency::Unknown => true,
                Dependency::Path { root, fields } => {
                    let paths = self
                        .paths
                        .iter()
                        .filter(|change| change.root == *root)
                        .collect::<Vec<_>>();
                    paths.is_empty()
                        || paths.into_iter().any(|change| {
                            is_path_prefix(&change.fields, fields)
                                || is_path_prefix(fields, &change.fields)
                        })
                }
            })
    }
}

/// Compare two immutable Jisp values under one root parameter. List changes
/// intentionally collapse to their containing field path for now; that causes
/// a keyed `for` block to rerun conservatively until per-item invalidation is
/// implemented.
pub fn changed_paths(root: impl Into<String>, before: &Value, after: &Value) -> ChangeSet {
    let root = root.into();
    let mut changes = ChangeSet::default();
    collect_changed_paths(before, after, &root, &mut Vec::new(), &mut changes);
    changes
}

#[derive(Clone, Debug)]
pub struct Event {
    pub handler: Expr,
    pub dependencies: Vec<Dependency>,
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
    let source_map = components
        .iter()
        .flat_map(|(name, component)| source_map_entries(name, &component.root))
        .collect();
    Ok(Program {
        components,
        source_map,
    })
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

/// Serialize the static shape of one compiled component for a host's initial
/// DOM/native mount. Dynamic nodes deliberately become explicit holes: their
/// current renderer-neutral value still comes from [`execute`], so this plan
/// never requires a host to evaluate Jisp expressions.
///
/// The result is a compact implementation protocol. It does not replace the
/// structural tree oracle or define user-facing source syntax.
pub fn mount_plan(program: &Program, component: &str) -> Result<JsonValue, CompileError> {
    let component =
        program
            .components
            .get(component)
            .ok_or_else(|| CompileError::UnknownComponent {
                name: component.to_owned(),
            })?;
    Ok(json!({
        "protocol": "jisp-ui-mount-plan/1",
        "component": component.name,
        "root": mount_plan_node(&component.root),
    }))
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
    Ok(execute_incremental(
        program,
        evaluator,
        module_env,
        component,
        arguments,
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )?
    .value)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionStats {
    pub evaluated_slots: usize,
    pub reused_slots: usize,
    pub reused_subtrees: usize,
    pub reused_blocks: usize,
    pub reused_items: usize,
    pub reused_components: usize,
}

pub struct Execution {
    pub value: Value,
    pub stats: ExecutionStats,
    each_cache: BTreeMap<String, Vec<CachedEachItem>>,
}

#[derive(Clone)]
struct CachedEachItem {
    value: Value,
    rendered: Value,
}

/// Execute JUIR while conservatively reusing unaffected scalar slots and
/// collection blocks from a previous structural value.
pub fn execute_incremental(
    program: &Program,
    evaluator: &mut Evaluator,
    module_env: &Env,
    component: &str,
    arguments: &[Value],
    previous: Option<&Value>,
    changes: &ChangeSet,
) -> Result<Execution, ExecuteError> {
    execute_incremental_inner(
        program,
        evaluator,
        module_env,
        component,
        arguments,
        IncrementalInputs {
            previous,
            previous_each_cache: None,
            changes,
        },
    )
}

/// Execute JUIR with an execution cache from the prior turn. Besides scalar
/// slots and whole blocks, this can retain rendered rows whose immutable item
/// value is unchanged even when a keyed collection itself changed.
pub fn execute_incremental_cached(
    program: &Program,
    evaluator: &mut Evaluator,
    module_env: &Env,
    component: &str,
    arguments: &[Value],
    previous: Option<&Execution>,
    changes: &ChangeSet,
) -> Result<Execution, ExecuteError> {
    execute_incremental_inner(
        program,
        evaluator,
        module_env,
        component,
        arguments,
        IncrementalInputs {
            previous: previous.map(|execution| &execution.value),
            previous_each_cache: previous.map(|execution| &execution.each_cache),
            changes,
        },
    )
}

struct IncrementalInputs<'a> {
    previous: Option<&'a Value>,
    previous_each_cache: Option<&'a BTreeMap<String, Vec<CachedEachItem>>>,
    changes: &'a ChangeSet,
}

fn execute_incremental_inner(
    program: &Program,
    evaluator: &mut Evaluator,
    module_env: &Env,
    component: &str,
    arguments: &[Value],
    inputs: IncrementalInputs<'_>,
) -> Result<Execution, ExecuteError> {
    let mut executor = Executor {
        program,
        evaluator,
        module_env,
        changes: inputs.changes,
        stats: ExecutionStats::default(),
        previous_each_cache: inputs.previous_each_cache,
        each_cache: BTreeMap::new(),
    };
    let value = executor.component(component, arguments, inputs.previous, "root")?;
    Ok(Execution {
        value,
        stats: executor.stats,
        each_cache: executor.each_cache,
    })
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
            let mut body_parameters = parameters.to_vec();
            body_parameters.push(binding.to_owned());
            return Ok(Node::Each {
                binding: binding.to_owned(),
                collection: collection.clone(),
                dependencies: expression_dependencies(collection, parameters),
                body: Box::new(self.node(body, &body_parameters)?),
                span: expr.span,
            });
        }
        if let Some((name, arguments)) = component_call(expr, self.component_names) {
            return Ok(Node::ComponentCall {
                name: name.to_owned(),
                arguments: arguments.to_vec(),
                dependencies: arguments
                    .iter()
                    .flat_map(|argument| expression_dependencies(argument, parameters))
                    .collect(),
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
            return Ok(Node::Text(Text {
                value: self.slot(required_field(&fields, "value", span)?, parameters)?,
                span,
            }));
        }
        Ok(Node::Element(Box::new(Element {
            tag,
            attrs: self.slots(fields.get("attrs"), parameters)?,
            props: self.slots(fields.get("props"), parameters)?,
            classes: self.slots(fields.get("classes"), parameters)?,
            events: self.events(fields.get("events"), parameters)?,
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
        })))
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

    fn events(
        &self,
        expr: Option<&&Expr>,
        parameters: &[String],
    ) -> Result<IndexMap<String, Event>, CompileError> {
        let Some(expr) = expr else {
            return Ok(IndexMap::new());
        };
        let ExprKind::Object(fields) = &expr.kind else {
            return Err(invalid(expr.span, "JUIR events must be an object"));
        };
        fields
            .iter()
            .map(|(name, descriptor)| {
                let (handler, policy) = event_descriptor(descriptor)?;
                Ok((
                    static_string(name)?,
                    Event {
                        span: descriptor.span,
                        dependencies: expression_dependencies(&handler, parameters),
                        handler,
                        policy,
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

fn event_descriptor(expr: &Expr) -> Result<(Expr, EventPolicy), CompileError> {
    let ExprKind::Object(fields) = &expr.kind else {
        return Ok((expr.clone(), EventPolicy::default()));
    };
    let fields = object_fields(fields)?;
    let Some(handler) = fields.get("handler") else {
        return Ok((expr.clone(), EventPolicy::default()));
    };
    let policy = match fields.get("policy") {
        None => EventPolicy::default(),
        Some(policy) => event_policy(policy)?,
    };
    Ok(((*handler).clone(), policy))
}

fn event_policy(expr: &Expr) -> Result<EventPolicy, CompileError> {
    let ExprKind::Object(fields) = &expr.kind else {
        return Err(invalid(expr.span, "JUIR event policy must be an object"));
    };
    let mut policy = EventPolicy::default();
    for (name, value) in fields {
        let name = static_string(name)?;
        let ExprKind::Literal(Literal::Bool(enabled)) = value.kind else {
            return Err(invalid(value.span, "JUIR event policy flags must be bools"));
        };
        match name.as_str() {
            "prevent-default" => policy.prevent_default = enabled,
            "stop-propagation" => policy.stop_propagation = enabled,
            "capture" => policy.capture = enabled,
            _ => {
                return Err(invalid(
                    value.span,
                    format!("unknown JUIR event policy `{name}`"),
                ))
            }
        }
    }
    Ok(policy)
}

struct Executor<'a> {
    program: &'a Program,
    evaluator: &'a mut Evaluator,
    module_env: &'a Env,
    changes: &'a ChangeSet,
    stats: ExecutionStats,
    previous_each_cache: Option<&'a BTreeMap<String, Vec<CachedEachItem>>>,
    each_cache: BTreeMap<String, Vec<CachedEachItem>>,
}

impl Executor<'_> {
    fn each_body_uses_only_stable_inputs(&self, body: &Node, binding: &str) -> bool {
        if self.changes.unknown {
            return false;
        }
        node_dependencies(body)
            .into_iter()
            .all(|dependency| match &dependency {
                Dependency::Unknown => false,
                Dependency::Path { root, .. } if root == binding => true,
                Dependency::Path { .. } => !self.changes.affects(&[dependency]),
            })
    }

    fn component(
        &mut self,
        name: &str,
        arguments: &[Value],
        previous: Option<&Value>,
        path: &str,
    ) -> Result<Value, ExecuteError> {
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
        self.node(&component.root, &env, previous, path)
    }

    fn node(
        &mut self,
        node: &Node,
        env: &Env,
        previous: Option<&Value>,
        path: &str,
    ) -> Result<Value, ExecuteError> {
        match node {
            Node::Text(text) => Ok(Value::Obj(IndexMap::from([
                ("tag".to_owned(), Value::string("text")),
                (
                    "value".to_owned(),
                    self.slot(&text.value, env, previous.and_then(text_value))?,
                ),
            ]))),
            Node::Element(element) => {
                if !self.changes.affects(&node_dependencies(node)) {
                    if let Some(previous) = previous {
                        self.stats.reused_subtrees += 1;
                        return Ok(previous.clone());
                    }
                }
                self.element(element, env, previous, path)
            }
            Node::If {
                condition,
                then_branch,
                else_branch,
                dependencies,
                ..
            } => {
                // A previous structural value belongs to the branch selected
                // on the preceding turn. Reusing it after the condition
                // changed would let a static new branch inherit stale output.
                let previous = if self.changes.affects(dependencies) {
                    None
                } else {
                    previous
                };
                if self.evaluator.eval_in(condition, env)?.truthy() {
                    self.node(then_branch, env, previous, path)
                } else {
                    self.node(else_branch, env, previous, path)
                }
            }
            Node::Each {
                binding,
                collection,
                dependencies,
                body,
                span,
                ..
            } => {
                if !self.changes.affects(dependencies) {
                    if let Some(Value::List(previous)) = previous {
                        self.stats.reused_blocks += 1;
                        if let Some(cache) = self
                            .previous_each_cache
                            .and_then(|cache| cache.get(path))
                            .cloned()
                        {
                            self.each_cache.insert(path.to_owned(), cache);
                        }
                        return Ok(Value::List(previous.clone()));
                    }
                }
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
                let mut reusable = if self.each_body_uses_only_stable_inputs(body, binding) {
                    self.previous_each_cache
                        .and_then(|cache| cache.get(path))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                let mut cached = Vec::with_capacity(values.len());
                let mut rendered = Vec::with_capacity(values.len());
                for (index, value) in values.into_iter().enumerate() {
                    if let Some(previous_index) = reusable
                        .iter()
                        .position(|previous| values_equal(&previous.value, &value))
                    {
                        let previous = reusable.remove(previous_index);
                        self.stats.reused_items += 1;
                        rendered.push(previous.rendered.clone());
                        cached.push(CachedEachItem {
                            value,
                            rendered: previous.rendered,
                        });
                        continue;
                    }
                    let item_env = env.child();
                    item_env.define(binding.clone(), value.clone());
                    let rendered_item =
                        self.node(body, &item_env, None, &format!("{path}.item.{index}"))?;
                    cached.push(CachedEachItem {
                        value,
                        rendered: rendered_item.clone(),
                    });
                    rendered.push(rendered_item);
                }
                self.each_cache.insert(path.to_owned(), cached);
                Ok(Value::List(rendered))
            }
            Node::ComponentCall {
                name,
                arguments,
                dependencies,
                ..
            } => {
                if !self.changes.affects(dependencies) {
                    if let Some(previous) = previous {
                        self.stats.reused_components += 1;
                        return Ok(previous.clone());
                    }
                }
                let values = arguments
                    .iter()
                    .map(|argument| self.evaluator.eval_in(argument, env).map_err(Into::into))
                    .collect::<Result<Vec<_>, ExecuteError>>()?;
                self.component(name, &values, previous, path)
            }
            Node::Dynamic {
                expression,
                dependencies,
                ..
            } => {
                if !self.changes.affects(dependencies) {
                    if let Some(previous) = previous {
                        self.stats.reused_slots += 1;
                        return Ok(previous.clone());
                    }
                }
                self.stats.evaluated_slots += 1;
                self.evaluator.eval_in(expression, env).map_err(Into::into)
            }
        }
    }

    fn element(
        &mut self,
        element: &Element,
        env: &Env,
        previous: Option<&Value>,
        path: &str,
    ) -> Result<Value, ExecuteError> {
        let previous_fields = previous.and_then(object_fields_value);
        let mut fields = IndexMap::new();
        fields.insert("tag".to_owned(), Value::string(element.tag.clone()));
        self.insert_slots(&mut fields, "attrs", &element.attrs, env, previous_fields)?;
        self.insert_slots(&mut fields, "props", &element.props, env, previous_fields)?;
        self.insert_slots(
            &mut fields,
            "classes",
            &element.classes,
            env,
            previous_fields,
        )?;
        if !element.events.is_empty() {
            let mut events = IndexMap::new();
            for (name, event) in &element.events {
                events.insert(
                    name.clone(),
                    Value::Obj(IndexMap::from([
                        (
                            "handler".to_owned(),
                            self.evaluator.eval_in(&event.handler, env)?,
                        ),
                        ("policy".to_owned(), event_policy_value(&event.policy)),
                    ])),
                );
            }
            fields.insert("events".to_owned(), Value::Obj(events));
        }
        if let Some(key) = &element.key {
            fields.insert(
                "key".to_owned(),
                self.slot(
                    key,
                    env,
                    previous_fields.and_then(|fields| fields.get("key")),
                )?,
            );
        }
        if !element.children.is_empty() {
            let previous_children = previous_fields
                .and_then(|fields| fields.get("children"))
                .and_then(list_value);
            let mut previous_offset = 0;
            let mut previous_alignment_safe = true;
            let mut children = vec![];
            for (index, child) in element.children.iter().enumerate() {
                let child_path = format!("{path}.child.{index}");
                let previous_child = if previous_alignment_safe {
                    match child {
                        Node::Each { .. } => self
                            .previous_each_cache
                            .and_then(|cache| cache.get(&child_path))
                            .map(|items| {
                                Value::List(
                                    items.iter().map(|item| item.rendered.clone()).collect(),
                                )
                            }),
                        _ => previous_children
                            .and_then(|children| children.get(previous_offset))
                            .cloned(),
                    }
                } else {
                    None
                };
                let previous_width = match &previous_child {
                    Some(Value::List(children)) if matches!(child, Node::Each { .. }) => {
                        children.len()
                    }
                    Some(_) => 1,
                    None => 0,
                };
                let rendered = self.node(child, env, previous_child.as_ref(), &child_path)?;
                append_flattened_child(rendered, &mut children);
                previous_offset += previous_width;
                if matches!(child, Node::Dynamic { .. } | Node::If { .. }) {
                    // A dynamic expression can contribute any number of
                    // flattened children. Do not risk reusing a later sibling
                    // at a shifted position; conservatively re-evaluate it.
                    previous_alignment_safe = false;
                }
            }
            fields.insert("children".to_owned(), Value::List(children));
        }
        Ok(Value::Obj(fields))
    }

    fn insert_slots(
        &mut self,
        fields: &mut IndexMap<String, Value>,
        name: &str,
        slots: &IndexMap<String, Slot>,
        env: &Env,
        previous: Option<&IndexMap<String, Value>>,
    ) -> Result<(), ExecuteError> {
        if slots.is_empty() {
            return Ok(());
        }
        let mut values = IndexMap::new();
        for (name, slot) in slots {
            values.insert(
                name.clone(),
                self.slot(slot, env, previous.and_then(|values| values.get(name)))?,
            );
        }
        fields.insert(name.to_owned(), Value::Obj(values));
        Ok(())
    }

    fn slot(
        &mut self,
        slot: &Slot,
        env: &Env,
        previous: Option<&Value>,
    ) -> Result<Value, ExecuteError> {
        match slot {
            Slot::Static(value) => Ok(scalar_value(value)),
            Slot::Dynamic {
                expression,
                dependencies,
                ..
            } => {
                if !self.changes.affects(dependencies) {
                    if let Some(previous) = previous {
                        self.stats.reused_slots += 1;
                        return Ok(previous.clone());
                    }
                }
                self.stats.evaluated_slots += 1;
                self.evaluator.eval_in(expression, env).map_err(Into::into)
            }
        }
    }
}

/// UI child lists are transparent containers in the source semantics: `for`,
/// `list.cat`, and a dynamic list contribute their items, not an extra node.
/// The executor must preserve that invariant before any browser/native/test
/// host sees the structural tree.
fn append_flattened_child(value: Value, output: &mut Vec<Value>) {
    match value {
        Value::List(values) => {
            for value in values {
                append_flattened_child(value, output);
            }
        }
        value => output.push(value),
    }
}

fn event_policy_value(policy: &EventPolicy) -> Value {
    Value::Obj(IndexMap::from([
        (
            "prevent-default".to_owned(),
            Value::Bool(policy.prevent_default),
        ),
        (
            "stop-propagation".to_owned(),
            Value::Bool(policy.stop_propagation),
        ),
        ("capture".to_owned(), Value::Bool(policy.capture)),
    ]))
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

fn object_fields_value(value: &Value) -> Option<&IndexMap<String, Value>> {
    let Value::Obj(fields) = value else {
        return None;
    };
    Some(fields)
}

fn list_value(value: &Value) -> Option<&[Value]> {
    let Value::List(values) = value else {
        return None;
    };
    Some(values)
}

fn text_value(value: &Value) -> Option<&Value> {
    object_fields_value(value)?.get("value")
}

fn is_path_prefix(prefix: &[String], path: &[String]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

fn collect_changed_paths(
    before: &Value,
    after: &Value,
    root: &str,
    fields: &mut Vec<String>,
    changes: &mut ChangeSet,
) {
    match (before, after) {
        (Value::Obj(before), Value::Obj(after)) => {
            let keys = before
                .keys()
                .chain(after.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                let Some(before) = before.get(key) else {
                    fields.push(key.clone());
                    record_change(root, fields, changes);
                    fields.pop();
                    continue;
                };
                let Some(after) = after.get(key) else {
                    fields.push(key.clone());
                    record_change(root, fields, changes);
                    fields.pop();
                    continue;
                };
                fields.push(key.clone());
                collect_changed_paths(before, after, root, fields, changes);
                fields.pop();
            }
        }
        (Value::List(before), Value::List(after)) => {
            if before.len() != after.len()
                || before
                    .iter()
                    .zip(after)
                    .any(|(before, after)| !values_equal(before, after))
            {
                record_change(root, fields, changes);
            }
        }
        _ if values_equal(before, after) => {}
        _ => record_change(root, fields, changes),
    }
}

fn record_change(root: &str, fields: &[String], changes: &mut ChangeSet) {
    changes.paths.insert(DependencyPath {
        root: root.to_owned(),
        fields: fields.to_vec(),
    });
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::BigInt(left), Value::BigInt(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left.to_bits() == right.to_bits(),
        (Value::Str(left), Value::Str(right)) => left == right,
        (Value::List(left), Value::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Obj(left), Value::Obj(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| values_equal(left, right))
                })
        }
        (
            Value::Variant {
                tag: left_tag,
                fields: left_fields,
            },
            Value::Variant {
                tag: right_tag,
                fields: right_fields,
            },
        ) => {
            left_tag == right_tag
                && left_fields.len() == right_fields.len()
                && left_fields
                    .iter()
                    .zip(right_fields)
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Builtin(left), Value::Builtin(right)) => left.name == right.name,
        (Value::Constructor(left), Value::Constructor(right)) => {
            left.name == right.name && left.arity == right.arity
        }
        (Value::Uninitialized(left), Value::Uninitialized(right)) => left == right,
        // Closures could observe lexical state that is not structurally exposed.
        // Treat them as changed, which is safe for an incremental scheduler.
        (Value::Closure(_), Value::Closure(_)) => false,
        _ => false,
    }
}

fn node_dependencies(node: &Node) -> Vec<Dependency> {
    let mut dependencies = vec![];
    append_node_dependencies(node, &mut dependencies);
    dependencies
}

fn source_map_entries(component: &str, root: &Node) -> Vec<SourceMapEntry> {
    let mut entries = vec![];
    append_source_map_entries(component, root, "root", &mut entries);
    entries
}

fn append_source_map_entries(
    component: &str,
    node: &Node,
    path: &str,
    output: &mut Vec<SourceMapEntry>,
) {
    match node {
        Node::Text(text) => {
            source_map_entry(output, component, path, SourceMapKind::Text, text.span);
            append_slot_source_map(output, component, &format!("{path}.value"), &text.value);
        }
        Node::Element(element) => {
            source_map_entry(
                output,
                component,
                path,
                SourceMapKind::Element,
                element.span,
            );
            append_slots_source_map(output, component, path, "attrs", &element.attrs);
            append_slots_source_map(output, component, path, "props", &element.props);
            append_slots_source_map(output, component, path, "classes", &element.classes);
            if let Some(key) = &element.key {
                append_slot_source_map(output, component, &format!("{path}.key"), key);
            }
            for (name, event) in &element.events {
                source_map_entry(
                    output,
                    component,
                    &format!("{path}.events.{name}"),
                    SourceMapKind::Event,
                    event.span,
                );
            }
            for (index, child) in element.children.iter().enumerate() {
                append_source_map_entries(
                    component,
                    child,
                    &format!("{path}.children.{index}"),
                    output,
                );
            }
        }
        Node::If {
            condition,
            then_branch,
            else_branch,
            span,
            ..
        } => {
            source_map_entry(output, component, path, SourceMapKind::If, *span);
            source_map_entry(
                output,
                component,
                &format!("{path}.condition"),
                SourceMapKind::Condition,
                condition.span,
            );
            append_source_map_entries(component, then_branch, &format!("{path}.then"), output);
            append_source_map_entries(component, else_branch, &format!("{path}.else"), output);
        }
        Node::Each {
            collection,
            body,
            span,
            ..
        } => {
            source_map_entry(output, component, path, SourceMapKind::Each, *span);
            source_map_entry(
                output,
                component,
                &format!("{path}.collection"),
                SourceMapKind::Collection,
                collection.span,
            );
            append_source_map_entries(component, body, &format!("{path}.body"), output);
        }
        Node::ComponentCall {
            arguments, span, ..
        } => {
            source_map_entry(output, component, path, SourceMapKind::ComponentCall, *span);
            for (index, argument) in arguments.iter().enumerate() {
                source_map_entry(
                    output,
                    component,
                    &format!("{path}.arguments.{index}"),
                    SourceMapKind::Argument,
                    argument.span,
                );
            }
        }
        Node::Dynamic { span, .. } => {
            source_map_entry(output, component, path, SourceMapKind::Dynamic, *span);
        }
    }
}

fn append_slots_source_map(
    output: &mut Vec<SourceMapEntry>,
    component: &str,
    path: &str,
    category: &str,
    slots: &IndexMap<String, Slot>,
) {
    for (name, slot) in slots {
        append_slot_source_map(
            output,
            component,
            &format!("{path}.{category}.{name}"),
            slot,
        );
    }
}

fn append_slot_source_map(
    output: &mut Vec<SourceMapEntry>,
    component: &str,
    path: &str,
    slot: &Slot,
) {
    if let Slot::Dynamic { span, .. } = slot {
        source_map_entry(output, component, path, SourceMapKind::Slot, *span);
    }
}

fn source_map_entry(
    output: &mut Vec<SourceMapEntry>,
    component: &str,
    path: &str,
    kind: SourceMapKind,
    span: Span,
) {
    output.push(SourceMapEntry {
        component: component.to_owned(),
        path: path.to_owned(),
        kind,
        span,
    });
}

fn mount_plan_node(node: &Node) -> JsonValue {
    match node {
        Node::Text(text) => json!({
            "kind": "text",
            "staticValue": static_slot_json(&text.value),
        }),
        Node::Element(element) => json!({
            "kind": "element",
            "tag": element.tag,
            "staticAttrs": static_slots_json(&element.attrs),
            "staticProps": static_slots_json(&element.props),
            "staticClasses": static_classes_json(&element.classes),
            "events": element.events.keys().collect::<Vec<_>>(),
            "children": element.children.iter().map(mount_plan_node).collect::<Vec<_>>(),
        }),
        // These plan nodes have no invariant initial host shape. Their current
        // structural value is mounted as a single dynamic region by the host.
        Node::If { .. } => json!({ "kind": "dynamic", "block": "if" }),
        Node::Each { .. } => json!({ "kind": "dynamic", "block": "each" }),
        Node::ComponentCall { .. } => json!({ "kind": "dynamic", "block": "component" }),
        Node::Dynamic { .. } => json!({ "kind": "dynamic", "block": "value" }),
    }
}

fn static_slots_json(slots: &IndexMap<String, Slot>) -> JsonValue {
    slots
        .iter()
        .filter_map(|(name, slot)| static_slot_json(slot).map(|value| (name.clone(), value)))
        .collect::<JsonMap<_, _>>()
        .into()
}

fn static_classes_json(classes: &IndexMap<String, Slot>) -> JsonValue {
    JsonValue::Array(
        classes
            .iter()
            .filter_map(|(name, slot)| {
                matches!(slot, Slot::Static(Scalar::Bool(true)))
                    .then_some(JsonValue::String(name.clone()))
            })
            .collect(),
    )
}

fn static_slot_json(slot: &Slot) -> Option<JsonValue> {
    let Slot::Static(value) = slot else {
        return None;
    };
    Some(match value {
        Scalar::Null => JsonValue::Null,
        Scalar::Bool(value) => JsonValue::Bool(*value),
        Scalar::Int(value) => JsonValue::Number((*value).into()),
        Scalar::Float(value) => serde_json::Number::from_f64(*value)
            .map(JsonValue::Number)
            .expect("Jisp source float literals are finite"),
        Scalar::Str(value) => JsonValue::String(value.clone()),
    })
}

fn append_node_dependencies(node: &Node, output: &mut Vec<Dependency>) {
    match node {
        Node::Text(text) => append_slot_dependencies(&text.value, output),
        Node::Element(element) => {
            for slot in element
                .attrs
                .values()
                .chain(element.props.values())
                .chain(element.classes.values())
            {
                append_slot_dependencies(slot, output);
            }
            if let Some(key) = &element.key {
                append_slot_dependencies(key, output);
            }
            for event in element.events.values() {
                output.extend(event.dependencies.iter().cloned());
            }
            for child in &element.children {
                append_node_dependencies(child, output);
            }
        }
        Node::If {
            dependencies,
            then_branch,
            else_branch,
            ..
        } => {
            output.extend(dependencies.iter().cloned());
            append_node_dependencies(then_branch, output);
            append_node_dependencies(else_branch, output);
        }
        Node::Each {
            dependencies, body, ..
        } => {
            output.extend(dependencies.iter().cloned());
            append_node_dependencies(body, output);
        }
        Node::ComponentCall { dependencies, .. } | Node::Dynamic { dependencies, .. } => {
            output.extend(dependencies.iter().cloned());
        }
    }
}

fn append_slot_dependencies(slot: &Slot, output: &mut Vec<Dependency>) {
    if let Slot::Dynamic { dependencies, .. } = slot {
        output.extend(dependencies.iter().cloned());
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
    is_ui_root(body).then_some((params.as_slice(), rest, body.as_ref()))
}

fn is_ui_root(expr: &Expr) -> bool {
    if ui_node_object(expr).is_some() {
        return true;
    }
    match &expr.kind {
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => is_ui_root(then_branch) && is_ui_root(else_branch),
        _ => false,
    }
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
            } else {
                // A local binding, module definition, or prelude function can
                // still influence the value. Until name resolution is carried
                // into JUIR, retain correctness by making this slot dynamic.
                *unknown = true;
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

fn object_fields(fields: &[(Expr, Expr)]) -> Result<BTreeMap<String, &Expr>, CompileError> {
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
        Node::Text(text) => output.push_str(&escape_text(&static_slot(&text.value)?)),
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
            ..
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
#[cfg(test)]
mod testing_test;

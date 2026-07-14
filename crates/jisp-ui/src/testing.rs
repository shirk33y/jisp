//! Portable, renderer-level tests for update-driven Jisp UI applications.
//!
//! These fixture-only forms are deliberately not Jisp runtime builtins. A
//! portable test executes the same reducer and JUIR program as a host, but it
//! has no DOM dependency:
//!
//! ```text
//! (ui.test "counter increments"
//!   (assert (= "<button>0</button>" (ui.test.html)))
//!   (dispatch (Increment))
//!   (assert (= 1 (ui.test.state))))
//! ```
//!
//! Every assertion first checks that the reference component value and the
//! compiled JUIR execution are structurally equal. The narrow accessor set is
//! intentional: it gives tests stable, portable observations now and leaves a
//! future browser E2E host free to replay the same action trace. Tests can
//! also inspect the commands and subscriptions declared by the most recent
//! dispatch without executing a host capability.

use jisp_core::{Node, Span};
use jisp_eval::{normalize_update_result, Evaluator, Value};
use jisp_ir::{Definition, Lowerer, Module};
use jisp_types::Inferencer;
use serde_json::{Map as JsonMap, Number, Value as JsonValue};

use crate::effects::{
    Capability, Delivery, FakeHost, HostError, HostErrorCode, Owner, ResourceKind,
};
use crate::{compile, execute};

#[path = "testing_parse.rs"]
mod testing_parse;

use testing_parse::{actual_accessor, parse_supports};

#[derive(Clone, Debug)]
pub struct UiTestSuite {
    pub module_nodes: Vec<Node>,
    pub tests: Vec<UiTest>,
}

#[derive(Clone, Debug)]
pub struct UiTest {
    pub name: String,
    capabilities: Vec<Capability>,
    steps: Vec<UiTestStep>,
}

#[derive(Clone, Debug)]
enum UiTestStep {
    Dispatch {
        action: Node,
        span: Span,
    },
    Assert {
        expected: Node,
        actual: UiTestActual,
        span: Span,
    },
    Supports {
        capability: Capability,
    },
    Deliver {
        kind: ResourceKind,
        id: String,
        result: Node,
        span: Span,
    },
    DeliverError {
        kind: ResourceKind,
        id: String,
        error: HostError,
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiTestActual {
    State,
    Html,
    Tree,
    Commands,
    Subscriptions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTestOutcome {
    pub name: String,
    pub assertions: usize,
    pub failure: Option<String>,
}

impl UiTestOutcome {
    pub fn passed(&self) -> bool {
        self.failure.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTestError(pub String);

impl std::fmt::Display for UiTestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UiTestError {}

/// Separate fixture-only `ui.test` forms before lowering a regular module.
pub fn split_ui_tests(nodes: Vec<Node>) -> Result<UiTestSuite, UiTestError> {
    let mut module_nodes = vec![];
    let mut tests = vec![];
    for node in nodes {
        if node
            .as_form()
            .and_then(|items| items.first())
            .and_then(Node::as_symbol)
            == Some("ui.test")
        {
            tests.push(parse_test(node)?);
        } else {
            module_nodes.push(node);
        }
    }
    Ok(UiTestSuite {
        module_nodes,
        tests,
    })
}

/// Run all extracted UI scenarios against the reference evaluator and JUIR.
pub fn run_ui_tests(suite: UiTestSuite) -> Result<Vec<UiTestOutcome>, UiTestError> {
    if suite.tests.is_empty() {
        return Ok(vec![]);
    }

    let mut module = Lowerer
        .lower_module(&suite.module_nodes)
        .map_err(lower_error)?;
    let prepared = prepare_steps(&mut module, &suite.tests)?;
    let typed = Inferencer::with_prelude()
        .infer_typed_module(module)
        .map_err(|error| UiTestError(format!("ui.test type check failed: {error}")))?;
    let app = typed.module.ui_app.clone().ok_or_else(|| {
        UiTestError("ui.test requires one `(ui.app init update app)` declaration".to_owned())
    })?;
    let program = compile(&typed)
        .map_err(|error| UiTestError(format!("ui.test JUIR compile failed: {error}")))?;
    if !program.components.contains_key(&app.app) {
        return Err(UiTestError(format!(
            "ui.test view `{}` must be a Jisp UI component",
            app.app
        )));
    }

    let mut evaluator = Evaluator::new();
    let loaded = evaluator
        .load_module(&typed.module)
        .map_err(|error| UiTestError(format!("ui.test evaluation failed: {error}")))?;
    let init = lookup(&loaded.env, &app.init, "init")?;
    let update = lookup(&loaded.env, &app.update, "update")?;
    let view = lookup(&loaded.env, &app.app, "view")?;
    let html = evaluator
        .root_env()
        .lookup("ui.html")
        .map_err(|error| UiTestError(format!("ui.test cannot access ui.html: {error}")))?;

    let outcomes = prepared
        .iter()
        .map(|test| {
            run_one(
                test,
                &mut evaluator,
                &loaded.env,
                &program,
                &app.app,
                &init,
                &update,
                &view,
                &html,
                app.span,
            )
        })
        .collect();
    Ok(outcomes)
}

fn parse_test(node: Node) -> Result<UiTest, UiTestError> {
    let items = node
        .as_form()
        .ok_or_else(|| UiTestError("ui.test must be a form".to_owned()))?;
    if items.len() < 3 {
        return Err(UiTestError(
            "ui.test expects a name and at least one step".to_owned(),
        ));
    }
    let name = items[1]
        .as_string()
        .ok_or_else(|| UiTestError("ui.test name must be a string".to_owned()))?
        .to_owned();
    let mut capabilities = vec![];
    let mut steps = vec![];
    let mut test_started = false;
    for step in &items[2..] {
        match parse_step(&name, step)? {
            UiTestStep::Supports { capability } if !test_started => capabilities.push(capability),
            UiTestStep::Supports { .. } => {
                return Err(UiTestError(format!(
                    "{name}: supports must appear before dispatch, deliver, or assert"
                )))
            }
            step => {
                test_started = true;
                steps.push(step);
            }
        }
    }
    Ok(UiTest {
        name,
        capabilities,
        steps,
    })
}

fn parse_step(name: &str, node: &Node) -> Result<UiTestStep, UiTestError> {
    let items = node
        .as_form()
        .ok_or_else(|| UiTestError(format!("{name}: ui.test step must be a form")))?;
    match items.first().and_then(Node::as_symbol) {
        Some("dispatch") => {
            if items.len() != 2 {
                return Err(UiTestError(format!("{name}: dispatch expects one action")));
            }
            Ok(UiTestStep::Dispatch {
                action: items[1].clone(),
                span: node.span,
            })
        }
        Some("supports") => parse_supports(name, items),
        Some("deliver") => parse_delivery(name, node, items),
        Some("deliver-error") => parse_error_delivery(name, node, items),
        Some("assert") => parse_assert(name, node, items),
        Some(other) => Err(UiTestError(format!(
            "{name}: unsupported ui.test step `{other}`; use supports, dispatch, deliver, deliver-error, or assert"
        ))),
        None => Err(UiTestError(format!(
            "{name}: ui.test step must start with a symbol"
        ))),
    }
}

fn parse_delivery(name: &str, node: &Node, items: &[Node]) -> Result<UiTestStep, UiTestError> {
    if items.len() != 4 {
        return Err(UiTestError(format!(
            "{name}: deliver expects a resource kind, id, and portable result"
        )));
    }
    Ok(UiTestStep::Deliver {
        kind: resource_kind(name, &items[1])?,
        id: resource_id(name, &items[2])?,
        result: items[3].clone(),
        span: node.span,
    })
}

fn parse_error_delivery(
    name: &str,
    node: &Node,
    items: &[Node],
) -> Result<UiTestStep, UiTestError> {
    if items.len() != 5 {
        return Err(UiTestError(format!(
            "{name}: deliver-error expects a resource kind, id, error code, and message"
        )));
    }
    let code = items[3]
        .as_string()
        .ok_or_else(|| UiTestError(format!("{name}: deliver-error code must be a string")))?;
    let message = items[4]
        .as_string()
        .ok_or_else(|| UiTestError(format!("{name}: deliver-error message must be a string")))?;
    Ok(UiTestStep::DeliverError {
        kind: resource_kind(name, &items[1])?,
        id: resource_id(name, &items[2])?,
        error: HostError {
            code: host_error_code(name, code)?,
            message: message.to_owned(),
        },
        span: node.span,
    })
}

fn resource_kind(name: &str, node: &Node) -> Result<ResourceKind, UiTestError> {
    match node.as_symbol() {
        Some("command") => Ok(ResourceKind::Command),
        Some("subscription") => Ok(ResourceKind::Subscription),
        _ => Err(UiTestError(format!(
            "{name}: resource kind must be command or subscription"
        ))),
    }
}

fn resource_id(name: &str, node: &Node) -> Result<String, UiTestError> {
    node.as_string()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| UiTestError(format!("{name}: resource id must be a nonempty string")))
}

fn host_error_code(name: &str, code: &str) -> Result<HostErrorCode, UiTestError> {
    match code {
        "unsupported-capability" => Ok(HostErrorCode::UnsupportedCapability),
        "permission-denied" => Ok(HostErrorCode::PermissionDenied),
        "invalid-request" => Ok(HostErrorCode::InvalidRequest),
        "cancelled" => Ok(HostErrorCode::Cancelled),
        "host-failure" => Ok(HostErrorCode::HostFailure),
        _ => Err(UiTestError(format!(
            "{name}: unsupported deliver-error code `{code}`"
        ))),
    }
}

fn parse_assert(name: &str, node: &Node, items: &[Node]) -> Result<UiTestStep, UiTestError> {
    if items.len() != 2 {
        return Err(UiTestError(format!(
            "{name}: assert expects one condition, for example `(assert (= 1 (ui.test.state)))`"
        )));
    }
    let equal = items[1].as_form().ok_or_else(|| {
        UiTestError(format!(
            "{name}: assert condition must be `(= expected actual)`"
        ))
    })?;
    if equal.len() != 3 || equal.first().and_then(Node::as_symbol) != Some("=") {
        return Err(UiTestError(format!(
            "{name}: ui.test assertions must use `(assert (= expected (ui.test.state|html|tree|commands|subscriptions)))`"
        )));
    }
    let left = actual_accessor(&equal[1]);
    let right = actual_accessor(&equal[2]);
    let (expected, actual) = match (left, right) {
        (Some(actual), None) => (equal[2].clone(), actual),
        (None, Some(actual)) => (equal[1].clone(), actual),
        _ => {
            return Err(UiTestError(format!(
                "{name}: assert must compare exactly one ui.test accessor"
            )))
        }
    };
    Ok(UiTestStep::Assert {
        expected,
        actual,
        span: node.span,
    })
}

struct PreparedTest {
    name: String,
    capabilities: Vec<Capability>,
    uses_fake_host: bool,
    steps: Vec<PreparedStep>,
}

enum PreparedStep {
    Dispatch {
        action: String,
        span: Span,
    },
    Assert {
        expected: String,
        actual: UiTestActual,
        span: Span,
    },
    Deliver {
        kind: ResourceKind,
        id: String,
        result: String,
        span: Span,
    },
    DeliverError {
        kind: ResourceKind,
        id: String,
        error: HostError,
        span: Span,
    },
}

fn prepare_steps(module: &mut Module, tests: &[UiTest]) -> Result<Vec<PreparedTest>, UiTestError> {
    let lowerer = Lowerer;
    let mut prepared = vec![];
    for (test_index, test) in tests.iter().enumerate() {
        let mut steps = vec![];
        for (step_index, step) in test.steps.iter().enumerate() {
            let name = format!("__jisp_ui_test_{test_index}_{step_index}");
            let prepared_step = match step {
                UiTestStep::Dispatch { action, span } => {
                    let value = lowerer.lower_expr(action).map_err(lower_error)?;
                    module.definitions.push(Definition {
                        name: name.clone(),
                        public: false,
                        value,
                        span: *span,
                    });
                    PreparedStep::Dispatch {
                        action: name,
                        span: *span,
                    }
                }
                UiTestStep::Assert {
                    expected,
                    actual,
                    span,
                } => {
                    let value = lowerer.lower_expr(expected).map_err(lower_error)?;
                    module.definitions.push(Definition {
                        name: name.clone(),
                        public: false,
                        value,
                        span: *span,
                    });
                    PreparedStep::Assert {
                        expected: name,
                        actual: *actual,
                        span: *span,
                    }
                }
                UiTestStep::Deliver {
                    kind,
                    id,
                    result,
                    span,
                } => {
                    let value = lowerer.lower_expr(result).map_err(lower_error)?;
                    module.definitions.push(Definition {
                        name: name.clone(),
                        public: false,
                        value,
                        span: *span,
                    });
                    PreparedStep::Deliver {
                        kind: *kind,
                        id: id.clone(),
                        result: name,
                        span: *span,
                    }
                }
                UiTestStep::DeliverError {
                    kind,
                    id,
                    error,
                    span,
                } => PreparedStep::DeliverError {
                    kind: *kind,
                    id: id.clone(),
                    error: error.clone(),
                    span: *span,
                },
                UiTestStep::Supports { .. } => {
                    unreachable!("setup steps are removed before preparation")
                }
            };
            steps.push(prepared_step);
        }
        prepared.push(PreparedTest {
            name: test.name.clone(),
            capabilities: test.capabilities.clone(),
            uses_fake_host: !test.capabilities.is_empty()
                || test.steps.iter().any(|step| {
                    matches!(
                        step,
                        UiTestStep::Deliver { .. } | UiTestStep::DeliverError { .. }
                    )
                }),
            steps,
        });
    }
    Ok(prepared)
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    test: &PreparedTest,
    evaluator: &mut Evaluator,
    module_env: &jisp_eval::Env,
    program: &crate::Program,
    component: &str,
    init: &Value,
    update: &Value,
    view: &Value,
    html: &Value,
    app_span: Span,
) -> UiTestOutcome {
    let mut state = init.clone();
    let mut commands = vec![];
    let mut subscriptions = vec![];
    let mut host = test
        .uses_fake_host
        .then(|| FakeHost::with_capabilities(test.capabilities.clone()));
    let mut assertions = 0;
    for step in &test.steps {
        let result = match step {
            PreparedStep::Dispatch { action, span } => {
                lookup(module_env, action, "dispatch action").and_then(|action| {
                    reduce_and_reconcile(
                        evaluator,
                        update,
                        &mut state,
                        &mut commands,
                        &mut subscriptions,
                        host.as_mut(),
                        action,
                        *span,
                    )
                })
            }
            PreparedStep::Deliver {
                kind,
                id,
                result,
                span,
            } => lookup(module_env, result, "delivery result")
                .and_then(portable_json)
                .and_then(|result| {
                    let host = host.as_mut().ok_or_else(|| {
                        UiTestError(
                            "ui.test delivery requires one or more supports setup steps".to_owned(),
                        )
                    })?;
                    delivery_action(host, *kind, id, Delivery::Ok(result)).ok_or_else(|| {
                        UiTestError(format!(
                            "delivery at {} has no live {kind:?} `{id}` with a completion action",
                            span.start
                        ))
                    })
                })
                .and_then(|action| {
                    reduce_and_reconcile(
                        evaluator,
                        update,
                        &mut state,
                        &mut commands,
                        &mut subscriptions,
                        host.as_mut(),
                        action,
                        *span,
                    )
                }),
            PreparedStep::DeliverError {
                kind,
                id,
                error,
                span,
            } => host
                .as_mut()
                .ok_or_else(|| {
                    UiTestError(
                        "ui.test delivery requires one or more supports setup steps".to_owned(),
                    )
                })
                .and_then(|host| {
                    delivery_action(host, *kind, id, Delivery::Err(error.clone())).ok_or_else(
                        || {
                            UiTestError(format!(
                        "delivery at {} has no live {kind:?} `{id}` with a completion action",
                        span.start
                    ))
                        },
                    )
                })
                .and_then(|action| {
                    reduce_and_reconcile(
                        evaluator,
                        update,
                        &mut state,
                        &mut commands,
                        &mut subscriptions,
                        host.as_mut(),
                        action,
                        *span,
                    )
                }),
            PreparedStep::Assert {
                expected,
                actual,
                span,
            } => {
                assertions += 1;
                assert_consistent(
                    evaluator, module_env, program, component, &state, view, html, app_span,
                )
                .and_then(|rendered| {
                    let expected = lookup(module_env, expected, "assertion expected value")?;
                    let actual = match actual {
                        UiTestActual::State => state.clone(),
                        UiTestActual::Html => rendered.html,
                        UiTestActual::Tree => rendered.tree,
                        UiTestActual::Commands => Value::List(commands.clone()),
                        UiTestActual::Subscriptions => Value::List(subscriptions.clone()),
                    };
                    let equal = expected
                        .structurally_equal(&actual)
                        .map_err(runtime_error)?;
                    if equal {
                        Ok(())
                    } else {
                        Err(UiTestError(format!(
                            "assertion at {} failed: expected {}, got {}",
                            span.start,
                            expected.display_string(),
                            actual.display_string()
                        )))
                    }
                })
            }
        };
        if let Err(error) = result {
            return UiTestOutcome {
                name: test.name.clone(),
                assertions,
                failure: Some(error.to_string()),
            };
        }
    }
    UiTestOutcome {
        name: test.name.clone(),
        assertions,
        failure: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn reduce_and_reconcile(
    evaluator: &mut Evaluator,
    update: &Value,
    state: &mut Value,
    commands: &mut Vec<Value>,
    subscriptions: &mut Vec<Value>,
    host: Option<&mut FakeHost>,
    action: Value,
    span: Span,
) -> Result<(), UiTestError> {
    let result = evaluator
        .apply(update.clone(), &[state.clone(), action], span)
        .map_err(runtime_error)
        .and_then(|result| normalize_update_result(result, span).map_err(runtime_error))?;
    if let Some(host) = host {
        host.reconcile_declared_resources(&result.commands, &result.subscriptions)
            .map_err(|error| {
                UiTestError(format!("ui.test fake host reconciliation failed: {error}"))
            })?;
    }
    *state = result.state;
    *commands = result.commands;
    *subscriptions = result.subscriptions;
    Ok(())
}

fn delivery_action(
    host: &mut FakeHost,
    kind: ResourceKind,
    id: &str,
    result: Delivery,
) -> Option<Value> {
    match kind {
        ResourceKind::Command => host.deliver_current_command_action(Owner::App, id, result),
        ResourceKind::Subscription => {
            host.deliver_current_subscription_action(Owner::App, id, result)
        }
    }
}

fn portable_json(value: Value) -> Result<JsonValue, UiTestError> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(value)),
        Value::Int(value) => Ok(JsonValue::Number(Number::from(value))),
        Value::Float(value) => Number::from_f64(value)
            .map(JsonValue::Number)
            .ok_or_else(|| {
                UiTestError("ui.test delivery result cannot contain NaN or infinity".to_owned())
            }),
        Value::Str(value) => Ok(JsonValue::String(value.to_string())),
        Value::List(values) => values
            .into_iter()
            .map(portable_json)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        Value::Obj(values) => values
            .into_iter()
            .map(|(key, value)| portable_json(value).map(|value| (key, value)))
            .collect::<Result<JsonMap<_, _>, _>>()
            .map(JsonValue::Object),
        Value::Variant { tag, fields } => Ok(serde_json::json!({
            "tag": tag,
            "fields": fields
                .into_iter()
                .map(portable_json)
                .collect::<Result<Vec<_>, _>>()?,
        })),
        Value::BigInt(_) => Err(UiTestError(
            "ui.test delivery result cannot contain bigint".to_owned(),
        )),
        Value::Builtin(_) | Value::Closure(_) | Value::Constructor(_) | Value::Uninitialized(_) => {
            Err(UiTestError(format!(
                "ui.test delivery result must be portable data, got {}",
                value.type_name()
            )))
        }
    }
}

struct Rendered {
    tree: Value,
    html: Value,
}

#[allow(clippy::too_many_arguments)]
fn assert_consistent(
    evaluator: &mut Evaluator,
    module_env: &jisp_eval::Env,
    program: &crate::Program,
    component: &str,
    state: &Value,
    view: &Value,
    html: &Value,
    span: Span,
) -> Result<Rendered, UiTestError> {
    let reference = evaluator
        .apply(view.clone(), std::slice::from_ref(state), span)
        .map_err(runtime_error)?;
    let tree = execute(
        program,
        evaluator,
        module_env,
        component,
        std::slice::from_ref(state),
    )
    .map_err(|error| UiTestError(format!("JUIR render failed: {error}")))?;
    if !ui_values_equal(&reference, &tree)? {
        return Err(UiTestError(format!(
            "reference UI and JUIR disagree: reference {}, JUIR {}",
            reference.display_string(),
            tree.display_string()
        )));
    }
    let html = evaluator
        .apply(html.clone(), &[reference], span)
        .map_err(runtime_error)?;
    Ok(Rendered { tree, html })
}

/// JUIR keeps event handlers as executable values until a host assigns them an
/// opaque listener id. Two independently evaluated closures have no portable
/// structural identity, so renderer conformance compares their presence and
/// surrounding event shape rather than attempting function equality.
fn ui_values_equal(left: &Value, right: &Value) -> Result<bool, UiTestError> {
    match (left, right) {
        (Value::Builtin(_) | Value::Closure(_), Value::Builtin(_) | Value::Closure(_)) => Ok(true),
        (Value::List(left), Value::List(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left, right) in left.iter().zip(right) {
                if !ui_values_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Obj(left), Value::Obj(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (key, left) in left {
                let Some(right) = right.get(key) else {
                    return Ok(false);
                };
                let equal = if key == "events" {
                    event_bindings_equal(left, right)?
                } else {
                    ui_values_equal(left, right)?
                };
                if !equal {
                    return Ok(false);
                }
            }
            Ok(true)
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
            if left_tag != right_tag || left_fields.len() != right_fields.len() {
                return Ok(false);
            }
            for (left, right) in left_fields.iter().zip(right_fields) {
                if !ui_values_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => left.structurally_equal(right).map_err(|error| {
            UiTestError(format!(
                "ui.test renderer comparison cannot compare {} with {}: {error}",
                left.type_name(),
                right.type_name()
            ))
        }),
    }
}

fn event_bindings_equal(left: &Value, right: &Value) -> Result<bool, UiTestError> {
    let (Value::Obj(left), Value::Obj(right)) = (left, right) else {
        return Ok(false);
    };
    if left.len() != right.len() {
        return Ok(false);
    }
    for (name, left) in left {
        let Some(right) = right.get(name) else {
            return Ok(false);
        };
        let right_handler = match right {
            Value::Obj(descriptor) => descriptor.get("handler"),
            _ => None,
        };
        if !matches!(left, Value::Builtin(_) | Value::Closure(_))
            || !matches!(right_handler, Some(Value::Builtin(_) | Value::Closure(_)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn lookup(env: &jisp_eval::Env, name: &str, role: &str) -> Result<Value, UiTestError> {
    env.lookup(name)
        .map_err(|error| UiTestError(format!("ui.test missing {role} `{name}`: {error}")))
}

fn lower_error(error: jisp_ir::LowerError) -> UiTestError {
    UiTestError(format!("ui.test lowering failed: {error}"))
}

fn runtime_error(error: jisp_eval::RuntimeError) -> UiTestError {
    UiTestError(format!("ui.test runtime failed: {error}"))
}

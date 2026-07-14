//! Browser-facing WebAssembly entry points for the interpreter-backed playground.

use jisp::jisp_core::{ui_element, Node, NodeKind, SourceId, Span, SyntaxParser};
#[cfg(feature = "juir")]
use jisp::jisp_eval::Env;
use jisp::jisp_eval::{normalize_update_result, Evaluator, Value};
#[cfg(feature = "juir")]
use jisp::jisp_types::Inferencer;
use jisp::jisp_ui::effects::{
    Capability, Delivery, FakeHost, HostError, HostErrorCode, Owner, ResourceKind,
};
#[cfg(feature = "juir")]
use jisp::jisp_ui::{
    changed_paths, compile as compile_juir, execute_incremental_cached,
    mount_plan as juir_mount_plan, ChangeSet, Execution as JuirExecution, ExecutionStats,
    Program as JuirProgram,
};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_ws::WsParser;
use jisp_syntax_yaml::YamlParser;
use serde_json::{json, Map, Number, Value as JsonValue};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_html(source: &str) -> Result<String, JsValue> {
    render_html_source(source).map_err(|error| JsValue::from_str(&error))
}

/// Render a `ui.app` program into a versioned SSR payload.
///
/// The payload keeps escaped HTML, serializable initial state, and the
/// renderer-neutral tree separate so an embedding host chooses its own safe
/// document serialization and hydration strategy.
#[wasm_bindgen]
pub fn render_ssr(source: &str) -> Result<String, JsValue> {
    let mut session = PlaygroundSession::new();
    session.load_source(source).map_err(js_error)?;
    session.ssr_payload().map_err(js_error)
}

/// An update-driven Jisp UI program loaded by a browser host.
///
/// `load` evaluates the program once and creates the initial state. Each
/// `dispatch` invokes the event closure, passes its result to the update function, and
/// returns a fresh renderer-neutral tree as JSON.
#[wasm_bindgen]
pub struct PlaygroundSession {
    runtime: Option<Runtime>,
}

impl Default for PlaygroundSession {
    fn default() -> Self {
        Self::new()
    }
}

struct Runtime {
    evaluator: Evaluator,
    state: Value,
    update: Value,
    #[cfg(not(feature = "juir"))]
    view: Value,
    #[cfg(feature = "juir")]
    program: JuirProgram,
    #[cfg(feature = "juir")]
    module_env: Env,
    #[cfg(feature = "juir")]
    component: String,
    #[cfg(feature = "juir")]
    last_value: Option<Value>,
    #[cfg(feature = "juir")]
    changes: ChangeSet,
    #[cfg(feature = "juir")]
    last_execution: ExecutionStats,
    #[cfg(feature = "juir")]
    last_juir_execution: Option<JuirExecution>,
    handlers: Vec<Value>,
    last_render: Option<String>,
    last_tree: Option<JsonValue>,
    renders: usize,
    skipped_renders: usize,
    last_render_skipped: bool,
    desired_commands: Vec<Value>,
    desired_subscriptions: Vec<Value>,
    effect_host: Option<FakeHost>,
    span: Span,
}

#[wasm_bindgen]
impl PlaygroundSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Compile and evaluate source, returning its first UI tree.
    pub fn load(&mut self, source: &str) -> Result<String, JsValue> {
        self.load_source(source).map_err(js_error)
    }

    /// Compile and evaluate source in one of the playground's supported syntaxes.
    pub fn load_syntax(&mut self, source: &str, syntax: &str) -> Result<String, JsValue> {
        self.load_source_syntax(source, syntax).map_err(js_error)
    }

    /// Process a browser event through one event handler and the declared update function.
    pub fn dispatch(&mut self, handler: usize, event_json: &str) -> Result<String, JsValue> {
        self.dispatch_event(handler, event_json).map_err(js_error)
    }

    /// Process one event and return a compact, host-neutral patch batch.
    pub fn dispatch_patches(
        &mut self,
        handler: usize,
        event_json: &str,
    ) -> Result<String, JsValue> {
        self.dispatch_patch_event(handler, event_json)
            .map_err(js_error)
    }

    /// Return the most recent complete renderer-neutral tree for recovery.
    pub fn snapshot(&self) -> Result<String, JsValue> {
        self.runtime
            .as_ref()
            .and_then(|runtime| runtime.last_render.clone())
            .ok_or_else(|| JsValue::from_str("load a ui.app program before reading its snapshot"))
    }

    /// Return escaped HTML, initial state, and the structural tree for SSR.
    pub fn ssr(&mut self) -> Result<String, JsValue> {
        self.ssr_payload().map_err(js_error)
    }

    /// Return renderer-neutral execution counters for playground diagnostics.
    pub fn metrics(&self) -> Result<String, JsValue> {
        self.metrics_json().map_err(js_error)
    }

    /// Return resource declarations from the most recent reducer turn. They
    /// are data for the embedding host; this interpreter does not execute them.
    pub fn desired_resources(&self) -> Result<String, JsValue> {
        self.desired_resources_json().map_err(js_error)
    }

    /// Configure the immutable capability set of an embedding effect host.
    /// The host receives versioned resource declarations and must later return
    /// completions through [`Self::deliver_effect`]. Calling this twice would
    /// discard active generations, so it is rejected.
    pub fn configure_effect_host(&mut self, capabilities_json: &str) -> Result<(), JsValue> {
        self.configure_effect_host_json(capabilities_json)
            .map_err(js_error)
    }

    /// Deliver one generation-bound effect completion. The JSON envelope is
    /// either `{ "ok": value }` or
    /// `{ "error": { "code": string, "message": string } }`.
    #[wasm_bindgen(js_name = deliverEffect)]
    pub fn deliver_effect(
        &mut self,
        kind: &str,
        id: &str,
        generation: u64,
        completion_json: &str,
    ) -> Result<String, JsValue> {
        self.deliver_effect_json(kind, id, generation, completion_json)
            .map_err(js_error)
    }

    /// Return stable source locations for the currently compiled JUIR plan.
    ///
    /// These locations identify compiler-plan paths, not mutable DOM nodes, so
    /// hosts can attach diagnostics without owning Jisp source semantics.
    pub fn source_map(&self) -> Result<String, JsValue> {
        self.source_map_json().map_err(js_error)
    }

    /// Return a static JUIR mount skeleton for the currently loaded app.
    /// Dynamic slots/blocks remain values produced by the canonical executor.
    pub fn mount_plan(&self) -> Result<String, JsValue> {
        self.mount_plan_json().map_err(js_error)
    }

    /// Run fixture-only portable UI scenarios embedded in the current source.
    /// They execute in Wasm but never touch the browser DOM.
    pub fn run_tests(&self, source: &str, syntax: &str) -> Result<String, JsValue> {
        run_ui_tests_source(source, syntax).map_err(js_error)
    }
}

impl PlaygroundSession {
    fn load_source(&mut self, source: &str) -> Result<String, String> {
        self.load_source_syntax(source, "lisp")
    }

    fn load_source_syntax(&mut self, source: &str, syntax: &str) -> Result<String, String> {
        let extension = syntax_extension(syntax)?;
        let name = format!("playground.{extension}");
        let source = source_without_ui_tests(source, syntax)?;
        let parsed = jisp::check_detailed(&name, &source).map_err(render_module_error)?;
        if !parsed.module.imports.is_empty() {
            return Err("Playground ui.app programs cannot import local files yet".to_owned());
        }
        let app = parsed.module.ui_app.clone().ok_or_else(|| {
            "Playground source must declare `(ui.app init update app)`".to_owned()
        })?;
        #[cfg(feature = "juir")]
        let program = compile_juir(
            &Inferencer::with_prelude()
                .infer_typed_module(parsed.module.clone())
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        #[cfg(feature = "juir")]
        if !program.components.contains_key(&app.app) {
            return Err(format!(
                "ui.app view `{}` must be a Jisp UI component",
                app.app
            ));
        }
        let mut evaluator = Evaluator::new();
        let loaded = evaluator
            .load_module(&parsed.module)
            .map_err(|error| error.to_string())?;
        let state = loaded
            .env
            .lookup(&app.init)
            .map_err(|error| error.to_string())?;
        let update = loaded
            .env
            .lookup(&app.update)
            .map_err(|error| error.to_string())?;
        #[cfg(not(feature = "juir"))]
        let view = loaded
            .env
            .lookup(&app.app)
            .map_err(|error| error.to_string())?;

        self.runtime = Some(Runtime {
            evaluator,
            state,
            update,
            #[cfg(not(feature = "juir"))]
            view,
            #[cfg(feature = "juir")]
            program,
            #[cfg(feature = "juir")]
            module_env: loaded.env,
            #[cfg(feature = "juir")]
            component: app.app,
            #[cfg(feature = "juir")]
            last_value: None,
            #[cfg(feature = "juir")]
            changes: ChangeSet {
                unknown: true,
                ..ChangeSet::default()
            },
            #[cfg(feature = "juir")]
            last_execution: ExecutionStats::default(),
            #[cfg(feature = "juir")]
            last_juir_execution: None,
            handlers: vec![],
            last_render: None,
            last_tree: None,
            renders: 0,
            skipped_renders: 0,
            last_render_skipped: false,
            desired_commands: vec![],
            desired_subscriptions: vec![],
            effect_host: None,
            span: app.span,
        });
        self.render()
    }

    fn dispatch_event(&mut self, handler: usize, event_json: &str) -> Result<String, String> {
        let event = serde_json::from_str(event_json)
            .map_err(|error| format!("browser event is not JSON: {error}"))?;
        let event = value_from_json(event)?;
        let action = {
            let runtime = self
                .runtime
                .as_mut()
                .ok_or_else(|| "load a ui.app program before dispatching events".to_owned())?;
            let handler = runtime
                .handlers
                .get(handler)
                .cloned()
                .ok_or_else(|| format!("unknown UI event handler {handler}"))?;
            runtime
                .evaluator
                .apply(handler, &[event], runtime.span)
                .map_err(|error| error.to_string())?
        };
        self.apply_action(action)
    }

    fn configure_effect_host_json(&mut self, capabilities_json: &str) -> Result<(), String> {
        let capabilities = parse_capabilities(capabilities_json)?;
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| "load a ui.app program before configuring an effect host".to_owned())?;
        if runtime.effect_host.is_some() {
            return Err("an effect host is already configured for this session".to_owned());
        }
        let mut host = FakeHost::with_capabilities(capabilities);
        host.reconcile_declared_resources(
            &runtime.desired_commands,
            &runtime.desired_subscriptions,
        )
        .map_err(|error| format!("effect host reconciliation failed: {error}"))?;
        runtime.effect_host = Some(host);
        Ok(())
    }

    fn deliver_effect_json(
        &mut self,
        kind: &str,
        id: &str,
        generation: u64,
        completion_json: &str,
    ) -> Result<String, String> {
        let kind = parse_resource_kind(kind)?;
        if id.is_empty() {
            return Err("effect id must be nonempty".to_owned());
        }
        let completion = serde_json::from_str(completion_json)
            .map_err(|error| format!("effect completion is not JSON: {error}"))?;
        let delivery = parse_delivery(completion)?;
        let action = {
            let runtime = self
                .runtime
                .as_mut()
                .ok_or_else(|| "load a ui.app program before delivering an effect".to_owned())?;
            let host = runtime.effect_host.as_mut().ok_or_else(|| {
                "configure an effect host before delivering effect completions".to_owned()
            })?;
            match kind {
                ResourceKind::Command => {
                    host.deliver_command_action(Owner::App, id, generation, delivery)
                }
                ResourceKind::Subscription => {
                    host.deliver_subscription_action(Owner::App, id, generation, delivery)
                }
            }
            .ok_or_else(|| {
                format!(
                    "effect completion is not current for {} `{id}` generation {generation}",
                    resource_kind_name(kind)
                )
            })?
        };
        self.apply_action(action)
    }

    fn apply_action(&mut self, action: Value) -> Result<String, String> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| "load a ui.app program before dispatching actions".to_owned())?;
        #[cfg(feature = "juir")]
        let previous_state = runtime.state.clone();
        let result = runtime
            .evaluator
            .apply(
                runtime.update.clone(),
                &[runtime.state.clone(), action],
                runtime.span,
            )
            .map_err(|error| error.to_string())?;
        let result =
            normalize_update_result(result, runtime.span).map_err(|error| error.to_string())?;
        if let Some(host) = &mut runtime.effect_host {
            host.reconcile_declared_resources(&result.commands, &result.subscriptions)
                .map_err(|error| format!("effect host reconciliation failed: {error}"))?;
        }
        runtime.state = result.state;
        runtime.desired_commands = result.commands;
        runtime.desired_subscriptions = result.subscriptions;
        #[cfg(feature = "juir")]
        {
            let changes = changed_paths("state", &previous_state, &runtime.state);
            if changes.paths.is_empty() {
                runtime.skipped_renders += 1;
                runtime.last_render_skipped = true;
                return runtime
                    .last_render
                    .clone()
                    .ok_or_else(|| "JUIR runtime has no initial render to reuse".to_owned());
            }
            runtime.changes = changes;
        }
        self.render()
    }

    fn dispatch_patch_event(&mut self, handler: usize, event_json: &str) -> Result<String, String> {
        let previous = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.last_tree.clone())
            .ok_or_else(|| "load a ui.app program before dispatching events".to_owned())?;
        self.dispatch_event(handler, event_json)?;
        let current = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.last_tree.as_ref())
            .ok_or_else(|| "JUIR runtime has no current tree after dispatch".to_owned())?;
        let mut patches = vec![];
        collect_tree_patches(&previous, current, "0", &mut patches);
        serde_json::to_string(&json!({ "patches": patches })).map_err(|error| error.to_string())
    }

    fn desired_resources_json(&self) -> Result<String, String> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| "load a ui.app program before reading resources".to_owned())?;
        serde_json::to_string(&json!({
            "protocol": "jisp-ui-resources/1",
            "commands": runtime
                .desired_commands
                .iter()
                .map(|value| resource_json(value, runtime.effect_host.as_ref(), ResourceKind::Command))
                .collect::<Result<Vec<_>, _>>()?,
            "subscriptions": runtime
                .desired_subscriptions
                .iter()
                .map(|value| resource_json(value, runtime.effect_host.as_ref(), ResourceKind::Subscription))
                .collect::<Result<Vec<_>, _>>()?,
        }))
        .map_err(|error| error.to_string())
    }

    fn source_map_json(&self) -> Result<String, String> {
        #[cfg(feature = "juir")]
        {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| "load a ui.app program before reading its source map".to_owned())?;
            serde_json::to_string(&json!({
                "protocol": "jisp-ui-source-map/1",
                "entries": runtime.program.source_map.iter().map(|entry| json!({
                    "component": entry.component,
                    "path": entry.path,
                    "kind": entry.kind.as_str(),
                    "source": entry.span.source.0,
                    "start": entry.span.start,
                    "end": entry.span.end,
                })).collect::<Vec<_>>(),
            }))
            .map_err(|error| error.to_string())
        }
        #[cfg(not(feature = "juir"))]
        Err("JUIR source maps require the `juir` feature".to_owned())
    }

    fn mount_plan_json(&self) -> Result<String, String> {
        #[cfg(feature = "juir")]
        {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| "load a ui.app program before reading its mount plan".to_owned())?;
            serde_json::to_string(
                &juir_mount_plan(&runtime.program, &runtime.component)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        #[cfg(not(feature = "juir"))]
        Err("JUIR mount plans require the `juir` feature".to_owned())
    }

    fn render(&mut self) -> Result<String, String> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| "load a ui.app program before rendering".to_owned())?;
        #[cfg(feature = "juir")]
        let execution = execute_incremental_cached(
            &runtime.program,
            &mut runtime.evaluator,
            &runtime.module_env,
            &runtime.component,
            std::slice::from_ref(&runtime.state),
            runtime.last_juir_execution.as_ref(),
            &runtime.changes,
        )
        .map_err(|error| error.to_string())?;
        #[cfg(feature = "juir")]
        let vnode = execution.value.clone();
        #[cfg(feature = "juir")]
        {
            runtime.last_execution = execution.stats.clone();
            runtime.last_juir_execution = Some(execution);
        }
        #[cfg(not(feature = "juir"))]
        let vnode = runtime
            .evaluator
            .apply(
                runtime.view.clone(),
                std::slice::from_ref(&runtime.state),
                runtime.span,
            )
            .map_err(|error| error.to_string())?;
        let mut handlers = vec![];
        let tree = ui_node(&vnode, &mut handlers)?;
        runtime.handlers = handlers;
        let rendered = serde_json::to_string(&tree).map_err(|error| error.to_string())?;
        runtime.renders += 1;
        runtime.last_render_skipped = false;
        runtime.last_render = Some(rendered.clone());
        runtime.last_tree = Some(tree);
        #[cfg(feature = "juir")]
        {
            runtime.last_value = Some(vnode);
            runtime.changes = ChangeSet {
                unknown: true,
                ..ChangeSet::default()
            };
        }
        Ok(rendered)
    }

    fn metrics_json(&self) -> Result<String, String> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| "load a ui.app program before reading metrics".to_owned())?;
        #[cfg(not(feature = "juir"))]
        let metrics = json!({
            "renders": runtime.renders,
            "skippedRenders": runtime.skipped_renders,
            "lastRenderSkipped": runtime.last_render_skipped,
        });
        #[cfg(feature = "juir")]
        let metrics = json!({
            "renders": runtime.renders,
            "skippedRenders": runtime.skipped_renders,
            "lastRenderSkipped": runtime.last_render_skipped,
            "execution": {
                "evaluatedSlots": runtime.last_execution.evaluated_slots,
                "reusedSlots": runtime.last_execution.reused_slots,
                "reusedSubtrees": runtime.last_execution.reused_subtrees,
                "reusedBlocks": runtime.last_execution.reused_blocks,
                "reusedItems": runtime.last_execution.reused_items,
                "reusedComponents": runtime.last_execution.reused_components,
                "componentDecisions": runtime.last_execution.component_decisions.iter().map(|decision| json!({
                    "component": decision.component,
                    "path": decision.path,
                    "decision": decision.outcome.decision(),
                    "reason": decision.outcome.reason(),
                })).collect::<Vec<_>>(),
            },
        });
        serde_json::to_string(&metrics).map_err(|error| error.to_string())
    }

    fn ssr_payload(&mut self) -> Result<String, String> {
        #[cfg(feature = "juir")]
        {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| "load a ui.app program before rendering SSR".to_owned())?;
            let tree = runtime
                .last_tree
                .clone()
                .ok_or_else(|| "JUIR runtime has no serializable SSR tree".to_owned())?;
            let html = render_ssr_html(&tree)?;
            serde_json::to_string(&json!({
                "protocol": "jisp-ui-ssr/1",
                "html": html,
                "state": json_value(&runtime.state)?,
                "tree": tree,
            }))
            .map_err(|error| error.to_string())
        }
        #[cfg(not(feature = "juir"))]
        Err("SSR payloads require the `juir` feature".to_owned())
    }
}

/// Serialize the renderer-neutral tree as safe server HTML and retain the
/// stable host markers needed to attach a browser host without replacing a
/// matching node. These markers are host protocol metadata, never Jisp source
/// attributes, so a user cannot override them through `attr`.
#[cfg(feature = "juir")]
fn render_ssr_html(tree: &JsonValue) -> Result<String, String> {
    let mut html = String::new();
    render_ssr_node(tree, "0", &mut html)?;
    Ok(html)
}

#[cfg(feature = "juir")]
fn render_ssr_node(tree: &JsonValue, path: &str, output: &mut String) -> Result<(), String> {
    match tree.get("kind").and_then(JsonValue::as_str) {
        Some("text") => {
            escape_ssr_text(&ssr_scalar_display(tree.get("value")), output);
            Ok(())
        }
        Some("element") => {
            let tag = tree
                .get("tag")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| "SSR UI element is missing a string `tag`".to_owned())?;
            if ui_element(tag).is_none() {
                return Err(format!("SSR UI element has unsupported tag `{tag}`"));
            }
            output.push('<');
            output.push_str(tag);
            render_ssr_attributes(tree.get("attrs"), output)?;
            render_ssr_attributes(tree.get("props"), output)?;
            render_ssr_classes(tree.get("classes"), output)?;
            output.push_str(" data-jisp-path=\"");
            escape_ssr_attribute(path, output);
            output.push('"');
            if let Some(key) = tree.get("key").filter(|key| !key.is_null()) {
                output.push_str(" data-jisp-key=\"");
                escape_ssr_attribute(&ssr_key(key)?, output);
                output.push('"');
            }
            output.push('>');
            let children = tree
                .get("children")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| "SSR UI element is missing `children`".to_owned())?;
            for (index, child) in children.iter().enumerate() {
                render_ssr_node(child, &format!("{path}.{index}"), output)?;
            }
            output.push_str("</");
            output.push_str(tag);
            output.push('>');
            Ok(())
        }
        _ => Err("SSR tree node must have kind `text` or `element`".to_owned()),
    }
}

#[cfg(feature = "juir")]
fn render_ssr_attributes(value: Option<&JsonValue>, output: &mut String) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let attributes = value
        .as_object()
        .ok_or_else(|| "SSR UI attributes must be an object".to_owned())?;
    for (name, value) in attributes {
        let lower = name.to_ascii_lowercase();
        if !is_safe_ssr_attribute_name(name) {
            return Err(format!("SSR UI attribute `{name}` is invalid or unsafe"));
        }
        if matches!(lower.as_str(), "data-jisp-path" | "data-jisp-key") {
            return Err(format!(
                "SSR UI attribute `{name}` is reserved for hydration"
            ));
        }
        if value.is_null() || value == &JsonValue::Bool(false) {
            continue;
        }
        if value == &JsonValue::Bool(true) {
            output.push(' ');
            output.push_str(name);
            continue;
        }
        let value = ssr_scalar_display(Some(value));
        if matches!(lower.as_str(), "href" | "src")
            && value
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("javascript:")
        {
            return Err(format!("SSR UI {name} must not use a javascript: URL"));
        }
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        escape_ssr_attribute(&value, output);
        output.push('"');
    }
    Ok(())
}

#[cfg(feature = "juir")]
fn render_ssr_classes(value: Option<&JsonValue>, output: &mut String) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let classes = value
        .as_array()
        .ok_or_else(|| "SSR UI classes must be an array".to_owned())?;
    if classes.is_empty() {
        return Ok(());
    }
    output.push_str(" class=\"");
    for (index, class) in classes.iter().enumerate() {
        let class = class
            .as_str()
            .ok_or_else(|| "SSR UI class must be a string".to_owned())?;
        if index > 0 {
            output.push(' ');
        }
        escape_ssr_attribute(class, output);
    }
    output.push('"');
    Ok(())
}

#[cfg(feature = "juir")]
fn is_safe_ssr_attribute_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        && !name.to_ascii_lowercase().starts_with("on")
}

#[cfg(feature = "juir")]
fn ssr_key(key: &JsonValue) -> Result<String, String> {
    let kind = match key {
        JsonValue::String(_) => "string",
        JsonValue::Number(_) => "number",
        JsonValue::Bool(_) => "boolean",
        _ => return Err("SSR UI key must be a string, number, or bool".to_owned()),
    };
    Ok(format!(
        "{kind}:{}",
        serde_json::to_string(key).map_err(|error| error.to_string())?
    ))
}

#[cfg(feature = "juir")]
fn ssr_scalar_display(value: Option<&JsonValue>) -> String {
    match value {
        None | Some(JsonValue::Null) => String::new(),
        Some(JsonValue::Bool(value)) => value.to_string(),
        Some(JsonValue::Number(value)) => {
            if let Some(value) = value.as_i64() {
                value.to_string()
            } else if let Some(value) = value.as_u64() {
                value.to_string()
            } else if let Some(value) = value.as_f64() {
                if value == 0.0 {
                    "0".to_owned()
                } else {
                    value.to_string()
                }
            } else {
                value.to_string()
            }
        }
        Some(JsonValue::String(value)) => value.clone(),
        Some(_) => String::new(),
    }
}

#[cfg(feature = "juir")]
fn escape_ssr_text(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

#[cfg(feature = "juir")]
fn escape_ssr_attribute(value: &str, output: &mut String) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(character),
        }
    }
}

/// Convert a complete Jisp module between Lisp, canonical JSON, and restricted YAML syntax.
#[wasm_bindgen]
pub fn convert_source(source: &str, from: &str, to: &str) -> Result<String, JsValue> {
    let nodes = parse_source(source, from).map_err(js_error)?;
    format_source(&nodes, to).map_err(js_error)
}

fn syntax_extension(syntax: &str) -> Result<&'static str, String> {
    match syntax {
        "lisp" => Ok("lisp"),
        "json" => Ok("json"),
        "yaml" => Ok("yaml"),
        "ws" => Ok("ws"),
        _ => Err(format!("unsupported playground syntax `{syntax}`")),
    }
}

fn parse_source(source: &str, syntax: &str) -> Result<Vec<Node>, String> {
    let parsed = match syntax_extension(syntax)? {
        "lisp" => LispParser.parse_module(SourceId(0), source),
        "json" => JsonParser.parse_module(SourceId(0), source),
        "yaml" => YamlParser.parse_module(SourceId(0), source),
        "ws" => WsParser.parse_module(SourceId(0), source),
        _ => unreachable!("syntax_extension returns a closed set"),
    };
    parsed.map_err(|error| error.to_string())
}

fn source_without_ui_tests(source: &str, syntax: &str) -> Result<String, String> {
    let nodes = parse_source(source, syntax)?;
    let expanded = jisp::jisp_expand::expand_module(&nodes).map_err(|error| error.to_string())?;
    let suite = jisp::jisp_ui::testing::split_ui_tests(expanded.nodes)
        .map_err(|error| error.to_string())?;
    if suite.tests.is_empty() {
        Ok(source.to_owned())
    } else {
        format_source(&suite.module_nodes, syntax)
    }
}

fn run_ui_tests_source(source: &str, syntax: &str) -> Result<String, String> {
    let nodes = parse_source(source, syntax)?;
    let expanded = jisp::jisp_expand::expand_module(&nodes).map_err(|error| error.to_string())?;
    let suite = jisp::jisp_ui::testing::split_ui_tests(expanded.nodes)
        .map_err(|error| error.to_string())?;
    let outcomes =
        jisp::jisp_ui::testing::run_ui_tests(suite).map_err(|error| error.to_string())?;
    serde_json::to_string(&json!({
        "protocol": "jisp-ui-test/1",
        "tests": outcomes.into_iter().map(|outcome| json!({
            "name": outcome.name,
            "assertions": outcome.assertions,
            "passed": outcome.passed(),
            "failure": outcome.failure,
        })).collect::<Vec<_>>(),
    }))
    .map_err(|error| error.to_string())
}

fn format_source(nodes: &[Node], syntax: &str) -> Result<String, String> {
    match syntax_extension(syntax)? {
        "lisp" => Ok(format_lisp_module(nodes)),
        "json" => Ok(format_json_module(nodes)),
        "yaml" => Ok(format_yaml_module(nodes)),
        "ws" => Ok(format_ws_module(nodes)),
        _ => unreachable!("syntax_extension returns a closed set"),
    }
}

fn format_json_module(nodes: &[Node]) -> String {
    if nodes.is_empty() {
        return "[]\n".to_owned();
    }
    let inline = format!(
        "[{}]",
        nodes
            .iter()
            .map(format_json_inline)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if inline.chars().count() <= FORMAT_WIDTH
        && nodes.iter().all(|node| !json_node_needs_layout(node))
    {
        return format!("{inline}\n");
    }

    let mut output = String::from("[");
    for (index, node) in nodes.iter().enumerate() {
        output.push('\n');
        output.push_str("  ");
        output.push_str(&format_json_layout(node, 2));
        if index + 1 < nodes.len() {
            output.push(',');
        }
    }
    output.push_str("\n]\n");
    output
}

fn format_json_layout(node: &Node, indent: usize) -> String {
    let inline = format_json_inline(node);
    let NodeKind::Form(items) = &node.kind else {
        return inline;
    };
    if !json_node_needs_layout(node) && indent + inline.chars().count() <= FORMAT_WIDTH {
        return inline;
    }
    if items.first().and_then(Node::as_symbol) == Some("obj") && items.len() % 2 == 1 {
        return format_json_object(items, indent);
    }
    format_json_form(items, indent)
}

fn json_node_needs_layout(node: &Node) -> bool {
    let NodeKind::Form(items) = &node.kind else {
        return false;
    };
    items.iter().skip(1).any(is_json_compound)
}

fn format_json_form(items: &[Node], indent: usize) -> String {
    if items.is_empty() {
        return "[]".to_owned();
    }
    let child_indent = indent + 2;
    let mut output = String::from("[");
    let mut line_width = indent + 1;
    let mut multiline = false;
    let mut after_block = false;

    for (index, item) in items.iter().enumerate() {
        let rendered = format_json_layout(item, child_indent);
        let is_block = is_json_compound(item) || rendered.contains('\n');
        let separator_width = usize::from(index > 0) * 2;
        if !is_block
            && !after_block
            && line_width + separator_width + rendered.chars().count() <= FORMAT_WIDTH
        {
            if index > 0 {
                output.push_str(", ");
                line_width += 2;
            }
            output.push_str(&rendered);
            line_width += rendered.chars().count();
            continue;
        }

        if index > 0 {
            output.push(',');
        }
        output.push('\n');
        output.push_str(&" ".repeat(child_indent));
        output.push_str(&rendered);
        line_width = child_indent
            + rendered
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count();
        multiline = true;
        after_block = true;
    }

    if multiline {
        output.push('\n');
        output.push_str(&" ".repeat(indent));
    }
    output.push(']');
    output
}

fn format_json_object(items: &[Node], indent: usize) -> String {
    let child_indent = indent + 2;
    let mut output = format!("[{}", format_json_inline(&items[0]));

    for pair in items[1..].chunks_exact(2) {
        let key = format_json_layout(&pair[0], child_indent);
        let value = format_json_layout(&pair[1], child_indent);
        output.push_str(",\n");
        output.push_str(&" ".repeat(child_indent));
        if !value.contains('\n')
            && child_indent + key.chars().count() + 2 + value.chars().count() <= FORMAT_WIDTH
        {
            output.push_str(&key);
            output.push_str(", ");
            output.push_str(&value);
        } else {
            output.push_str(&key);
            output.push_str(",\n");
            output.push_str(&" ".repeat(child_indent));
            output.push_str(&value);
        }
    }
    output.push('\n');
    output.push_str(&" ".repeat(indent));
    output.push(']');
    output
}

fn is_json_compound(node: &Node) -> bool {
    let NodeKind::Form(items) = &node.kind else {
        return false;
    };
    !items.is_empty() && !is_json_string_template(items)
}

fn is_json_string_template(items: &[Node]) -> bool {
    matches!(
        items.first().and_then(Node::as_symbol),
        Some("str" | "str.lines")
    )
}

fn format_json_inline(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => serde_json::to_string(value.as_str()).expect("valid symbol"),
        NodeKind::String(value) => format!(
            "[\"str\", {}]",
            serde_json::to_string(value.as_ref()).expect("valid string")
        ),
        NodeKind::Form(items) => {
            let string_template = is_json_string_template(items);
            format!(
                "[{}]",
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        if string_template && index > 0 {
                            if let NodeKind::String(value) = &item.kind {
                                return serde_json::to_string(value.as_ref())
                                    .expect("valid string template part");
                            }
                        }
                        format_json_inline(item)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

fn format_lisp_module(nodes: &[Node]) -> String {
    format!(
        "{}\n",
        nodes
            .iter()
            .map(format_lisp_node)
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}

fn format_lisp_node(node: &Node) -> String {
    format_lisp_layout(node, 0)
}

const FORMAT_WIDTH: usize = 88;

fn format_lisp_layout(node: &Node, indent: usize) -> String {
    let inline = format_lisp_inline(node);
    let NodeKind::Form(items) = &node.kind else {
        return inline;
    };
    if indent + inline.chars().count() <= FORMAT_WIDTH && !prefers_lisp_block(items) {
        return inline;
    }
    if items.is_empty() {
        return "()".to_owned();
    }
    if is_reader_form(items) {
        let prefix = items[0].as_symbol().expect("quoted form symbol");
        return format!(
            "{prefix}{}",
            format_lisp_layout(&items[1], indent + prefix.len())
        );
    }
    if items.first().and_then(Node::as_symbol) == Some("obj") && items.len() % 2 == 1 {
        return format_lisp_object(items, indent);
    }

    let head = format_lisp_inline(&items[0]);
    let child_indent = indent + 2;
    let mut output = format!("({head}");
    for item in &items[1..] {
        let item_inline = format_lisp_inline(item);
        let current_width = indent
            + output
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count();
        if (!matches!(item.kind, NodeKind::Form(_))
            || matches!(&item.kind, NodeKind::Form(children) if children.is_empty()))
            && current_width + 1 + item_inline.chars().count() <= FORMAT_WIDTH
        {
            output.push(' ');
            output.push_str(&item_inline);
            continue;
        }
        output.push('\n');
        output.push_str(&" ".repeat(child_indent));
        output.push_str(&format_lisp_layout(item, child_indent));
    }
    output.push('\n');
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
}

fn prefers_lisp_block(items: &[Node]) -> bool {
    let Some(head) = items.first().and_then(Node::as_symbol) else {
        return false;
    };
    let has_form_child = items[1..]
        .iter()
        .any(|item| matches!(item.kind, NodeKind::Form(_)));
    has_form_child
        && (ui_element(head).is_some()
            || matches!(
                head,
                "component" | "def" | "defn" | "export" | "type" | "ui.app"
            ))
}

fn format_lisp_object(items: &[Node], indent: usize) -> String {
    let child_indent = indent + 2;
    let mut output = "(obj".to_owned();
    for pair in items[1..].chunks_exact(2) {
        let key = format_lisp_inline(&pair[0]);
        let value_inline = format_lisp_inline(&pair[1]);
        output.push('\n');
        output.push_str(&" ".repeat(child_indent));
        if child_indent + key.chars().count() + 1 + value_inline.chars().count() <= FORMAT_WIDTH {
            output.push_str(&key);
            output.push(' ');
            output.push_str(&value_inline);
        } else {
            output.push_str(&key);
            output.push('\n');
            output.push_str(&" ".repeat(child_indent));
            output.push_str(&format_lisp_layout(&pair[1], child_indent));
        }
    }
    output.push('\n');
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
}

fn is_reader_form(items: &[Node]) -> bool {
    matches!(
        items.first().and_then(Node::as_symbol),
        Some("`" | "," | ",@")
    ) && items.len() == 2
}

fn format_lisp_inline(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => serde_json::to_string(value.as_ref()).expect("valid string"),
        NodeKind::Form(items) if is_reader_form(items) => {
            format!(
                "{}{}",
                items[0].as_symbol().expect("quoted form symbol"),
                format_lisp_inline(&items[1])
            )
        }
        NodeKind::Form(items) => format!(
            "({})",
            items
                .iter()
                .map(format_lisp_inline)
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn format_yaml_module(nodes: &[Node]) -> String {
    format!("{}\n", format_yaml_items(nodes))
}

fn format_yaml_items(items: &[Node]) -> String {
    let inline = format!(
        "[{}]",
        items
            .iter()
            .map(format_yaml_inline)
            .collect::<Vec<_>>()
            .join(", ")
    );
    if inline.chars().count() <= FORMAT_WIDTH {
        return inline;
    }
    let mut lines = vec!["[".to_owned()];
    for item in items {
        lines.push(format!(
            "  {},",
            indent_yaml_block(&format_yaml_node(item), 2)
        ));
    }
    lines.push("]".to_owned());
    lines.join("\n")
}

fn format_yaml_node(node: &Node) -> String {
    let inline = format_yaml_inline(node);
    if inline.chars().count() <= FORMAT_WIDTH || !matches!(node.kind, NodeKind::Form(_)) {
        return inline;
    }
    let NodeKind::Form(items) = &node.kind else {
        return inline;
    };
    format_yaml_items(items)
}

fn format_yaml_inline(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => match value.as_str() {
            "`" => "quasiquote".to_owned(),
            "," => "unquote".to_owned(),
            ",@" => "unquote-splicing".to_owned(),
            value => value.to_owned(),
        },
        NodeKind::String(value) => serde_json::to_string(value.as_ref()).expect("valid string"),
        NodeKind::Form(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_yaml_inline)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn indent_yaml_block(text: &str, indent: usize) -> String {
    let continuation = format!("\n{}", " ".repeat(indent));
    text.replace('\n', &continuation)
}

fn format_ws_module(nodes: &[Node]) -> String {
    format!(
        "{}\n",
        nodes
            .iter()
            .map(|node| format_ws_layout(node, 0))
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}

fn format_ws_layout(node: &Node, indent: usize) -> String {
    if !matches!(node.kind, NodeKind::Form(_)) || is_reader_node(node) {
        return format!("{}{}", " ".repeat(indent), format_ws_inline(node));
    }
    let NodeKind::Form(items) = &node.kind else {
        unreachable!("checked form node")
    };
    if items.len() <= 1 {
        return format!("{}{}", " ".repeat(indent), format_lisp_inline(node));
    }

    let head = &items[0];
    let prefix = " ".repeat(indent);
    if let Some(head_name) = head.as_symbol() {
        if head_name == "obj" {
            if let Some(layout) = format_ws_object(items, indent) {
                return layout;
            }
        }
    } else {
        let mut lines = vec![format!("{prefix}{}", format_ws_inline(head))];
        append_ws_children(&mut lines, &items[1..], indent);
        return lines.join("\n");
    }

    let head_name = head.as_symbol().expect("symbol head");
    let mut inline = vec![format_ws_inline(head)];
    let mut inline_forms = 0;
    let mut index = 1;
    let mut broke_on_width = false;
    while index < items.len() {
        let item = &items[index];
        if !can_ws_inline(item) {
            break;
        }
        if is_ws_form_argument(item) {
            if !allows_ws_inline_form(head_name, inline_forms) {
                break;
            }
            inline_forms += 1;
        }
        let rendered = format_ws_inline(item);
        let candidate = format!("{} {rendered}", inline.join(" "));
        if prefix.chars().count() + candidate.chars().count() > FORMAT_WIDTH {
            broke_on_width = true;
            break;
        }
        inline.push(rendered);
        index += 1;
    }
    if broke_on_width && items[1..].iter().all(is_ws_scalar) {
        inline.truncate(1);
        index = 1;
    }
    let mut lines = vec![format!("{prefix}{}", inline.join(" "))];
    append_ws_children(&mut lines, &items[index..], indent);
    lines.join("\n")
}

fn is_ws_scalar(node: &Node) -> bool {
    !matches!(node.kind, NodeKind::Form(_)) || is_reader_node(node)
}

fn is_reader_node(node: &Node) -> bool {
    matches!(&node.kind, NodeKind::Form(items) if is_reader_form(items))
}

fn is_ws_form_argument(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Form(_)) && !is_reader_node(node)
}

fn allows_ws_inline_form(head: &str, count: usize) -> bool {
    matches!(head, "fn" | "defn" | "let" | "component") && count == 0
}

fn can_ws_inline(node: &Node) -> bool {
    is_ws_scalar(node) || (ws_node_count(node) <= 8 && ws_form_depth(node) <= 3)
}

fn ws_node_count(node: &Node) -> usize {
    match &node.kind {
        NodeKind::Form(items) => 1 + items.iter().map(ws_node_count).sum::<usize>(),
        _ => 1,
    }
}

fn ws_form_depth(node: &Node) -> usize {
    match &node.kind {
        NodeKind::Form(items) => 1 + items.iter().map(ws_form_depth).max().unwrap_or_default(),
        _ => 0,
    }
}

fn append_ws_children(lines: &mut Vec<String>, items: &[Node], indent: usize) {
    let child_indent = indent + 2;
    let mut index = 0;
    while index < items.len() {
        if is_ws_scalar(&items[index]) {
            let start = index;
            while index < items.len() && is_ws_scalar(&items[index]) {
                index += 1;
            }
            for item in &items[start..index] {
                lines.push(format!(
                    "{}{}",
                    " ".repeat(child_indent),
                    format_ws_inline(item)
                ));
            }
        } else {
            lines.push(format_ws_layout(&items[index], child_indent));
            index += 1;
        }
    }
}

fn format_ws_object(items: &[Node], indent: usize) -> Option<String> {
    if !(items.len() - 1).is_multiple_of(2) {
        return None;
    }
    let prefix = " ".repeat(indent);
    let child_prefix = " ".repeat(indent + 2);
    let mut lines = vec![format!("{prefix}obj")];
    let mut can_extend_head = true;
    for pair in items[1..].chunks_exact(2) {
        if can_ws_inline(&pair[0]) && can_ws_inline(&pair[1]) {
            let rendered = format!(
                "{} {}",
                format_ws_inline(&pair[0]),
                format_ws_inline(&pair[1])
            );
            if can_extend_head
                && prefix.chars().count() + 4 + rendered.chars().count() <= FORMAT_WIDTH
            {
                lines[0] = format!("{prefix}obj {rendered}");
                can_extend_head = false;
                continue;
            }
            if child_prefix.chars().count() + 4 + rendered.chars().count() <= FORMAT_WIDTH {
                lines.push(format!("{child_prefix}... {rendered}"));
                can_extend_head = false;
                continue;
            }
        }
        lines.push(format_ws_layout(&pair[0], indent + 2));
        lines.push(format_ws_layout(&pair[1], indent + 2));
        can_extend_head = false;
    }
    Some(lines.join("\n"))
}

fn format_ws_inline(node: &Node) -> String {
    format_lisp_inline(node)
}

fn render_module_error(error: jisp::ModuleError) -> String {
    error
        .render_diagnostics()
        .unwrap_or_else(|| error.error.to_string())
}

fn js_error(error: String) -> JsValue {
    JsValue::from_str(&error)
}

fn parse_capabilities(source: &str) -> Result<Vec<Capability>, String> {
    let JsonValue::Array(values) = serde_json::from_str(source)
        .map_err(|error| format!("effect capabilities are not JSON: {error}"))?
    else {
        return Err("effect capabilities must be a JSON array".to_owned());
    };
    values
        .into_iter()
        .map(|value| {
            let JsonValue::Object(fields) = value else {
                return Err("each effect capability must be an object".to_owned());
            };
            if fields.len() != 2 || !fields.contains_key("name") || !fields.contains_key("version")
            {
                return Err(
                    "each effect capability must contain exactly name and version".to_owned(),
                );
            }
            let name = fields
                .get("name")
                .and_then(JsonValue::as_str)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "effect capability name must be a nonempty string".to_owned())?;
            let version = fields
                .get("version")
                .and_then(JsonValue::as_u64)
                .and_then(|version| u32::try_from(version).ok())
                .filter(|version| *version > 0)
                .ok_or_else(|| "effect capability version must be a positive u32".to_owned())?;
            Ok(Capability {
                name: name.to_owned(),
                version,
            })
        })
        .collect()
}

fn parse_resource_kind(kind: &str) -> Result<ResourceKind, String> {
    match kind {
        "command" => Ok(ResourceKind::Command),
        "subscription" => Ok(ResourceKind::Subscription),
        _ => Err("effect kind must be command or subscription".to_owned()),
    }
}

fn resource_kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Command => "command",
        ResourceKind::Subscription => "subscription",
    }
}

fn parse_delivery(value: JsonValue) -> Result<Delivery, String> {
    let JsonValue::Object(mut fields) = value else {
        return Err("effect completion must be a JSON object".to_owned());
    };
    if fields.len() != 1 {
        return Err("effect completion must contain exactly ok or error".to_owned());
    }
    if let Some(value) = fields.remove("ok") {
        return Ok(Delivery::Ok(value));
    }
    let Some(JsonValue::Object(error)) = fields.remove("error") else {
        return Err("effect completion must contain exactly ok or error".to_owned());
    };
    if error.len() != 2 || !error.contains_key("code") || !error.contains_key("message") {
        return Err("effect completion error must contain exactly code and message".to_owned());
    }
    let code = error
        .get("code")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "effect completion error code must be a string".to_owned())?;
    let message = error
        .get("message")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "effect completion error message must be a string".to_owned())?;
    Ok(Delivery::Err(HostError {
        code: parse_host_error_code(code)?,
        message: message.to_owned(),
    }))
}

fn parse_host_error_code(code: &str) -> Result<HostErrorCode, String> {
    match code {
        "unsupported-capability" => Ok(HostErrorCode::UnsupportedCapability),
        "permission-denied" => Ok(HostErrorCode::PermissionDenied),
        "invalid-request" => Ok(HostErrorCode::InvalidRequest),
        "cancelled" => Ok(HostErrorCode::Cancelled),
        "host-failure" => Ok(HostErrorCode::HostFailure),
        _ => Err(format!("unsupported effect error code `{code}`")),
    }
}

fn resource_json(
    value: &Value,
    host: Option<&FakeHost>,
    kind: ResourceKind,
) -> Result<JsonValue, String> {
    let mut value = json_value(value)?;
    let Some(host) = host else {
        return Ok(value);
    };
    let fields = value
        .as_object_mut()
        .ok_or_else(|| "effect descriptor must serialize as an object".to_owned())?;
    let id = fields
        .get("id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| "effect descriptor must include string id".to_owned())?;
    let generation = host
        .active_generation(kind, &Owner::App, id)
        .ok_or_else(|| {
            format!(
                "effect host has no active {} `{id}`",
                resource_kind_name(kind)
            )
        })?;
    fields.insert(
        "generation".to_owned(),
        JsonValue::Number(Number::from(generation)),
    );
    Ok(value)
}

fn value_from_json(value: JsonValue) -> Result<Value, String> {
    match value {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(value) => Ok(Value::Bool(value)),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Value::Int(value))
            } else if let Some(value) = value.as_f64() {
                Ok(Value::Float(value))
            } else {
                Err("browser event contains an unsupported number".to_owned())
            }
        }
        JsonValue::String(value) => Ok(Value::string(value)),
        JsonValue::Array(values) => values
            .into_iter()
            .map(value_from_json)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        JsonValue::Object(values) => values
            .into_iter()
            .map(|(key, value)| value_from_json(value).map(|value| (key, value)))
            .collect::<Result<_, _>>()
            .map(Value::Obj),
    }
}

fn json_value(value: &Value) -> Result<JsonValue, String> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::Int(value) => Ok(JsonValue::Number(Number::from(*value))),
        Value::BigInt(value) => Ok(JsonValue::String(value.to_string())),
        Value::Float(value) => Number::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| "UI data cannot contain NaN or infinity".to_owned()),
        Value::Str(value) => Ok(JsonValue::String(value.to_string())),
        Value::List(values) => values
            .iter()
            .map(json_value)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        Value::Obj(values) => values
            .iter()
            .map(|(key, value)| json_value(value).map(|value| (key.clone(), value)))
            .collect::<Result<Map<_, _>, _>>()
            .map(JsonValue::Object),
        Value::Variant { tag, fields } => Ok(
            json!({ "tag": tag, "fields": fields.iter().map(json_value).collect::<Result<Vec<_>, _>>()? }),
        ),
        Value::Builtin(_) | Value::Closure(_) | Value::Constructor(_) | Value::Uninitialized(_) => {
            Err(format!("UI data cannot serialize a {}", value.type_name()))
        }
    }
}

fn ui_node(value: &Value, handlers: &mut Vec<Value>) -> Result<JsonValue, String> {
    let Value::Obj(fields) = value else {
        return Err(format!(
            "ui.app view must return a UI node, got {}",
            value.type_name()
        ));
    };
    let tag = field_string(fields, "tag")?;
    if tag == "text" {
        return Ok(json!({ "kind": "text", "value": json_value(field(fields, "value")?)? }));
    }
    let attrs = object_json(fields.get("attrs"))?;
    let props = object_json(fields.get("props"))?;
    let classes = classes_json(fields.get("classes"))?;
    let events = event_json(fields.get("events"), handlers)?;
    let children = children_json(fields.get("children"), handlers)?;
    let key = key_json(fields.get("key"))?;
    Ok(json!({
        "kind": "element",
        "tag": tag,
        "attrs": attrs,
        "props": props,
        "classes": classes,
        "events": events,
        "key": key,
        "children": children,
    }))
}

fn collect_tree_patches(
    before: &JsonValue,
    after: &JsonValue,
    path: &str,
    patches: &mut Vec<JsonValue>,
) {
    if before == after {
        return;
    }
    let before_kind = before.get("kind").and_then(JsonValue::as_str);
    let after_kind = after.get("kind").and_then(JsonValue::as_str);
    if before_kind == Some("text") && after_kind == Some("text") {
        patches.push(json!({
            "op": "text",
            "path": path,
            "value": after.get("value").cloned().unwrap_or(JsonValue::Null),
        }));
        return;
    }

    let same_element = before_kind == Some("element")
        && after_kind == Some("element")
        && before.get("tag") == after.get("tag")
        && before.get("key") == after.get("key");
    if !same_element {
        patches.push(json!({ "op": "replace", "path": path, "tree": after }));
        return;
    }

    let mut metadata = Map::new();
    metadata.insert("op".to_owned(), JsonValue::String("element".to_owned()));
    metadata.insert("path".to_owned(), JsonValue::String(path.to_owned()));
    for field in ["attrs", "props", "classes", "events"] {
        if before.get(field) != after.get(field) {
            metadata.insert(
                field.to_owned(),
                after.get(field).cloned().unwrap_or(JsonValue::Null),
            );
        }
    }
    if metadata.len() > 2 {
        patches.push(JsonValue::Object(metadata));
    }

    let before_children = before
        .get("children")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let after_children = after
        .get("children")
        .and_then(JsonValue::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if !children_have_matching_identities(before_children, after_children) {
        patches.push(json!({ "op": "children", "path": path, "trees": after_children }));
        return;
    }
    for (index, (before, after)) in before_children.iter().zip(after_children).enumerate() {
        collect_tree_patches(before, after, &format!("{path}.{index}"), patches);
    }
}

fn children_have_matching_identities(before: &[JsonValue], after: &[JsonValue]) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .zip(after)
            .all(|(before, after)| child_identity(before) == child_identity(after))
}

fn child_identity(tree: &JsonValue) -> (&str, Option<&JsonValue>, Option<&str>) {
    (
        tree.get("kind")
            .and_then(JsonValue::as_str)
            .unwrap_or("invalid"),
        tree.get("key"),
        tree.get("tag").and_then(JsonValue::as_str),
    )
}

fn field<'a>(
    fields: &'a indexmap::IndexMap<String, Value>,
    name: &str,
) -> Result<&'a Value, String> {
    fields
        .get(name)
        .ok_or_else(|| format!("UI node is missing `{name}`"))
}

fn field_string(fields: &indexmap::IndexMap<String, Value>, name: &str) -> Result<String, String> {
    let Value::Str(value) = field(fields, name)? else {
        return Err(format!("UI node `{name}` must be a string"));
    };
    Ok(value.to_string())
}

fn object_json(value: Option<&Value>) -> Result<JsonValue, String> {
    match value {
        None => Ok(JsonValue::Object(Map::new())),
        Some(Value::Obj(values)) => values
            .iter()
            .map(|(key, value)| json_value(value).map(|value| (key.clone(), value)))
            .collect::<Result<Map<_, _>, _>>()
            .map(JsonValue::Object),
        Some(value) => Err(format!(
            "UI metadata must be an object, got {}",
            value.type_name()
        )),
    }
}

fn classes_json(value: Option<&Value>) -> Result<JsonValue, String> {
    let Some(value) = value else {
        return Ok(JsonValue::Array(vec![]));
    };
    let Value::Obj(classes) = value else {
        return Err(format!(
            "UI classes must be an object, got {}",
            value.type_name()
        ));
    };
    let classes = classes
        .iter()
        .filter_map(|(name, enabled)| enabled.truthy().then_some(JsonValue::String(name.clone())))
        .collect::<Vec<_>>();
    Ok(JsonValue::Array(classes))
}

fn key_json(value: Option<&Value>) -> Result<JsonValue, String> {
    let Some(value) = value else {
        return Ok(JsonValue::Null);
    };
    let value = json_value(value)?;
    if value.is_string() || value.is_number() || value.is_boolean() {
        Ok(value)
    } else {
        Err("UI key must be a string, number, or bool".to_owned())
    }
}

fn event_json(value: Option<&Value>, handlers: &mut Vec<Value>) -> Result<JsonValue, String> {
    let Some(Value::Obj(events)) = value else {
        return object_json(value);
    };
    let mut result = Map::new();
    for (name, descriptor) in events {
        let (handler, policy) = event_handler_and_policy(descriptor)?;
        if !matches!(
            handler,
            Value::Builtin(_) | Value::Closure(_) | Value::Constructor(_)
        ) {
            return Err(format!("UI event `{name}` must be a function"));
        }
        let id = handlers.len();
        handlers.push(handler.clone());
        result.insert(
            name.clone(),
            json!({
                "handler": id,
                "policy": policy,
            }),
        );
    }
    Ok(JsonValue::Object(result))
}

fn event_handler_and_policy(value: &Value) -> Result<(&Value, JsonValue), String> {
    let default = json!({
        "preventDefault": false,
        "stopPropagation": false,
        "capture": false,
    });
    let Value::Obj(descriptor) = value else {
        return Ok((value, default));
    };
    let Some(handler) = descriptor.get("handler") else {
        return Ok((value, default));
    };
    let policy = descriptor
        .get("policy")
        .ok_or_else(|| "UI event descriptor is missing `policy`".to_owned())?;
    let Value::Obj(policy) = policy else {
        return Err("UI event policy must be an object".to_owned());
    };
    let flag = |name: &str| match policy.get(name) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(value) => Err(format!(
            "UI event policy `{name}` must be bool, got {}",
            value.type_name()
        )),
        None => Err(format!("UI event policy is missing `{name}`")),
    };
    Ok((
        handler,
        json!({
            "preventDefault": flag("prevent-default")?,
            "stopPropagation": flag("stop-propagation")?,
            "capture": flag("capture")?,
        }),
    ))
}

fn children_json(value: Option<&Value>, handlers: &mut Vec<Value>) -> Result<JsonValue, String> {
    let Some(value) = value else {
        return Ok(JsonValue::Array(vec![]));
    };
    let mut children = vec![];
    append_children(value, handlers, &mut children)?;
    validate_child_keys(&children)?;
    Ok(JsonValue::Array(children))
}

fn validate_child_keys(children: &[JsonValue]) -> Result<(), String> {
    let mut keys = std::collections::BTreeSet::new();
    for child in children {
        let Some(key) = child.get("key").filter(|key| !key.is_null()) else {
            continue;
        };
        let key = serde_json::to_string(key).expect("JSON key serializes");
        if !keys.insert(key.clone()) {
            return Err(format!("duplicate UI key `{key}` among sibling children"));
        }
    }
    Ok(())
}

fn append_children(
    value: &Value,
    handlers: &mut Vec<Value>,
    children: &mut Vec<JsonValue>,
) -> Result<(), String> {
    match value {
        Value::Null => Ok(()),
        Value::List(values) => {
            for value in values {
                append_children(value, handlers, children)?;
            }
            Ok(())
        }
        value => {
            children.push(ui_node(value, handlers)?);
            Ok(())
        }
    }
}

pub(crate) fn render_html_source(source: &str) -> Result<String, String> {
    let value = jisp::run_main("playground.lisp", source).map_err(|error| error.to_string())?;
    match value {
        jisp::jisp_eval::Value::Str(html) => Ok(html.to_string()),
        other => Err(format!(
            "playground main must return HTML text through ui.html, got {}",
            other.type_name()
        )),
    }
}

#[cfg(test)]
mod lib_test;

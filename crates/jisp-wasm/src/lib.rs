//! Browser-facing WebAssembly entry points for the interpreter-backed playground.

use jisp::jisp_core::{Node, NodeKind, SourceId, Span, SyntaxParser};
use jisp::jisp_eval::{Evaluator, Value};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_yaml::YamlParser;
use serde_json::{json, Map, Number, Value as JsonValue};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_html(source: &str) -> Result<String, JsValue> {
    render_html_source(source).map_err(|error| JsValue::from_str(&error))
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
    view: Value,
    handlers: Vec<Value>,
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
}

impl PlaygroundSession {
    fn load_source(&mut self, source: &str) -> Result<String, String> {
        self.load_source_syntax(source, "lisp")
    }

    fn load_source_syntax(&mut self, source: &str, syntax: &str) -> Result<String, String> {
        let extension = syntax_extension(syntax)?;
        let name = format!("playground.{extension}");
        let parsed = jisp::check_detailed(&name, source).map_err(render_module_error)?;
        if !parsed.module.imports.is_empty() {
            return Err("Playground ui.app programs cannot import local files yet".to_owned());
        }
        let app = parsed.module.ui_app.clone().ok_or_else(|| {
            "Playground source must declare `(ui.app init update app)`".to_owned()
        })?;
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
        let view = loaded
            .env
            .lookup(&app.app)
            .map_err(|error| error.to_string())?;

        self.runtime = Some(Runtime {
            evaluator,
            state,
            update,
            view,
            handlers: vec![],
            span: app.span,
        });
        self.render()
    }

    fn dispatch_event(&mut self, handler: usize, event_json: &str) -> Result<String, String> {
        let event = serde_json::from_str(event_json)
            .map_err(|error| format!("browser event is not JSON: {error}"))?;
        let event = value_from_json(event)?;
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| "load a ui.app program before dispatching events".to_owned())?;
        let handler = runtime
            .handlers
            .get(handler)
            .cloned()
            .ok_or_else(|| format!("unknown UI event handler {handler}"))?;
        let action = runtime
            .evaluator
            .apply(handler, &[event], runtime.span)
            .map_err(|error| error.to_string())?;
        runtime.state = runtime
            .evaluator
            .apply(
                runtime.update.clone(),
                &[runtime.state.clone(), action],
                runtime.span,
            )
            .map_err(|error| error.to_string())?;
        self.render()
    }

    fn render(&mut self) -> Result<String, String> {
        let runtime = self
            .runtime
            .as_mut()
            .ok_or_else(|| "load a ui.app program before rendering".to_owned())?;
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
        serde_json::to_string(&tree).map_err(|error| error.to_string())
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
        _ => Err(format!("unsupported playground syntax `{syntax}`")),
    }
}

fn parse_source(source: &str, syntax: &str) -> Result<Vec<Node>, String> {
    let parsed = match syntax_extension(syntax)? {
        "lisp" => LispParser.parse_module(SourceId(0), source),
        "json" => JsonParser.parse_module(SourceId(0), source),
        "yaml" => YamlParser.parse_module(SourceId(0), source),
        _ => unreachable!("syntax_extension returns a closed set"),
    };
    parsed.map_err(|error| error.to_string())
}

fn format_source(nodes: &[Node], syntax: &str) -> Result<String, String> {
    match syntax_extension(syntax)? {
        "lisp" => Ok(format_lisp_module(nodes)),
        "json" => serde_json::to_string_pretty(&serde_json::Value::Array(
            nodes.iter().map(json_node).collect(),
        ))
        .map(|text| format!("{text}\n"))
        .map_err(|error| error.to_string()),
        "yaml" => Ok(format!(
            "[{}]\n",
            nodes
                .iter()
                .map(format_yaml_node)
                .collect::<Vec<_>>()
                .join(", ")
        )),
        _ => unreachable!("syntax_extension returns a closed set"),
    }
}

fn json_node(node: &Node) -> serde_json::Value {
    match &node.kind {
        NodeKind::Null => serde_json::Value::Null,
        NodeKind::Bool(value) => serde_json::Value::Bool(*value),
        NodeKind::Int(value) => serde_json::json!(value),
        NodeKind::Float(value) => serde_json::json!(value),
        NodeKind::Symbol(value) => serde_json::json!(value.as_str()),
        NodeKind::String(value) => serde_json::json!(["str", value]),
        NodeKind::Form(items) => {
            let string_template = matches!(
                items.first().and_then(Node::as_symbol),
                Some("str" | "str.lines")
            );
            serde_json::Value::Array(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        if string_template && index > 0 {
                            if let NodeKind::String(value) = &item.kind {
                                return serde_json::json!(value);
                            }
                        }
                        json_node(item)
                    })
                    .collect(),
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
    if indent + inline.chars().count() <= FORMAT_WIDTH {
        return inline;
    }
    let NodeKind::Form(items) = &node.kind else {
        return inline;
    };
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
        if !matches!(item.kind, NodeKind::Form(_))
            && current_width + 1 + item_inline.chars().count() <= FORMAT_WIDTH
        {
            output.push(' ');
            output.push_str(&item_inline);
            continue;
        }
        output.push('\n');
        output.push_str(&" ".repeat(child_indent));
        output.push_str(&indent_lisp_block(
            &format_lisp_layout(item, child_indent),
            child_indent,
        ));
    }
    output.push('\n');
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
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
            output.push_str(&indent_lisp_block(
                &format_lisp_layout(&pair[1], child_indent),
                child_indent,
            ));
        }
    }
    output.push('\n');
    output.push_str(&" ".repeat(indent));
    output.push(')');
    output
}

fn indent_lisp_block(text: &str, indent: usize) -> String {
    let continuation = format!("\n{}", " ".repeat(indent));
    text.replace('\n', &continuation)
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

fn format_yaml_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => serde_json::to_string(value.as_ref()).expect("valid string"),
        NodeKind::Form(items) => format!(
            "[{}]",
            items
                .iter()
                .map(format_yaml_node)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_module_error(error: jisp::ModuleError) -> String {
    error
        .render_diagnostics()
        .unwrap_or_else(|| error.error.to_string())
}

fn js_error(error: String) -> JsValue {
    JsValue::from_str(&error)
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
    Ok(json!({
        "kind": "element",
        "tag": tag,
        "attrs": attrs,
        "props": props,
        "classes": classes,
        "events": events,
        "children": children,
    }))
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

fn event_json(value: Option<&Value>, handlers: &mut Vec<Value>) -> Result<JsonValue, String> {
    let Some(Value::Obj(events)) = value else {
        return object_json(value);
    };
    let mut result = Map::new();
    for (name, handler) in events {
        if !matches!(
            handler,
            Value::Builtin(_) | Value::Closure(_) | Value::Constructor(_)
        ) {
            return Err(format!("UI event `{name}` must be a function"));
        }
        let id = handlers.len();
        handlers.push(handler.clone());
        result.insert(name.clone(), JsonValue::Number(Number::from(id)));
    }
    Ok(JsonValue::Object(result))
}

fn children_json(value: Option<&Value>, handlers: &mut Vec<Value>) -> Result<JsonValue, String> {
    let Some(value) = value else {
        return Ok(JsonValue::Array(vec![]));
    };
    let mut children = vec![];
    append_children(value, handlers, &mut children)?;
    Ok(JsonValue::Array(children))
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

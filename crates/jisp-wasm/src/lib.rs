//! Browser-facing WebAssembly entry points for the interpreter-backed playground.

use jisp::jisp_core::{ui_element, Node, NodeKind, SourceId, Span, SyntaxParser};
#[cfg(feature = "juir")]
use jisp::jisp_eval::Env;
use jisp::jisp_eval::{Evaluator, Value};
#[cfg(feature = "juir")]
use jisp::jisp_types::Inferencer;
#[cfg(feature = "juir")]
use jisp::jisp_ui::{compile as compile_juir, execute as execute_juir, Program as JuirProgram};
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
        #[cfg(feature = "juir")]
        let vnode = execute_juir(
            &runtime.program,
            &mut runtime.evaluator,
            &runtime.module_env,
            &runtime.component,
            std::slice::from_ref(&runtime.state),
        )
        .map_err(|error| error.to_string())?;
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
    if inline.chars().count() <= FORMAT_WIDTH {
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
    if (indent + inline.chars().count() <= FORMAT_WIDTH && !prefers_json_block(node))
        || !matches!(node.kind, NodeKind::Form(_))
    {
        return inline;
    }
    let NodeKind::Form(items) = &node.kind else {
        return inline;
    };
    format_json_form(items, indent)
}

fn prefers_json_block(node: &Node) -> bool {
    let NodeKind::Form(items) = &node.kind else {
        return false;
    };
    let Some(head) = items.first().and_then(Node::as_symbol) else {
        return false;
    };
    items[1..]
        .iter()
        .any(|item| matches!(item.kind, NodeKind::Form(_)))
        && matches!(
            head,
            "component" | "def" | "defn" | "export" | "type" | "ui.app"
        )
}

fn format_json_form(items: &[Node], indent: usize) -> String {
    if items.is_empty() {
        return "[]".to_owned();
    }
    let child_indent = indent + 2;
    let mut output = String::from("[");
    let mut line_width = indent + 1;
    let mut multiline = false;

    for (index, item) in items.iter().enumerate() {
        let rendered = format_json_layout(item, child_indent);
        let is_block = is_json_block(item);
        let separator_width = usize::from(index > 0) * 2;
        if !is_block && line_width + separator_width + rendered.chars().count() <= FORMAT_WIDTH {
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
    }

    if multiline {
        output.push('\n');
        output.push_str(&" ".repeat(indent));
    }
    output.push(']');
    output
}

fn is_json_block(node: &Node) -> bool {
    let NodeKind::Form(items) = &node.kind else {
        return false;
    };
    !items.is_empty() && !matches!(items.first().and_then(Node::as_symbol), Some("str"))
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
            let string_template = matches!(
                items.first().and_then(Node::as_symbol),
                Some("str" | "str.lines")
            );
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

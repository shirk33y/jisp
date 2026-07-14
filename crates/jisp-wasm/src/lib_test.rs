use super::{format_source, parse_source, render_html_source, PlaygroundSession};
use serde_json::Value;

#[test]
fn renders_a_component_program_with_the_real_interpreter() {
    let html = render_html_source(
        r#"
(component todo-row (title)
  (li (class "rounded") (text title)))

(component app ()
  (ul
    (for title (list "Plan" "Ship")
      (todo-row title))))

(export main
  (fn ()
    (ui.html (app))))
"#,
    )
    .unwrap();

    assert_eq!(
        html,
        "<ul><li class=\"rounded\">Plan</li><li class=\"rounded\">Ship</li></ul>"
    );
}

#[test]
fn update_session_rebuilds_the_view_after_an_emitted_action() {
    let mut session = PlaygroundSession::new();
    let first: Value = serde_json::from_str(
        &session
            .load_source(
                r#"
(def init (obj "count" 0))

(defn update (state next-count)
  (obj.set state "count" next-count))

(component counter (state)
  (button
    (on click (emit (+ (. state "count") 1)))
    (text (str "Count: " ,(str.from (. state "count"))))))

(ui.app init update counter)
"#,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["children"][0]["value"], "Count: 0");
    assert_eq!(first["events"]["click"], 0);

    let second: Value =
        serde_json::from_str(&session.dispatch_event(0, r#"{"type":"click"}"#).unwrap()).unwrap();
    assert_eq!(second["children"][0]["value"], "Count: 1");
}

#[test]
fn update_todo_example_type_checks_and_renders() {
    let mut session = PlaygroundSession::new();
    let tree: Value = serde_json::from_str(
        &session
            .load_source(include_str!("../../../playground/examples/todos.lisp"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(tree["kind"], "element");
    assert_eq!(tree["tag"], "main");
}

#[test]
fn every_update_playground_example_type_checks_and_renders() {
    let examples = [
        (
            "todo list",
            include_str!("../../../playground/examples/todos.lisp"),
        ),
        (
            "product launch board",
            include_str!("../../../playground/examples/kanban.lisp"),
        ),
        (
            "habit tracker",
            include_str!("../../../playground/examples/habits.lisp"),
        ),
        (
            "personal spend",
            include_str!("../../../playground/examples/finance.lisp"),
        ),
    ];

    for (name, source) in examples {
        let mut session = PlaygroundSession::new();
        let tree: Value =
            serde_json::from_str(&session.load_source(source).unwrap_or_else(|error| {
                panic!("update playground example `{name}` failed: {error}")
            }))
            .unwrap();
        assert_eq!(tree["kind"], "element", "{name}");
        assert_ui_tree_shape(&tree, name);
    }
}

#[test]
fn syntax_conversion_preserves_a_ui_module() {
    let source = include_str!("../../../playground/examples/todos.lisp");
    let nodes = parse_source(source, "lisp").unwrap();
    let json = format_source(&nodes, "json").unwrap();
    let yaml = format_source(&nodes, "yaml").unwrap();
    let lisp = format_source(&parse_source(&json, "json").unwrap(), "lisp").unwrap();
    let ws = format_source(&parse_source(&json, "json").unwrap(), "ws").unwrap();

    let mut json_session = PlaygroundSession::new();
    let mut yaml_session = PlaygroundSession::new();
    let mut lisp_session = PlaygroundSession::new();
    let mut ws_session = PlaygroundSession::new();
    assert!(json_session.load_source_syntax(&json, "json").is_ok());
    assert!(yaml_session.load_source_syntax(&yaml, "yaml").is_ok());
    assert!(lisp.contains("\n  ("));
    assert!(yaml.contains("\n  ["));
    assert!(ws.contains("\n  "));
    assert!(lisp_session.load_source_syntax(&lisp, "lisp").is_ok());
    ws_session
        .load_source_syntax(&ws, "ws")
        .unwrap_or_else(|error| panic!("formatted WS did not load: {error}\n\n{ws}"));
}

#[test]
fn update_todo_example_updates_draft_then_adds_a_task() {
    let mut session = PlaygroundSession::new();
    let first: Value = serde_json::from_str(
        &session
            .load_source(include_str!("../../../playground/examples/todos.lisp"))
            .unwrap(),
    )
    .unwrap();
    let input = handler_for(&first, "input").expect("todo input handler");
    let draft: Value = serde_json::from_str(
        &session
            .dispatch_event(
                usize::try_from(input).unwrap(),
                r#"{"type":"input","value":"Review docs"}"#,
            )
            .unwrap(),
    )
    .unwrap();
    assert!(contains_prop_value(&draft, "value", "Review docs"));

    let add = handler_for(&draft, "click").expect("add button handler");
    let added: Value = serde_json::from_str(
        &session
            .dispatch_event(usize::try_from(add).unwrap(), r#"{"type":"click"}"#)
            .unwrap(),
    )
    .unwrap();
    assert!(contains_text(&added, "Review docs"));
}

#[test]
fn update_session_serializes_scalar_keys_for_reconciliation() {
    let mut session = PlaygroundSession::new();
    let tree: Value = serde_json::from_str(
        &session
            .load_source(
                r#"
(def init null)
(defn update (state action) state)

(component app (state)
  (ul
    (li (key "first") (text "First"))
    (li (key 2) (text "Second"))))

(ui.app init update app)
"#,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(tree["children"][0]["key"], "first");
    assert_eq!(tree["children"][1]["key"], 2);
}

#[test]
fn update_session_rejects_duplicate_or_structural_sibling_keys() {
    let duplicate = r#"
(def init null)
(defn update (state action) state)
(component app (state)
  (ul
    (li (key "same") (text "First"))
    (li (key "same") (text "Second"))))
(ui.app init update app)
"#;
    let structural = r#"
(def init null)
(defn update (state action) state)
(component app (state) (div (key (list 1)) (text "Nope")))
(ui.app init update app)
"#;

    let mut session = PlaygroundSession::new();
    assert!(session
        .load_source(duplicate)
        .unwrap_err()
        .contains("duplicate UI key"));

    let mut session = PlaygroundSession::new();
    assert!(session
        .load_source(structural)
        .unwrap_err()
        .contains("UI key must be a string, number, or bool"));
}

fn handler_for(tree: &Value, event: &str) -> Option<u64> {
    tree.get("events")
        .and_then(|events| events.get(event))
        .and_then(Value::as_u64)
        .or_else(|| {
            tree.get("children")
                .and_then(Value::as_array)
                .and_then(|children| children.iter().find_map(|child| handler_for(child, event)))
        })
}

fn contains_text(tree: &Value, text: &str) -> bool {
    tree.get("kind").and_then(Value::as_str) == Some("text")
        && tree.get("value").and_then(Value::as_str) == Some(text)
        || tree
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| children.iter().any(|child| contains_text(child, text)))
}

fn contains_prop_value(tree: &Value, property: &str, value: &str) -> bool {
    tree.get("props")
        .and_then(|props| props.get(property))
        .and_then(Value::as_str)
        == Some(value)
        || tree
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|children| {
                children
                    .iter()
                    .any(|child| contains_prop_value(child, property, value))
            })
}

fn assert_ui_tree_shape(tree: &Value, name: &str) {
    if tree["kind"] == "text" {
        return;
    }
    assert_eq!(tree["kind"], "element", "{name} must contain UI nodes");
    assert!(
        tree["classes"].is_array(),
        "{name} must serialize classes as an array"
    );
    for child in tree["children"].as_array().expect("element children") {
        assert_ui_tree_shape(child, name);
    }
}

#[test]
fn static_playground_examples_are_valid_interpreter_programs() {
    let examples = [
        (
            "welcome",
            include_str!("../../../playground/examples/welcome.lisp"),
        ),
        (
            "profile",
            include_str!("../../../playground/examples/profile.lisp"),
        ),
        (
            "notifications",
            include_str!("../../../playground/examples/notifications.lisp"),
        ),
        (
            "dashboard",
            include_str!("../../../playground/examples/dashboard.lisp"),
        ),
        (
            "settings",
            include_str!("../../../playground/examples/settings.lisp"),
        ),
        (
            "product",
            include_str!("../../../playground/examples/product.lisp"),
        ),
        (
            "navigation",
            include_str!("../../../playground/examples/navigation.lisp"),
        ),
        (
            "empty state",
            include_str!("../../../playground/examples/empty-state.lisp"),
        ),
        (
            "projects",
            include_str!("../../../playground/examples/projects.lisp"),
        ),
    ];

    for (name, source) in examples {
        let html = render_html_source(source)
            .unwrap_or_else(|error| panic!("playground example `{name}` did not render: {error}"));
        assert!(
            html.starts_with('<'),
            "playground example `{name}` returned {html:?}"
        );
    }
}

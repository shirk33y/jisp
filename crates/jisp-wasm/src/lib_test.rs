#[cfg(feature = "juir")]
use super::render_ssr;
use super::{
    collect_tree_patches, format_source, parse_source, render_html_source, run_ui_tests_source,
    source_without_ui_tests, PlaygroundSession,
};
use serde_json::{json, Value};

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
fn playground_ui_tests_run_in_wasm_and_are_removed_before_rendering() {
    let source = r#"
(type Action (Increment))
(def init 0)
(defn update (state action) (+ state 1))
(component app (state) (button (text (str.from state))))
(ui.app init update app)
(ui.test "increments"
  (assert (= "<button>0</button>" (ui.test.html)))
  (dispatch Increment)
  (assert (= 1 (ui.test.state))))
"#;
    let report: Value =
        serde_json::from_str(&run_ui_tests_source(source, "lisp").unwrap()).unwrap();
    assert_eq!(report["protocol"], "jisp-ui-test/1");
    assert_eq!(report["tests"][0]["passed"], true);
    assert_eq!(report["tests"][0]["assertions"], 2);

    let stripped = source_without_ui_tests(source, "lisp").unwrap();
    assert!(!stripped.contains("ui.test"));
    let mut session = PlaygroundSession::new();
    session.load_source(&stripped).unwrap();
}

#[test]
fn playground_ui_tests_simulate_declared_effect_completions() {
    let report: Value = serde_json::from_str(
        &run_ui_tests_source(include_str!("../../../tests/ui/effects.lisp"), "lisp").unwrap(),
    )
    .unwrap();
    assert_eq!(report["protocol"], "jisp-ui-test/1");
    assert_eq!(report["tests"].as_array().unwrap().len(), 3);
    assert!(report["tests"]
        .as_array()
        .unwrap()
        .iter()
        .all(|test| test["passed"] == true));
}

#[cfg(feature = "juir")]
#[test]
fn ssr_payload_matches_the_initial_ui_app_tree() {
    let payload: Value = serde_json::from_str(
        &render_ssr(
            r#"
(def init (obj "title" "Plan"))
(defn update (state action) state)
(component app (state)
  (main
    (h1 (text (. state "title")))
    (ul (li (key "first") (text "One")))))
(ui.app init update app)
"#,
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(payload["protocol"], "jisp-ui-ssr/1");
    assert_eq!(
        payload["html"],
        "<main data-jisp-path=\"0\"><h1 data-jisp-path=\"0.0\">Plan</h1><ul data-jisp-path=\"0.1\"><li data-jisp-path=\"0.1.0\" data-jisp-key=\"string:&quot;first&quot;\">One</li></ul></main>"
    );
    assert_eq!(payload["state"]["title"], "Plan");
    assert_eq!(payload["tree"]["tag"], "main");
}

#[cfg(feature = "juir")]
#[test]
fn ssr_rejects_hydration_marker_collisions() {
    let mut session = PlaygroundSession::new();
    session
        .load_source(
            r#"
(def init null)
(defn update (state action) state)
(component app (state)
  (div (attr "data-jisp-path" "spoofed") (text "Unsafe")))
(ui.app init update app)
"#,
        )
        .unwrap();
    let error = session.ssr_payload().unwrap_err();

    assert!(error.contains("reserved for hydration"));
}

#[test]
fn json_formatter_keeps_modules_and_nested_forms_readable() {
    let nodes = parse_source(
        r#"
(export main
  (fn ()
    (str "Hello from " "Jisp" "!")))
"#,
        "lisp",
    )
    .unwrap();

    assert_eq!(
        format_source(&nodes, "json").unwrap(),
        concat!(
            "[\n",
            "  [\"export\", \"main\",\n",
            "    [\"fn\", [], [\"str\", \"Hello from \", \"Jisp\", \"!\"]]\n",
            "  ]\n",
            "]\n"
        )
    );
}

#[test]
fn json_formatter_groups_object_fields_like_lisp_object_layouts() {
    let nodes = parse_source(
        r#"
(def profile
  (obj
    "name" "Mina"
    "role" "Designer"
    "tags" (list "ui" "wasm")))
"#,
        "lisp",
    )
    .unwrap();

    assert_eq!(
        format_source(&nodes, "json").unwrap(),
        concat!(
            "[\n",
            "  [\"def\", \"profile\",\n",
            "    [\"obj\",\n",
            "      [\"str\", \"name\"], [\"str\", \"Mina\"],\n",
            "      [\"str\", \"role\"], [\"str\", \"Designer\"],\n",
            "      [\"str\", \"tags\"], [\"list\", [\"str\", \"ui\"], [\"str\", \"wasm\"]]\n",
            "    ]\n",
            "  ]\n",
            "]\n"
        )
    );
}

#[test]
fn update_session_rebuilds_the_view_after_an_emitted_action() {
    let mut session = PlaygroundSession::new();
    let first_text = session
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
        .unwrap();
    let first: Value = serde_json::from_str(&first_text).unwrap();
    assert_eq!(first["children"][0]["value"], "Count: 0");
    assert_eq!(first["events"]["click"]["handler"], 0);
    assert_eq!(first["events"]["click"]["policy"]["preventDefault"], false);

    let second: Value =
        serde_json::from_str(&session.dispatch_event(0, r#"{"type":"click"}"#).unwrap()).unwrap();
    assert_eq!(second["children"][0]["value"], "Count: 1");
}

#[cfg(feature = "juir")]
#[test]
fn update_session_exposes_compiled_juir_source_map() {
    let mut session = PlaygroundSession::new();
    session
        .load_source(
            r#"
(def init (obj "title" "Plan"))
(defn update (state action) state)
(component app (state)
  (main
    (h1 (text (. state "title")))
    (p (text "Static"))))
(ui.app init update app)
"#,
        )
        .unwrap();

    let source_map: Value = serde_json::from_str(&session.source_map_json().unwrap()).unwrap();
    assert_eq!(source_map["protocol"], "jisp-ui-source-map/1");
    let entries = source_map["entries"].as_array().unwrap();
    assert!(entries.iter().any(|entry| {
        entry["component"] == "app" && entry["path"] == "root" && entry["kind"] == "element"
    }));
    assert!(entries.iter().any(|entry| {
        entry["component"] == "app"
            && entry["path"] == "root.children.0.children.0.value"
            && entry["kind"] == "slot"
            && entry["start"].as_u64().is_some()
            && entry["end"].as_u64().is_some()
    }));
}

#[cfg(feature = "juir")]
#[test]
fn update_session_exposes_static_mount_plan_without_expression_code() {
    let mut session = PlaygroundSession::new();
    session
        .load_source(
            r#"
(def init (obj "title" "Plan"))
(defn update (state action) state)
(component app (state)
  (main
    (class "shell")
    (h1 (text "Tasks"))
    (if (. state "title")
      (p (text (. state "title")))
      (p (text "Empty")))))
(ui.app init update app)
"#,
        )
        .unwrap();

    let plan: Value = serde_json::from_str(&session.mount_plan_json().unwrap()).unwrap();
    assert_eq!(plan["protocol"], "jisp-ui-mount-plan/1");
    assert_eq!(plan["root"]["tag"], "main");
    assert_eq!(plan["root"]["staticClasses"][0], "shell");
    assert_eq!(plan["root"]["children"][0]["tag"], "h1");
    assert_eq!(plan["root"]["children"][1]["kind"], "dynamic");
    assert!(!plan.to_string().contains("expression"));
}

#[test]
fn update_session_emits_local_tree_patches() {
    let mut session = PlaygroundSession::new();
    let first: Value = serde_json::from_str(
        &session
            .load_source(
                r#"
(def init 0)
(defn update (state action) action)
(component app (state)
  (button
    (on click (emit (+ state 1)))
    (text (str.from state))))
(ui.app init update app)
"#,
            )
            .unwrap(),
    )
    .unwrap();
    let handler = usize::try_from(first["events"]["click"]["handler"].as_u64().unwrap()).unwrap();
    let update: Value = serde_json::from_str(
        &session
            .dispatch_patch_event(handler, r#"{"type":"click"}"#)
            .unwrap(),
    )
    .unwrap();

    assert_eq!(update["patches"].as_array().unwrap().len(), 1);
    assert_eq!(update["patches"][0]["op"], "text");
    assert_eq!(update["patches"][0]["path"], "0.0");
    assert_eq!(update["patches"][0]["value"], "1");
}

#[test]
fn tree_patches_reconcile_keyed_reorders_at_the_collection_boundary() {
    let before = json!({
        "kind": "element", "tag": "ul", "key": null,
        "attrs": {}, "props": {}, "classes": [], "events": {},
        "children": [
            { "kind": "element", "tag": "li", "key": 1, "attrs": {}, "props": {}, "classes": [], "events": {}, "children": [] },
            { "kind": "element", "tag": "li", "key": 2, "attrs": {}, "props": {}, "classes": [], "events": {}, "children": [] }
        ]
    });
    let after = json!({
        "kind": "element", "tag": "ul", "key": null,
        "attrs": {}, "props": {}, "classes": [], "events": {},
        "children": [
            { "kind": "element", "tag": "li", "key": 2, "attrs": {}, "props": {}, "classes": [], "events": {}, "children": [] },
            { "kind": "element", "tag": "li", "key": 1, "attrs": {}, "props": {}, "classes": [], "events": {}, "children": [] }
        ]
    });
    let mut patches = vec![];
    collect_tree_patches(&before, &after, "0", &mut patches);

    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0]["op"], "children");
    assert_eq!(patches[0]["path"], "0");
    assert_eq!(patches[0]["trees"][0]["key"], 2);
}

#[test]
fn update_session_serializes_declared_event_policies() {
    let mut session = PlaygroundSession::new();
    let tree: Value = serde_json::from_str(
        &session
            .load_source(
                r#"
(def init 0)
(defn update (state action) action)
(component app (state)
  (button
    (on click
      (prevent-default)
      (stop-propagation)
      (capture)
      (emit 7))
    (text (str.from state))))
(ui.app init update app)
"#,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(tree["events"]["click"]["handler"], 0);
    assert_eq!(tree["events"]["click"]["policy"]["preventDefault"], true);
    assert_eq!(tree["events"]["click"]["policy"]["stopPropagation"], true);
    assert_eq!(tree["events"]["click"]["policy"]["capture"], true);
    let updated: Value =
        serde_json::from_str(&session.dispatch_event(0, r#"{"type":"click"}"#).unwrap()).unwrap();
    assert_eq!(updated["children"][0]["value"], "7");
}

#[cfg(feature = "juir")]
#[test]
fn juir_reuses_the_previous_tree_when_update_returns_the_same_state() {
    let mut session = PlaygroundSession::new();
    let first_text = session
        .load_source(
            r#"
(def init (obj "count" 0))
(defn update (state action) state)
(component app (state)
  (button
    (on click (emit null))
    (text (str.from (. state "count")))))
(ui.app init update app)
"#,
        )
        .unwrap();
    let first: Value = serde_json::from_str(&first_text).unwrap();
    let handler = usize::try_from(first["events"]["click"]["handler"].as_u64().unwrap()).unwrap();
    assert_eq!(session.runtime.as_ref().unwrap().renders, 1);
    let initial_metrics: Value = serde_json::from_str(&session.metrics_json().unwrap()).unwrap();
    assert_eq!(initial_metrics["renders"], 1);
    assert_eq!(initial_metrics["skippedRenders"], 0);
    assert_eq!(initial_metrics["lastRenderSkipped"], false);

    let second = session
        .dispatch_event(handler, r#"{"type":"click"}"#)
        .unwrap();

    assert_eq!(second, first_text);
    assert_eq!(session.runtime.as_ref().unwrap().renders, 1);
    let metrics: Value = serde_json::from_str(&session.metrics_json().unwrap()).unwrap();
    assert_eq!(metrics["renders"], 1);
    assert_eq!(metrics["skippedRenders"], 1);
    assert_eq!(metrics["lastRenderSkipped"], true);
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
fn source_conversion_uses_compact_json_and_structured_lisp_layouts() {
    let nodes = parse_source(
        r#"
(component app ()
  (div (span (text "hello"))))
"#,
        "lisp",
    )
    .unwrap();

    let json = format_source(&nodes, "json").unwrap();
    let lisp = format_source(&nodes, "lisp").unwrap();

    assert!(json.contains(r#"["str", "hello"]"#));
    assert!(!json.contains("[\n          \"str\""));
    assert!(json.contains("[\"component\", \"app\", []"));
    assert_eq!(
        lisp,
        "(component app ()\n  (div\n    (span\n      (text \"hello\")\n    )\n  )\n)\n"
    );
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

    #[cfg(feature = "juir")]
    {
        let metrics: Value = serde_json::from_str(&session.metrics_json().unwrap()).unwrap();
        assert!(
            metrics["execution"]["reusedItems"].as_u64().unwrap() >= 3,
            "adding one todo should retain the already rendered keyed rows"
        );
        assert!(metrics["execution"]["componentDecisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|decision| {
                decision["component"] == "todo-list"
                    && decision["decision"] == "executed"
                    && decision["reason"] == "opaque-dependency"
            }));
    }
}

#[test]
fn update_result_updates_state_and_exposes_declared_resources() {
    let mut session = PlaygroundSession::new();
    let tree: Value = serde_json::from_str(
        &session
            .load_source(
                r#"
(def init (obj "count" 0))

(defn update (state action)
  (ui.result
    (obj.set state "count" (+ (. state "count") 1))
    (list (ui.command "save:1" "storage.write" 1 (obj "key" "draft") true (ui.action-result "Saved" (list)) (ui.action-error "SaveFailed" (list))))
    (list (ui.subscription "clock" "timer.tick" 1 (obj "every-ms" 1000) false (ui.action-result "Tick" (list)) (ui.action-error "ClockFailed" (list))))))

(component app (state)
  (button
    (on click (emit "increment"))
    (text (str.from (. state "count")))))

(ui.app init update app)
"#,
            )
            .unwrap(),
    )
    .unwrap();
    let handler = usize::try_from(handler_for(&tree, "click").unwrap()).unwrap();
    let updated: Value = serde_json::from_str(
        &session
            .dispatch_event(handler, r#"{"type":"click"}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(updated["children"][0]["value"], "1");

    let resources: Value =
        serde_json::from_str(&session.desired_resources_json().unwrap()).unwrap();
    assert_eq!(resources["commands"][0]["id"], "save:1");
    assert_eq!(
        resources["commands"][0]["capability"]["name"],
        "storage.write"
    );
    assert_eq!(resources["subscriptions"][0]["id"], "clock");
    assert_eq!(resources["commands"][0]["on-ok"]["tag"], "Saved");
    assert_eq!(
        resources["commands"][0]["on-ok"]["fields"][0]["$jisp"],
        "result"
    );
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
        .and_then(|descriptor| descriptor.get("handler").and_then(Value::as_u64))
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

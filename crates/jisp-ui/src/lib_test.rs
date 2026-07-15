use jisp_core::{SourceId, SyntaxParser};
use jisp_eval::{Evaluator, Value};
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;
use jisp_types::{Inferencer, TypedModule};

use crate::native::{self, NativeError, NativeScalar, NativeWidgetKind};
use crate::{
    changed_paths, compile, execute, execute_incremental, execute_incremental_cached, mount_plan,
    render_static_html, ChangeSet, Dependency, DependencyPath, Node, Scalar, Slot,
};

fn typed(source: &str) -> TypedModule {
    let nodes = LispParser.parse_module(SourceId(0), source).unwrap();
    let module = Lowerer.lower_module(&nodes).unwrap();
    Inferencer::with_prelude()
        .infer_typed_module(module)
        .unwrap()
}

#[test]
fn compiles_static_template_and_escapes_html() {
    let program = compile(&typed(
        r#"
(component app ()
  (div
    (attr "data-z" "last")
    (attr "id" "app")
    (class "wide" "panel")
    (span (text "Hello <Jisp>"))))
"#,
    ))
    .unwrap();

    assert_eq!(
        render_static_html(&program, "app").unwrap(),
        r#"<div class="wide panel" data-z="last" id="app"><span>Hello &lt;Jisp&gt;</span></div>"#
    );
}

#[test]
fn retains_typed_dynamic_slots_and_event_descriptors() {
    let program = compile(&typed(
        r#"
(component app (state)
  (button
    (prop "disabled" (. state "disabled"))
    (class-if "pending" (. state "pending"))
    (on click (emit (. state "id")))
    (text (. state "label"))))
"#,
    ))
    .unwrap();
    let Node::Element(button) = &program.components["app"].root else {
        panic!("expected a button template");
    };

    let Slot::Dynamic {
        dependencies: disabled_dependencies,
        ..
    } = &button.props["disabled"]
    else {
        panic!("expected dynamic disabled property");
    };
    assert_eq!(
        disabled_dependencies,
        &[Dependency::Path {
            root: "state".to_owned(),
            fields: vec!["disabled".to_owned()],
        }]
    );
    assert!(matches!(button.classes["pending"], Slot::Dynamic { .. }));
    assert!(button.events.contains_key("click"));
    let [Node::Text(crate::Text {
        value: Slot::Dynamic { .. },
        ..
    })] = button.children.as_slice()
    else {
        panic!("expected dynamic text slot");
    };
}

#[test]
fn compiles_explicit_event_policies() {
    let program = compile(&typed(
        r#"
(component app (state)
  (button
    (on click
      (prevent-default)
      (stop-propagation)
      (capture)
      (emit (. state "id")))
    (text "Open")))
"#,
    ))
    .unwrap();
    let Node::Element(button) = &program.components["app"].root else {
        panic!("expected a button template");
    };
    let policy = &button.events["click"].policy;
    assert!(policy.prevent_default);
    assert!(policy.stop_propagation);
    assert!(policy.capture);
}

#[test]
fn compiles_for_to_a_keyed_each_block() {
    let program = compile(&typed(
        r#"
(component app (items)
  (ul
    (for item items
      (li (key item) (text item)))))
"#,
    ))
    .unwrap();
    let Node::Element(list) = &program.components["app"].root else {
        panic!("expected a list template");
    };
    let [Node::Each {
        binding,
        dependencies,
        body,
        ..
    }] = list.children.as_slice()
    else {
        panic!("expected one each block");
    };
    assert_eq!(binding, "item");
    assert_eq!(
        dependencies,
        &[Dependency::Path {
            root: "items".to_owned(),
            fields: vec![],
        }]
    );
    let Node::Element(item) = body.as_ref() else {
        panic!("expected list item template");
    };
    assert!(matches!(item.key, Some(Slot::Dynamic { .. })));
}

#[test]
fn preserves_component_boundaries() {
    let program = compile(&typed(
        r#"
(component row () (li (text "Row")))
(component app () (ul (row)))
"#,
    ))
    .unwrap();
    let Node::Element(list) = &program.components["app"].root else {
        panic!("expected a list template");
    };
    let [Node::ComponentCall {
        name,
        arguments,
        dependencies,
        ..
    }] = list.children.as_slice()
    else {
        panic!("expected a component boundary");
    };
    assert_eq!(name, "row");
    assert!(arguments.is_empty());
    assert!(dependencies.is_empty());
    assert_eq!(
        render_static_html(&program, "app").unwrap(),
        "<ul><li>Row</li></ul>"
    );
}

#[test]
fn static_renderer_rejects_dynamic_slots() {
    let program = compile(&typed(
        r#"
(component app (title) (p (text title)))
"#,
    ))
    .unwrap();

    assert!(render_static_html(&program, "app")
        .unwrap_err()
        .to_string()
        .contains("without parameters"));
}

#[test]
fn static_scalar_text_is_retained_in_the_ir() {
    let program = compile(&typed("(component app () (p (text 42)))")).unwrap();
    let Node::Element(paragraph) = &program.components["app"].root else {
        panic!("expected paragraph");
    };
    let [Node::Text(crate::Text {
        value: Slot::Static(Scalar::Int(42)),
        ..
    })] = paragraph.children.as_slice()
    else {
        panic!("expected static integer text");
    };
}

#[test]
fn compiled_program_keeps_stable_source_map_entries_for_templates_and_slots() {
    let source = r#"
(component row (title)
  (li (text title)))

(component app (state)
  (main
    (if (. state "visible")
      (row (. state "title"))
      (p (text "Hidden")))
    (input (prop value (. state "draft")))))
"#;
    let program = compile(&typed(source)).unwrap();

    assert!(program.source_map.iter().any(|entry| {
        entry.component == "app"
            && entry.path == "root"
            && entry.kind == crate::SourceMapKind::Element
    }));
    assert!(program.source_map.iter().any(|entry| {
        entry.component == "app"
            && entry.path == "root.children.0"
            && entry.kind == crate::SourceMapKind::If
    }));
    let draft = program
        .source_map
        .iter()
        .find(|entry| {
            entry.component == "app"
                && entry.path == "root.children.1.props.value"
                && entry.kind == crate::SourceMapKind::Slot
        })
        .expect("dynamic property source map entry");
    assert_eq!(draft.span.source, SourceId(0));
    assert!(source[draft.span.start..draft.span.end].contains("draft"));
    assert!(program.source_map.iter().any(|entry| {
        entry.component == "app"
            && entry.path == "root.children.0.else.children.0"
            && entry.kind == crate::SourceMapKind::Text
    }));
}

#[test]
fn mount_plan_retains_static_template_shape_and_marks_dynamic_blocks() {
    let program = compile(&typed(
        r#"
(component app (state)
  (main
    (class "shell")
    (attr "data-app" "tasks")
    (h1 (text "Tasks"))
    (if (. state "visible")
      (p (text (. state "title")))
      (p (text "Hidden")))))
"#,
    ))
    .unwrap();

    let plan = mount_plan(&program, "app").unwrap();
    assert_eq!(plan["protocol"], "jisp-ui-mount-plan/1");
    assert_eq!(plan["component"], "app");
    assert_eq!(plan["root"]["kind"], "element");
    assert_eq!(plan["root"]["tag"], "main");
    assert_eq!(plan["root"]["staticAttrs"]["data-app"], "tasks");
    assert_eq!(plan["root"]["staticClasses"][0], "shell");
    assert_eq!(plan["root"]["children"][0]["kind"], "element");
    assert_eq!(plan["root"]["children"][1]["kind"], "dynamic");
    assert_eq!(plan["root"]["children"][1]["block"], "if");
}

#[test]
fn executor_matches_the_reference_evaluator_for_components_and_each() {
    let typed = typed(
        r#"
(component row (item)
  (li
    (key (. item "id"))
    (class-if "done" (. item "done"))
    (text (. item "title"))))

(component app (state)
  (ul
    (attr "aria-label" "Tasks")
    (for item (. state "items")
      (row item))))
"#,
    );
    let program = compile(&typed).unwrap();
    let state = Value::Obj(indexmap::IndexMap::from([(
        "items".to_owned(),
        Value::List(vec![
            Value::Obj(indexmap::IndexMap::from([
                ("id".to_owned(), Value::Int(1)),
                ("title".to_owned(), Value::string("Plan")),
                ("done".to_owned(), Value::Bool(false)),
            ])),
            Value::Obj(indexmap::IndexMap::from([
                ("id".to_owned(), Value::Int(2)),
                ("title".to_owned(), Value::string("Ship")),
                ("done".to_owned(), Value::Bool(true)),
            ])),
        ]),
    )]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let span = typed.module.definitions[1].span;

    let reference = evaluator
        .apply(
            loaded.env.lookup("app").unwrap(),
            std::slice::from_ref(&state),
            span,
        )
        .unwrap();
    let rendered = execute(&program, &mut evaluator, &loaded.env, "app", &[state]).unwrap();
    assert!(
        reference.structurally_equal(&rendered).unwrap(),
        "JUIR must flatten `for` children to the reference structural tree"
    );
    let ui_html = evaluator.root_env().lookup("ui.html").unwrap();
    let reference_html = evaluator
        .apply(ui_html.clone(), &[reference], span)
        .unwrap()
        .display_string();
    let rendered_html = evaluator
        .apply(ui_html, &[rendered], span)
        .unwrap()
        .display_string();

    assert_eq!(rendered_html, reference_html);
    assert_eq!(
        rendered_html,
        r#"<ul aria-label="Tasks"><li>Plan</li><li class="done">Ship</li></ul>"#
    );
}

#[test]
fn executor_evaluates_event_handlers_in_component_scope() {
    let typed = typed(
        r#"
(component app (state)
  (button
    (on click (emit (. state "id")))
    (text (. state "label"))))
"#,
    );
    let program = compile(&typed).unwrap();
    let state = Value::Obj(indexmap::IndexMap::from([
        ("id".to_owned(), Value::Int(7)),
        ("label".to_owned(), Value::string("Open")),
    ]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let rendered = execute(&program, &mut evaluator, &loaded.env, "app", &[state]).unwrap();

    let Value::Obj(element) = rendered else {
        panic!("expected element value");
    };
    let Some(Value::Obj(events)) = element.get("events") else {
        panic!("expected event bindings");
    };
    let Some(Value::Obj(click)) = events.get("click") else {
        panic!("expected a structured click descriptor");
    };
    assert!(matches!(click.get("handler"), Some(Value::Closure(_))));
    assert!(matches!(click.get("policy"), Some(Value::Obj(_))));
}

#[test]
fn changed_paths_only_invalidate_intersecting_static_dependencies() {
    let before = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(1)),
        ("todos".to_owned(), Value::List(vec![Value::Int(1)])),
    ]));
    let after = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(2)),
        (
            "todos".to_owned(),
            Value::List(vec![Value::Int(1), Value::Int(2)]),
        ),
    ]));

    let changes = changed_paths("state", &before, &after);
    assert_eq!(
        changes.paths,
        std::collections::BTreeSet::from([
            DependencyPath {
                root: "state".to_owned(),
                fields: vec!["count".to_owned()],
            },
            DependencyPath {
                root: "state".to_owned(),
                fields: vec!["todos".to_owned()],
            },
        ])
    );
    assert!(!changes.affects(&[Dependency::Path {
        root: "state".to_owned(),
        fields: vec!["title".to_owned()],
    }]));
    assert!(changes.affects(&[Dependency::Path {
        root: "state".to_owned(),
        fields: vec!["todos".to_owned(), "done".to_owned()],
    }]));
    assert!(changes.affects(&[Dependency::Unknown]));
}

#[test]
fn incremental_executor_reuses_unaffected_slots() {
    let typed = typed(
        r#"
(component app (state)
  (div
    (text (. state "title"))
    (text (str.from (. state "count")))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(1)),
    ]));
    let after = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(2)),
    ]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let first = execute_incremental(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first.value),
        &changed_paths("state", &before, &after),
    )
    .unwrap();

    assert_eq!(second.stats.reused_slots, 1);
    assert_eq!(second.stats.evaluated_slots, 1);
    let html = evaluator
        .apply(
            evaluator.root_env().lookup("ui.html").unwrap(),
            &[second.value],
            typed.module.definitions[0].span,
        )
        .unwrap()
        .display_string();
    assert_eq!(html, "<div>Plan2</div>");
}

#[test]
fn incremental_executor_matches_full_execution_across_block_updates() {
    let typed = typed(
        r#"
(component row (item)
  (li
    (key (. item "id"))
    (text (. item "title"))))

(component app (state)
  (main
    (h1 (text (. state "title")))
    (if (. state "show")
      (div (text "Visible"))
      (div (text "Hidden")))
    (ul
      (for item (. state "items")
        (row item)))))
"#,
    );
    let program = compile(&typed).unwrap();
    let states = [
        app_state("Inbox", true, &["Plan", "Ship"]),
        app_state("Inbox", true, &["Plan", "Ship"]),
        app_state("Today", true, &["Plan", "Ship"]),
        app_state("Today", false, &["Plan", "Ship"]),
        app_state("Today", false, &["Plan", "Review"]),
    ];
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let ui_html = evaluator.root_env().lookup("ui.html").unwrap();
    let mut previous = None;

    for (index, state) in states.iter().enumerate() {
        let changes = previous
            .as_ref()
            .map(|before| changed_paths("state", before, state))
            .unwrap_or(ChangeSet {
                unknown: true,
                ..ChangeSet::default()
            });
        let incremental = execute_incremental(
            &program,
            &mut evaluator,
            &loaded.env,
            "app",
            std::slice::from_ref(state),
            previous.as_ref(),
            &changes,
        )
        .unwrap();
        let full = execute(
            &program,
            &mut evaluator,
            &loaded.env,
            "app",
            std::slice::from_ref(state),
        )
        .unwrap();
        let span = typed.module.definitions[1].span;
        let incremental_html = evaluator
            .apply(
                ui_html.clone(),
                std::slice::from_ref(&incremental.value),
                span,
            )
            .unwrap()
            .display_string();
        let full_html = evaluator
            .apply(ui_html.clone(), &[full], span)
            .unwrap()
            .display_string();

        assert_eq!(incremental_html, full_html, "state {index}");
        previous = Some(incremental.value);
    }
}

#[test]
fn cached_incremental_executor_reuses_unchanged_each_items_after_a_list_update() {
    let typed = typed(
        r#"
(component row (item)
  (li
    (key (. item "id"))
    (button (on click (emit (. item "id"))) (text (. item "title")))))

(component app (state)
  (ul
    (for item (. state "items")
      (row item))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = app_state("Inbox", true, &["Plan", "Ship"]);
    let after = app_state("Inbox", true, &["Plan", "Review"]);
    let reordered = state_with_items(&[(2, "Review"), (1, "Plan")]);
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let ui_html = evaluator.root_env().lookup("ui.html").unwrap();

    let first = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first),
        &changed_paths("state", &before, &after),
    )
    .unwrap();
    let full = execute(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
    )
    .unwrap();

    assert_eq!(second.stats.reused_items, 1);
    assert_eq!(
        evaluator
            .apply(
                ui_html.clone(),
                std::slice::from_ref(&second.value),
                typed.module.definitions[1].span
            )
            .unwrap()
            .display_string(),
        evaluator
            .apply(ui_html.clone(), &[full], typed.module.definitions[1].span)
            .unwrap()
            .display_string()
    );

    let third = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&reordered),
        Some(&second),
        &changed_paths("state", &after, &reordered),
    )
    .unwrap();
    let full_reordered = execute(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&reordered),
    )
    .unwrap();

    assert_eq!(third.stats.reused_items, 2);
    assert_eq!(
        evaluator
            .apply(
                ui_html.clone(),
                &[third.value],
                typed.module.definitions[1].span
            )
            .unwrap()
            .display_string(),
        evaluator
            .apply(ui_html, &[full_reordered], typed.module.definitions[1].span)
            .unwrap()
            .display_string()
    );
}

#[test]
fn cached_each_rows_rerender_when_an_external_body_dependency_changes() {
    let typed = typed(
        r#"
(component app (state)
  (ul
    (for item (. state "items")
      (li
        (key (. item "id"))
        (text (. state "title"))
        (text (. item "title"))))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = app_state("Inbox", true, &["Plan", "Ship"]);
    let after = app_state("Today", true, &["Plan", "Review"]);
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();

    let first = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first),
        &changed_paths("state", &before, &after),
    )
    .unwrap();
    let full = execute(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
    )
    .unwrap();
    let ui_html = evaluator.root_env().lookup("ui.html").unwrap();

    assert_eq!(second.stats.reused_items, 0);
    assert_eq!(
        evaluator
            .apply(
                ui_html.clone(),
                &[second.value],
                typed.module.definitions[0].span
            )
            .unwrap()
            .display_string(),
        evaluator
            .apply(ui_html, &[full], typed.module.definitions[0].span)
            .unwrap()
            .display_string()
    );
}

#[test]
fn cached_incremental_executor_reuses_an_unaffected_static_subtree() {
    let typed = typed(
        r#"
(component app (state)
  (main
    (section
      (h1 (text "Static heading"))
      (p (text "This subtree has no state reads.")))
    (p (text (str.from (. state "count"))))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = Value::Obj(indexmap::IndexMap::from([(
        "count".to_owned(),
        Value::Int(1),
    )]));
    let after = Value::Obj(indexmap::IndexMap::from([(
        "count".to_owned(),
        Value::Int(2),
    )]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let first = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first),
        &changed_paths("state", &before, &after),
    )
    .unwrap();
    let full = execute(&program, &mut evaluator, &loaded.env, "app", &[after]).unwrap();

    assert!(second.stats.reused_subtrees >= 1);
    assert!(second.value.structurally_equal(&full).unwrap());
}

#[test]
fn native_widget_adapter_materializes_semantic_widgets_without_dom_types() {
    let tree = Value::Obj(indexmap::IndexMap::from([
        ("tag".to_owned(), Value::string("div")),
        (
            "classes".to_owned(),
            Value::Obj(indexmap::IndexMap::from([
                ("panel".to_owned(), Value::Bool(true)),
                ("hidden".to_owned(), Value::Bool(false)),
            ])),
        ),
        (
            "children".to_owned(),
            Value::List(vec![Value::Obj(indexmap::IndexMap::from([
                ("tag".to_owned(), Value::string("input")),
                (
                    "attrs".to_owned(),
                    Value::Obj(indexmap::IndexMap::from([(
                        "placeholder".to_owned(),
                        Value::string("Task"),
                    )])),
                ),
                (
                    "props".to_owned(),
                    Value::Obj(indexmap::IndexMap::from([(
                        "value".to_owned(),
                        Value::string("Plan"),
                    )])),
                ),
                (
                    "events".to_owned(),
                    Value::Obj(indexmap::IndexMap::from([(
                        "input".to_owned(),
                        Value::Null,
                    )])),
                ),
            ]))]),
        ),
    ]));

    let native = native::render(&tree, &native::NativeRegistry::portable_baseline()).unwrap();

    assert_eq!(native.kind, NativeWidgetKind::Container);
    assert_eq!(
        native.style_tokens,
        std::collections::BTreeSet::from(["panel".to_owned()])
    );
    assert_eq!(native.children[0].kind, NativeWidgetKind::TextInput);
    assert_eq!(
        native.children[0].properties["value"],
        NativeScalar::Str("Plan".to_owned())
    );
    assert!(native.children[0].events.contains("input"));
}

#[test]
fn native_widget_adapter_accepts_flattened_dynamic_for_children() {
    let typed = typed(
        r#"
(component row (item)
  (li (key (. item "id")) (text (. item "title"))))

(component app (state)
  (ul
    (for item (. state "items")
      (row item))))
"#,
    );
    let state = Value::Obj(indexmap::IndexMap::from([(
        "items".to_owned(),
        Value::List(vec![
            Value::Obj(indexmap::IndexMap::from([
                ("id".to_owned(), Value::Int(1)),
                ("title".to_owned(), Value::string("Plan")),
            ])),
            Value::Obj(indexmap::IndexMap::from([
                ("id".to_owned(), Value::Int(2)),
                ("title".to_owned(), Value::string("Ship")),
            ])),
        ]),
    )]));
    let program = compile(&typed).unwrap();
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let tree = execute(&program, &mut evaluator, &loaded.env, "app", &[state]).unwrap();

    let native = native::render(&tree, &native::NativeRegistry::portable_baseline()).unwrap();
    assert_eq!(native.kind, NativeWidgetKind::List);
    assert_eq!(native.children.len(), 2);
    assert!(native
        .children
        .iter()
        .all(|child| child.kind == NativeWidgetKind::ListItem));
}

#[test]
fn native_widget_adapter_reports_unsupported_host_capabilities() {
    let image = Value::Obj(indexmap::IndexMap::from([(
        "tag".to_owned(),
        Value::string("img"),
    )]));

    let error = native::render(&image, &native::NativeRegistry::portable_baseline()).unwrap_err();

    assert_eq!(
        error,
        NativeError::UnsupportedElement {
            tag: "img".to_owned()
        }
    );
}

#[test]
fn compiles_a_conditional_component_root() {
    let program = compile(&typed(
        r#"
(component status (state)
  (if (. state "visible")
    (div (text "Visible"))
    (div (text "Hidden"))))
"#,
    ))
    .unwrap();

    assert!(program.components.contains_key("status"));
    let mut evaluator = Evaluator::new();
    let module = typed(
        r#"
(component status (state)
  (if (. state "visible")
    (div (text "Visible"))
    (div (text "Hidden"))))
"#,
    );
    let loaded = evaluator.load_module(&module.module).unwrap();
    let rendered = execute(
        &program,
        &mut evaluator,
        &loaded.env,
        "status",
        &[Value::Obj(indexmap::IndexMap::from([(
            "visible".to_owned(),
            Value::Bool(false),
        )]))],
    )
    .unwrap();
    let html = evaluator
        .apply(
            evaluator.root_env().lookup("ui.html").unwrap(),
            &[rendered],
            module.module.definitions[0].span,
        )
        .unwrap()
        .display_string();
    assert_eq!(html, "<div>Hidden</div>");
}

#[test]
fn incremental_executor_skips_a_component_with_unaffected_inputs() {
    let typed = typed(
        r#"
(component app-header (title)
  (header (text title)))

(component app (state)
  (main
    (app-header (. state "title"))
    (text (. state "count"))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(1)),
    ]));
    let after = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(2)),
    ]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let first = execute_incremental(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first.value),
        &changed_paths("state", &before, &after),
    )
    .unwrap();

    assert_eq!(second.stats.reused_components, 1);
    assert_eq!(second.stats.evaluated_slots, 1);
    assert_eq!(
        second.stats.component_decisions,
        vec![crate::ComponentDecision {
            component: "app-header".to_owned(),
            path: "root.child.0".to_owned(),
            outcome: crate::ComponentDecisionOutcome::Reused,
        }]
    );

    let title_changed = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Ship")),
        ("count".to_owned(), Value::Int(2)),
    ]));
    let third = execute_incremental(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&title_changed),
        Some(&second.value),
        &changed_paths("state", &after, &title_changed),
    )
    .unwrap();
    assert_eq!(
        third.stats.component_decisions,
        vec![crate::ComponentDecision {
            component: "app-header".to_owned(),
            path: "root.child.0".to_owned(),
            outcome: crate::ComponentDecisionOutcome::Executed(
                crate::ComponentExecutionReason::InputChanged
            ),
        }]
    );
}

#[test]
fn component_skip_diagnostics_name_opaque_dependencies() {
    let typed = typed(
        r#"
(component app-header (title)
  (header (text (str.from title))))

(component app (state)
  (main
    (app-header (str.from (. state "title")))
    (text (. state "count"))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(1)),
    ]));
    let after = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(2)),
    ]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let first = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first),
        &changed_paths("state", &before, &after),
    )
    .unwrap();

    assert_eq!(
        second.stats.component_decisions,
        vec![crate::ComponentDecision {
            component: "app-header".to_owned(),
            path: "root.child.0".to_owned(),
            outcome: crate::ComponentDecisionOutcome::Executed(
                crate::ComponentExecutionReason::OpaqueDependency
            ),
        }]
    );
}

#[test]
fn cached_executor_reuses_pure_component_when_opaque_input_evaluates_equal() {
    let typed = typed(
        r#"
(component app-header (title)
  (header (text title)))

(component app (state)
  (main
    (app-header (str.from (. state "title")))
    (text (. state "count"))))
"#,
    );
    let program = compile(&typed).unwrap();
    let before = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(1)),
    ]));
    let after = Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string("Plan")),
        ("count".to_owned(), Value::Int(2)),
    ]));
    let mut evaluator = Evaluator::new();
    let loaded = evaluator.load_module(&typed.module).unwrap();
    let first = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&before),
        None,
        &ChangeSet {
            unknown: true,
            ..ChangeSet::default()
        },
    )
    .unwrap();
    let second = execute_incremental_cached(
        &program,
        &mut evaluator,
        &loaded.env,
        "app",
        std::slice::from_ref(&after),
        Some(&first),
        &changed_paths("state", &before, &after),
    )
    .unwrap();
    let full = execute(&program, &mut evaluator, &loaded.env, "app", &[after]).unwrap();

    assert_eq!(second.stats.reused_components, 1);
    assert_eq!(
        second.stats.component_decisions,
        vec![crate::ComponentDecision {
            component: "app-header".to_owned(),
            path: "root.child.0".to_owned(),
            outcome: crate::ComponentDecisionOutcome::Reused,
        }]
    );
    assert!(second.value.structurally_equal(&full).unwrap());
}

fn app_state(title: &str, show: bool, items: &[&str]) -> Value {
    Value::Obj(indexmap::IndexMap::from([
        ("title".to_owned(), Value::string(title)),
        ("show".to_owned(), Value::Bool(show)),
        (
            "items".to_owned(),
            Value::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, title)| {
                        Value::Obj(indexmap::IndexMap::from([
                            ("id".to_owned(), Value::Int(index as i64 + 1)),
                            ("title".to_owned(), Value::string(*title)),
                        ]))
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn state_with_items(items: &[(i64, &str)]) -> Value {
    Value::Obj(indexmap::IndexMap::from([(
        "items".to_owned(),
        Value::List(
            items
                .iter()
                .map(|(id, title)| {
                    Value::Obj(indexmap::IndexMap::from([
                        ("id".to_owned(), Value::Int(*id)),
                        ("title".to_owned(), Value::string(*title)),
                    ]))
                })
                .collect(),
        ),
    )]))
}

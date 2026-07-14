use jisp_core::{SourceId, SyntaxParser};
use jisp_eval::{Evaluator, Value};
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;
use jisp_types::{Inferencer, TypedModule};

use crate::{compile, execute, render_static_html, Node, Scalar, Slot};

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

    assert!(matches!(button.props["disabled"], Slot::Dynamic { .. }));
    assert!(matches!(button.classes["pending"], Slot::Dynamic { .. }));
    assert!(button.events.contains_key("click"));
    let [Node::Text(Slot::Dynamic { .. })] = button.children.as_slice() else {
        panic!("expected dynamic text slot");
    };
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
    let [Node::Each { binding, body, .. }] = list.children.as_slice() else {
        panic!("expected one each block");
    };
    assert_eq!(binding, "item");
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
        name, arguments, ..
    }] = list.children.as_slice()
    else {
        panic!("expected a component boundary");
    };
    assert_eq!(name, "row");
    assert!(arguments.is_empty());
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
    let [Node::Text(Slot::Static(Scalar::Int(42)))] = paragraph.children.as_slice() else {
        panic!("expected static integer text");
    };
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
    assert!(matches!(events.get("click"), Some(Value::Closure(_))));
}

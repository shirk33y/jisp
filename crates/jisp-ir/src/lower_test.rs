use jisp_core::{SourceId, SyntaxParser};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;

use crate::{Expr, ExprKind, Literal, Lowerer};

fn lower(source: &str) -> Result<crate::Module, crate::LowerError> {
    let nodes = JsonParser.parse_module(SourceId(0), source).unwrap();
    Lowerer.lower_module(&nodes)
}

fn lower_lisp(source: &str) -> Result<crate::Module, crate::LowerError> {
    let nodes = LispParser.parse_module(SourceId(0), source).unwrap();
    Lowerer.lower_module(&nodes)
}

fn object_field<'a>(fields: &'a [(Expr, Expr)], key: &str) -> Option<&'a Expr> {
    fields
        .iter()
        .find(|(field, _)| literal_string(field) == Some(key))
        .map(|(_, value)| value)
}

fn literal_string(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Literal(Literal::String(value)) => Some(value),
        _ => None,
    }
}

#[test]
fn use_lowers_multiple_bindings_to_callback_params() {
    let module =
        lower(r#"[["def","x",["use",["left","right"],["with-pair"],["+","left","right"]]]]"#)
            .unwrap();

    let ExprKind::Call { callee, arguments } = &module.definitions[0].value.kind else {
        panic!("use should lower to a call");
    };
    assert!(matches!(callee.kind, ExprKind::Name(ref name) if name == "with-pair"));
    assert_eq!(arguments.len(), 1);

    let ExprKind::Lambda { params, rest, body } = &arguments[0].kind else {
        panic!("use should append a callback lambda");
    };
    assert_ne!(arguments[0].span, module.definitions[0].value.span);
    assert_eq!(params, &["left", "right"]);
    assert!(rest.is_none());
    assert!(matches!(body.kind, ExprKind::Call { .. }));
}

#[test]
fn use_rejects_empty_callback_target() {
    let error = lower(r#"[["def","x",["use","value",[],["ok","value"]]]]"#).unwrap_err();

    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(error.diagnostics[0].message, "use call cannot be empty");
}

#[test]
fn export_can_define_a_public_value() {
    let module = lower(r#"[["export","add",["fn",["a","b"],["+","a","b"]]]]"#).unwrap();

    assert!(module.definitions[0].public);
    assert_eq!(module.exports, ["add"]);
}

#[test]
fn defn_lowers_to_the_same_private_lambda_shape_as_def_and_fn() {
    let module = lower_lisp(
        r#"
(defn add (left right)
  (+ left right)
  (+ left right))
"#,
    )
    .unwrap();

    assert_eq!(module.definitions[0].name, "add");
    assert!(!module.definitions[0].public);
    let ExprKind::Lambda { params, rest, body } = &module.definitions[0].value.kind else {
        panic!("defn should lower to a lambda");
    };
    assert_eq!(params, &["left", "right"]);
    assert!(rest.is_none());
    assert!(matches!(body.kind, ExprKind::Do(_)));
}

#[test]
fn defn_requires_a_name_parameter_list_and_body() {
    let error = lower_lisp("(defn add (left right))").unwrap_err();
    assert_eq!(
        error.diagnostics[0].message,
        "defn expects a name, parameter list, and a body"
    );
}

#[test]
fn lower_rejects_duplicate_module_value_names() {
    let error = lower(r#"[["def","answer",1],["export","answer",2]]"#).unwrap_err();

    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(
        error.diagnostics[0].message,
        "duplicate value declaration `answer`"
    );
    assert_eq!(error.diagnostics[0].secondary.len(), 1);
}

#[test]
fn lower_rejects_duplicate_import_aliases() {
    let error =
        lower(r#"[["import","math",["str","one.lisp"]],["import","math",["str","two.lisp"]]]"#)
            .unwrap_err();

    assert_eq!(
        error.diagnostics[0].message,
        "duplicate import alias `math`"
    );
}

#[test]
fn lower_reserves_macro_import_for_future_cross_module_macros() {
    let error = lower(r#"[["macro-import","macros",["str","macros.lisp"]]]"#).unwrap_err();

    assert_eq!(
        error.diagnostics[0].message,
        "macro-import must be resolved before lowering; runtime import does not import macros"
    );
}

#[test]
fn lower_rejects_duplicate_type_constructors() {
    let error =
        lower(r#"[["type","first",["item"]],["type","second",["item","int"]]]"#).unwrap_err();

    assert_eq!(
        error.diagnostics[0].message,
        "duplicate value declaration `item`"
    );
}

#[test]
fn lower_rejects_duplicate_static_object_keys() {
    let error =
        lower(r#"[["def","value",["obj",["str","name"],1,["str","name"],2]]]"#).unwrap_err();

    assert_eq!(error.diagnostics[0].message, "duplicate object key `name`");
    assert_eq!(error.diagnostics[0].secondary.len(), 1);
}

#[test]
fn lower_rejects_duplicate_object_pattern_keys() {
    let error = lower(
        r#"[["def","value",["fn",["object"],["case","object",[["obj",["str","name"],"first",["str","name"],"second"],"first"]]]]]"#,
    )
    .unwrap_err();

    assert_eq!(
        error.diagnostics[0].message,
        "duplicate object pattern key `name`"
    );
}

#[test]
fn component_lowers_explicit_elements_directives_and_component_children() {
    let module = lower_lisp(
        r#"
(component todo-row (title)
  (li
    (attr "data-id" "7")
    (prop hidden false)
    (class "rounded" "px-2")
    (class-if "opacity-50" false)
    (on "click" (fn (_) title))
    (key title)
    (span (text title))))

(component todo-list (titles)
  (ul
    (for title titles
      (todo-row title))))
"#,
    )
    .unwrap();

    assert_eq!(module.definitions[0].name, "todo-row");
    assert!(!module.definitions[0].public);

    let ExprKind::Lambda { params, rest, body } = &module.definitions[0].value.kind else {
        panic!("component should lower to a function");
    };
    assert_eq!(params, &["title"]);
    assert!(rest.is_none());

    let ExprKind::Call { callee, arguments } = &body.kind else {
        panic!("component body should lower through ui.node");
    };
    assert!(matches!(callee.kind, ExprKind::Name(ref name) if name == "ui.node"));
    let [node] = arguments.as_slice() else {
        panic!("ui.node should receive one structural node");
    };
    let ExprKind::Object(fields) = &node.kind else {
        panic!("component body should lower to a structural UI object");
    };
    assert_eq!(
        object_field(fields, "tag").and_then(literal_string),
        Some("li")
    );

    let ExprKind::Object(attrs) = &object_field(fields, "attrs").unwrap().kind else {
        panic!("attrs should lower to a separate object");
    };
    assert_eq!(
        object_field(attrs, "data-id").and_then(literal_string),
        Some("7")
    );

    let ExprKind::Object(props) = &object_field(fields, "props").unwrap().kind else {
        panic!("props should lower to a separate object");
    };
    assert!(object_field(props, "hidden").is_some());

    let ExprKind::Object(classes) = &object_field(fields, "classes").unwrap().kind else {
        panic!("utility classes should lower to a classes object");
    };
    assert!(object_field(classes, "px-2").is_some());
    assert!(object_field(classes, "opacity-50").is_some());

    assert!(object_field(fields, "events").is_some());
    assert!(object_field(fields, "key").is_some());

    let ExprKind::List(children) = &object_field(fields, "children").unwrap().kind else {
        panic!("nested elements should lower to children");
    };
    assert_eq!(children.len(), 1);

    let ExprKind::Lambda { body, .. } = &module.definitions[1].value.kind else {
        panic!("component should lower to a function");
    };
    let ExprKind::Call { callee, arguments } = &body.kind else {
        panic!("component body should lower through ui.node");
    };
    assert!(matches!(callee.kind, ExprKind::Name(ref name) if name == "ui.node"));
    let [node] = arguments.as_slice() else {
        panic!("ui.node should receive one structural node");
    };
    let ExprKind::Object(list_fields) = &node.kind else {
        panic!("todo-list should lower to a structural UI object");
    };
    let ExprKind::Call { callee, arguments } = &object_field(list_fields, "children").unwrap().kind
    else {
        panic!("for should lower to a list mapping expression");
    };
    assert!(matches!(callee.kind, ExprKind::Name(ref name) if name == "list.map"));
    assert_eq!(arguments.len(), 2);
}

#[test]
fn component_lowers_ui_elements_in_conditional_branches() {
    let module = lower_lisp(
        r#"
(component status (visible)
  (if visible
    (div (text "Visible"))
    (div (text "Hidden"))))
"#,
    )
    .unwrap();

    let ExprKind::Lambda { body, .. } = &module.definitions[0].value.kind else {
        panic!("component should lower to a lambda");
    };
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &body.kind
    else {
        panic!("conditional UI should lower to an if expression");
    };
    for branch in [then_branch, else_branch] {
        let ExprKind::Call { callee, .. } = &branch.kind else {
            panic!("conditional branch should lower through ui.node");
        };
        assert!(matches!(callee.kind, ExprKind::Name(ref name) if name == "ui.node"));
    }
}

#[test]
fn component_lowers_event_policies_and_rejects_invalid_modifiers() {
    let module = lower_lisp(
        r#"
(component action ()
  (button
    (on click (prevent-default) (stop-propagation) (emit "ok"))
    (text "Run")))
"#,
    )
    .unwrap();
    let ExprKind::Lambda { body, .. } = &module.definitions[0].value.kind else {
        panic!("component should lower to a lambda");
    };
    let ExprKind::Call { arguments, .. } = &body.kind else {
        panic!("component should lower through ui.node");
    };
    let ExprKind::Object(fields) = &arguments[0].kind else {
        panic!("ui.node should receive a structural object");
    };
    let ExprKind::Object(events) = &object_field(fields, "events").unwrap().kind else {
        panic!("expected events metadata");
    };
    let ExprKind::Object(click) = &events[0].1.kind else {
        panic!("policy event should lower to a descriptor");
    };
    assert!(object_field(click, "handler").is_some());
    let ExprKind::Object(policy) = &object_field(click, "policy").unwrap().kind else {
        panic!("descriptor should retain policy flags");
    };
    assert!(object_field(policy, "prevent-default").is_some());
    assert!(object_field(policy, "stop-propagation").is_some());

    let error = lower_lisp(
        "(component action () (button (on click (stop-immediate-propagation) (emit null))))",
    )
    .unwrap_err();
    assert!(error.diagnostics[0]
        .message
        .contains("unknown event modifier `stop-immediate-propagation`"));
}

#[test]
fn component_rejects_reserved_element_names_and_duplicate_directives() {
    let reserved = lower_lisp("(component div () (div))").unwrap_err();
    assert_eq!(
        reserved.diagnostics[0].message,
        "component name `div` is reserved by the UI element registry"
    );

    let duplicate = lower_lisp(
        r#"
(component example ()
  (button
    (attr "aria-label" "first")
    (attr "aria-label" "second")))
"#,
    )
    .unwrap_err();
    assert_eq!(
        duplicate.diagnostics[0].message,
        "duplicate UI directive name `aria-label`"
    );
}

#[test]
fn ui_app_requires_local_bindings_and_emit_is_scoped_to_event_handlers() {
    let module = lower_lisp(
        r#"
(def init 0)
(defn update (state action) state)
(component view (state) (button (on click (emit action)) (text "ok")))
(ui.app init update view)
"#,
    )
    .unwrap();
    let app = module.ui_app.expect("ui.app metadata");
    assert_eq!(app.init, "init");
    assert_eq!(app.update, "update");
    assert_eq!(app.app, "view");

    let unknown = lower_lisp("(ui.app init update view)").unwrap_err();
    assert!(unknown.diagnostics[0]
        .message
        .contains("does not name a module definition"));

    let emit = lower_lisp("(def value (emit action))").unwrap_err();
    assert_eq!(
        emit.diagnostics[0].message,
        "UI syntax is only valid inside a component"
    );
}

#[test]
fn ui_syntax_is_scoped_to_components_and_view_is_not_an_alias() {
    let element = lower_lisp("(def root (div (text \"outside\")))").unwrap_err();
    assert_eq!(
        element.diagnostics[0].message,
        "UI element `div` is only valid inside a component"
    );

    let directive = lower_lisp("(def attribute (attr \"id\" \"root\"))").unwrap_err();
    assert_eq!(
        directive.diagnostics[0].message,
        "UI syntax is only valid inside a component"
    );

    let alias = lower_lisp("(view root () (div))").unwrap_err();
    assert!(alias.diagnostics[0]
        .message
        .contains("top-level expression `view` is not allowed"));
}

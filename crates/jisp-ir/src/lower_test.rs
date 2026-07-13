use jisp_core::{SourceId, SyntaxParser};
use jisp_syntax_json::JsonParser;

use crate::{ExprKind, Lowerer};

fn lower(source: &str) -> Result<crate::Module, crate::LowerError> {
    let nodes = JsonParser.parse_module(SourceId(0), source).unwrap();
    Lowerer.lower_module(&nodes)
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

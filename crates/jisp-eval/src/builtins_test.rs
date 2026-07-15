use jisp_core::{SourceId, Span};

use crate::{normalize_local_action, normalize_update_result, Evaluator, Value};

fn span() -> Span {
    Span::empty(SourceId(0), 0)
}

fn action(tag: &str) -> Value {
    Value::Obj(indexmap::IndexMap::from([
        ("tag".to_owned(), Value::string(tag)),
        ("fields".to_owned(), Value::List(vec![])),
    ]))
}

#[test]
fn ui_result_declares_resources_without_executing_them() {
    let mut evaluator = Evaluator::new();
    let result = evaluator
        .apply(
            evaluator.root_env().lookup("ui.result").unwrap(),
            &[
                Value::Int(2),
                Value::List(vec![Value::Obj(indexmap::IndexMap::from([(
                    "kind".to_owned(),
                    Value::string("storage.write"),
                )]))]),
                Value::List(vec![]),
            ],
            span(),
        )
        .unwrap();

    let result = normalize_update_result(result, span()).unwrap();
    assert!(matches!(result.state, Value::Int(2)));
    assert_eq!(result.commands.len(), 1);
    assert!(result.subscriptions.is_empty());
}

#[test]
fn ui_result_rejects_nonportable_resource_data() {
    let mut evaluator = Evaluator::new();
    let error = evaluator
        .apply(
            evaluator.root_env().lookup("ui.result").unwrap(),
            &[
                Value::Null,
                Value::List(vec![evaluator.root_env().lookup("ui.node").unwrap()]),
                Value::List(vec![]),
            ],
            span(),
        )
        .unwrap_err();

    assert!(error.message.contains("must contain portable data"));
}

#[test]
fn ui_local_result_produces_an_opaque_replacement_snapshot() {
    let mut evaluator = Evaluator::new();
    let result = evaluator
        .apply(
            evaluator.root_env().lookup("ui.local.result").unwrap(),
            &[
                Value::Bool(true),
                Value::List(vec![Value::Obj(indexmap::IndexMap::from([(
                    "kind".to_owned(),
                    Value::string("command"),
                )]))]),
                Value::List(vec![]),
            ],
            span(),
        )
        .unwrap();
    let action = normalize_local_action(result, span()).unwrap().unwrap();
    assert!(action.id.is_none());
    assert!(matches!(action.state, Value::Bool(true)));
    let (commands, subscriptions) = action.resources.expect("replacement resources");
    assert_eq!(commands.len(), 1);
    assert!(subscriptions.is_empty());
}

#[test]
fn ui_local_result_rejects_nonportable_resource_data() {
    let mut evaluator = Evaluator::new();
    let error = evaluator
        .apply(
            evaluator.root_env().lookup("ui.local.result").unwrap(),
            &[
                Value::Null,
                Value::List(vec![evaluator.root_env().lookup("ui.node").unwrap()]),
                Value::List(vec![]),
            ],
            span(),
        )
        .unwrap_err();
    assert!(error.message.contains("must contain portable data"));
}

#[test]
fn ui_resource_constructors_produce_canonical_portable_descriptors() {
    let mut evaluator = Evaluator::new();
    let command = evaluator
        .apply(
            evaluator.root_env().lookup("ui.command").unwrap(),
            &[
                Value::string("save:1"),
                Value::string("storage.write"),
                Value::Int(1),
                Value::Obj(indexmap::IndexMap::from([(
                    "key".to_owned(),
                    Value::string("draft"),
                )])),
                Value::Bool(true),
                action("Saved"),
                action("SaveFailed"),
            ],
            span(),
        )
        .unwrap();
    let Value::Obj(command) = command else {
        panic!("ui.command must return an object descriptor");
    };
    assert!(matches!(command.get("kind"), Some(Value::Str(kind)) if kind.as_ref() == "command"));
    assert!(matches!(command.get("id"), Some(Value::Str(id)) if id.as_ref() == "save:1"));
    let Some(Value::Obj(capability)) = command.get("capability") else {
        panic!("ui.command must include a capability object");
    };
    assert!(
        matches!(capability.get("name"), Some(Value::Str(name)) if name.as_ref() == "storage.write")
    );
    assert!(matches!(capability.get("version"), Some(Value::Int(1))));
    assert!(matches!(command.get("replace"), Some(Value::Bool(true))));
}

#[test]
fn ui_resource_constructors_reject_invalid_identity_and_nonportable_request() {
    let mut evaluator = Evaluator::new();
    let command = evaluator.root_env().lookup("ui.command").unwrap();
    let invalid_version = evaluator
        .apply(
            command.clone(),
            &[
                Value::string("save:1"),
                Value::string("storage.write"),
                Value::Int(0),
                Value::Null,
                Value::Bool(true),
                action("Saved"),
                action("SaveFailed"),
            ],
            span(),
        )
        .unwrap_err();
    assert!(invalid_version.message.contains("positive u32"));

    let invalid_request = evaluator
        .apply(
            command,
            &[
                Value::string("save:1"),
                Value::string("storage.write"),
                Value::Int(1),
                evaluator.root_env().lookup("ui.node").unwrap(),
                Value::Bool(true),
                action("Saved"),
                action("SaveFailed"),
            ],
            span(),
        )
        .unwrap_err();
    assert!(invalid_request
        .message
        .contains("JSON-shaped portable data"));
}

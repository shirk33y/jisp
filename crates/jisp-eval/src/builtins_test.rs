use jisp_core::{SourceId, Span};

use crate::{normalize_update_result, Evaluator, Value};

fn span() -> Span {
    Span::empty(SourceId(0), 0)
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

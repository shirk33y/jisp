use indexmap::IndexMap;
use jisp_eval::Value;
use serde_json::json;

use super::{
    super::Capability, super::FakeHost, super::ResourceKind, super::Trace, decode_resources,
};

fn obj(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Obj(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<IndexMap<_, _>>(),
    )
}

fn command(id: &str) -> Value {
    obj([
        ("kind", Value::string("command")),
        ("id", Value::string(id)),
        (
            "capability",
            obj([
                ("name", Value::string("storage.write")),
                ("version", Value::Int(1)),
            ]),
        ),
        ("request", obj([("key", Value::string("draft"))])),
        ("replace", Value::Bool(true)),
    ])
}

#[test]
fn decodes_source_descriptors_and_reconciles_them_through_the_fake_host() {
    let desired = decode_resources(&[command("save:1")], &[]).unwrap();
    assert_eq!(desired.commands[0].request, json!({ "key": "draft" }));

    let mut host = FakeHost::with_capabilities([Capability {
        name: "storage.write".to_owned(),
        version: 1,
    }]);
    host.reconcile_declared_resources(&[command("save:1")], &[])
        .unwrap();
    assert!(matches!(
        host.trace.as_slice(),
        [Trace::Start {
            kind: ResourceKind::Command,
            id,
            generation: 1,
            ..
        }] if id == "save:1"
    ));
}

#[test]
fn rejects_wrong_list_kind_unknown_fields_and_non_json_request_values() {
    let wrong_kind = obj([
        ("kind", Value::string("subscription")),
        ("id", Value::string("clock")),
        (
            "capability",
            obj([
                ("name", Value::string("timer.sleep")),
                ("version", Value::Int(1)),
            ]),
        ),
        ("request", Value::Null),
        ("replace", Value::Bool(true)),
    ]);
    assert!(decode_resources(&[wrong_kind], &[])
        .unwrap_err()
        .to_string()
        .contains("kind does not match"));

    let unknown = obj([
        ("kind", Value::string("command")),
        ("id", Value::string("save:1")),
        (
            "capability",
            obj([
                ("name", Value::string("storage.write")),
                ("version", Value::Int(1)),
            ]),
        ),
        ("request", Value::BigInt(1.into())),
        ("replace", Value::Bool(true)),
        ("extra", Value::Null),
    ]);
    assert!(decode_resources(&[unknown], &[])
        .unwrap_err()
        .to_string()
        .contains("exactly"));
}

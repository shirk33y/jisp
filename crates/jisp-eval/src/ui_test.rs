use indexmap::IndexMap;
use jisp_core::{SourceId, Span};

use crate::ui::render_html;
use crate::Value;

fn span() -> Span {
    Span::empty(SourceId(0), 0)
}

fn obj(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Obj(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<IndexMap<_, _>>(),
    )
}

fn string(value: &str) -> Value {
    Value::string(value)
}

#[test]
fn renders_structural_ui_data_to_html() {
    let node = obj([
        ("tag", string("button")),
        ("id", string("save")),
        (
            "classes",
            obj([
                ("px-4", Value::Bool(true)),
                ("opacity-50", Value::Bool(false)),
                ("bg-emerald-600", Value::Bool(true)),
            ]),
        ),
        (
            "children",
            Value::List(vec![obj([
                ("tag", string("text")),
                ("value", string("Save")),
            ])]),
        ),
    ]);

    let html = render_html(&node, span()).unwrap();

    assert_eq!(
        html,
        r#"<button class="px-4 bg-emerald-600" id="save">Save</button>"#
    );
}

#[test]
fn escapes_text_and_attribute_values() {
    let node = obj([
        ("tag", string("span")),
        ("title", string("\"<draft>\" & live")),
        (
            "children",
            Value::List(vec![obj([
                ("tag", string("text")),
                ("value", string("<Save & close>")),
            ])]),
        ),
    ]);

    let html = render_html(&node, span()).unwrap();

    assert_eq!(
        html,
        r#"<span title="&quot;&lt;draft&gt;&quot; &amp; live">&lt;Save &amp; close&gt;</span>"#
    );
}

#[test]
fn rejects_javascript_urls_in_static_html() {
    let node = obj([
        ("tag", string("a")),
        ("attrs", obj([("href", string("  JaVaScRiPt:alert(1)"))])),
    ]);

    let error = render_html(&node, span()).unwrap_err();

    assert!(error.message.contains("must not use a javascript: URL"));
}

#[test]
fn rejects_non_bool_class_flags() {
    let node = obj([
        ("tag", string("div")),
        ("classes", obj([("px-4", string("yes"))])),
    ]);

    let error = render_html(&node, span()).unwrap_err();

    assert_eq!(error.message, "ui class flags must be bool, got str");
}

#[test]
fn renders_explicit_attrs_props_and_nested_child_lists() {
    let node = obj([
        ("tag", string("div")),
        ("key", Value::Int(7)),
        ("events", obj([("click", Value::Null)])),
        (
            "attrs",
            obj([("aria-label", string("Tasks")), ("data-id", string("7"))]),
        ),
        ("props", obj([("hidden", Value::Bool(false))])),
        (
            "children",
            Value::List(vec![Value::List(vec![obj([
                ("tag", string("span")),
                (
                    "children",
                    Value::List(vec![obj([
                        ("tag", string("text")),
                        ("value", string("One")),
                    ])]),
                ),
            ])])]),
        ),
    ]);

    let html = render_html(&node, span()).unwrap();

    assert_eq!(
        html,
        r#"<div aria-label="Tasks" data-id="7"><span>One</span></div>"#
    );
}

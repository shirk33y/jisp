use std::{fs, path::PathBuf};

use jisp::{
    jisp_core::{SourceId, Span},
    jisp_eval::{Evaluator, Value},
};

jisp_macros::lisp_file!("tests/fixtures/differential.lisp");

const FIXTURE: &str = "tests/fixtures/differential.lisp";

#[test]
fn native_scalars_match_the_interpreter() {
    assert_matches_interpreter("scalar-entry", Value::Int(scalar_entry()));
    assert_matches_interpreter("object-field-entry", Value::Int(object_field_entry()));
    assert_matches_interpreter("boolean-entry", Value::Bool(boolean_entry()));
}

#[test]
fn native_strings_and_lists_match_the_interpreter() {
    assert_matches_interpreter("string-entry", Value::string(string_entry()));
    assert_matches_interpreter(
        "list-entry",
        Value::List(list_entry().into_iter().map(Value::Int).collect()),
    );
}

#[test]
fn native_callbacks_and_list_higher_order_helpers_match_the_interpreter() {
    assert_matches_interpreter(
        "map-entry",
        Value::List(map_entry().into_iter().map(Value::Int).collect()),
    );
    assert_matches_interpreter(
        "filter-entry",
        Value::List(filter_entry().into_iter().map(Value::Int).collect()),
    );
    assert_matches_interpreter("fold-entry", Value::Int(fold_entry()));
    assert_matches_interpreter("some-entry", Value::Int(some_entry()));
    assert_matches_interpreter("every-entry", Value::Int(every_entry()));
    assert_matches_interpreter("higher-order-entry", Value::Int(higher_order_entry()));
}

#[test]
fn native_enum_cases_match_the_interpreter() {
    assert_matches_interpreter("enum-case-entry", Value::Int(enum_case_entry()));
}

fn assert_matches_interpreter(export: &str, native: Value) {
    let interpreted = interpreter_export(export);
    assert!(
        interpreted.structurally_equal(&native).unwrap(),
        "{FIXTURE} export `{export}` diverged: interpreter={}, native={}",
        interpreted.display_string(),
        native.display_string(),
    );
}

fn interpreter_export(export: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let text = fs::read_to_string(&path).unwrap();
    let module = jisp::evaluate(&path, &text).unwrap();
    let entry = module.exports.get(export).unwrap().clone();
    Evaluator::new()
        .apply(entry, &[], Span::empty(SourceId(0), 0))
        .unwrap()
}

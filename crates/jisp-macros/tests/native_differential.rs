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
fn native_static_object_get_matches_the_interpreter() {
    assert_matches_interpreter(
        "object-get-discarded-entry",
        Value::Int(object_get_discarded_entry()),
    );
    assert_matches_interpreter("object-get-case-entry", Value::Int(object_get_case_entry()));
    assert_matches_interpreter(
        "inline-object-get-entry",
        Value::Int(inline_object_get_entry()),
    );
}

#[test]
fn native_result_helpers_match_the_interpreter() {
    assert_matches_interpreter("result-map-entry", Value::Int(result_map_entry()));
    assert_matches_interpreter("result-map-err-entry", Value::Int(result_map_err_entry()));
    assert_matches_interpreter("result-try-entry", Value::Int(result_try_entry()));
    assert_matches_interpreter("result-recover-entry", Value::Int(result_recover_entry()));
}

#[test]
fn native_option_cases_match_the_interpreter() {
    assert_matches_interpreter("option-case-entry", Value::Int(option_case_entry()));
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
    assert_matches_interpreter(
        "first-class-call-entry",
        Value::Int(first_class_call_entry()),
    );
}

#[test]
fn native_variadic_functions_match_the_interpreter() {
    assert_matches_interpreter("variadic-empty-entry", Value::Int(variadic_empty_entry()));
    assert_matches_interpreter("variadic-many-entry", Value::Int(variadic_many_entry()));
    assert_matches_interpreter("variadic-local-entry", Value::Int(variadic_local_entry()));
    assert_matches_interpreter(
        "variadic-expression-entry",
        Value::Int(variadic_expression_entry()),
    );
    assert_matches_interpreter(
        "variadic-returned-entry",
        Value::Int(variadic_returned_entry()),
    );
}

#[test]
fn native_local_closures_match_the_interpreter() {
    assert_matches_interpreter("local-function-entry", Value::Int(local_function_entry()));
    assert_matches_interpreter(
        "immediate-lambda-entry",
        Value::Int(immediate_lambda_entry()),
    );
    assert_matches_interpreter(
        "captured-map-entry",
        Value::List(captured_map_entry().into_iter().map(Value::Int).collect()),
    );
    assert_matches_interpreter(
        "captured-string-entry",
        Value::string(captured_string_entry()),
    );
    assert_matches_interpreter(
        "captured-string-map-entry",
        Value::List(
            captured_string_map_entry()
                .into_iter()
                .map(Value::string)
                .collect(),
        ),
    );
    assert_matches_interpreter("captured-use-entry", Value::Int(captured_use_entry()));
    assert_matches_interpreter(
        "returned-closure-entry",
        Value::Int(returned_closure_entry()),
    );
    assert_matches_interpreter(
        "returned-closure-map-entry",
        Value::List(
            returned_closure_map_entry()
                .into_iter()
                .map(Value::Int)
                .collect(),
        ),
    );
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

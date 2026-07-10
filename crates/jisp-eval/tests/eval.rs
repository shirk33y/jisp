use jisp_core::{SourceId, SyntaxParser};
use jisp_eval::{Evaluator, RuntimeError, Value};
use jisp_ir::Lowerer;
use jisp_syntax_json::JsonParser;

fn run(source: &str) -> Value {
    run_result(source).unwrap()
}

fn run_result(source: &str) -> Result<Value, RuntimeError> {
    let nodes = JsonParser.parse_module(SourceId(0), source).unwrap();
    let module = Lowerer.lower_module(&nodes).unwrap();
    Evaluator::new().run_main(&module)
}

#[test]
fn evaluates_recursive_functions() {
    let value = run(r#"[
          ["def","fact",
            ["fn",["n"],
              ["if",["=","n",0],
                1,
                ["*","n",["fact",["-","n",1]]]]]],
          ["export","main",["fn",[],["fact",5]]]
        ]"#);
    assert!(matches!(value, Value::Int(120)));
}

#[test]
fn evaluates_enum_constructors_and_case() {
    let value = run(r#"[
          ["type","state",["loading"],["ready","str"],["failed","str"]],
          ["export","main",
            ["fn",[],
              ["case",["ready",["str","Ada"]],
                [["loading"],["str","wait"]],
                [["ready","name"],["str","Hello ",[",","name"]]],
                [["failed","reason"],"reason"]]]]
        ]"#);
    assert_eq!(value.display_string(), "Hello Ada");
}

#[test]
fn use_desugars_result_propagation() {
    let value = run(r#"[
          ["def","load",["fn",[],["ok",41]]],
          ["export","main",
            ["fn",[],
              ["use","value",["result.try",["load"]],
                ["ok",["+","value",1]]]]]
        ]"#);
    assert_eq!(value.display_string(), "[ok, 42]");
}

#[test]
fn strings_can_splice_a_list() {
    let value = run(r#"[
          ["export","main",
            ["fn",[],
              ["str.lines","one",[",@",["list",["str","two"],["str","three"]]]]]]
        ]"#);
    assert_eq!(value.display_string(), "one\ntwo\nthree");
}

#[test]
fn integer_arithmetic_reports_overflow() {
    let error = run_result(
        r#"[
          ["export","main",["fn",[],["+",
            9223372036854775807,
            1]]]
        ]"#,
    )
    .unwrap_err();

    assert_eq!(error.message, "integer overflow");
}

#[test]
fn floor_division_reports_min_value_overflow() {
    let error = run_result(
        r#"[
          ["export","main",["fn",[],["//",
            -9223372036854775808,
            -1]]]
        ]"#,
    )
    .unwrap_err();

    assert_eq!(error.message, "division by zero or integer overflow");
}

#[test]
fn integer_division_modes_are_explicit() {
    let value = run(r#"[
          ["export","main",["fn",[],
            ["list",["/",-7,3],["//",-7,3],["%",-7,3]]]]
        ]"#);

    assert_eq!(value.display_string(), "[-2, -3, 2]");
}

#[test]
fn numeric_operations_reject_mixed_int_and_float() {
    let error = run_result(
        r#"[
          ["export","main",["fn",[],["/",1,2.0]]]
        ]"#,
    )
    .unwrap_err();

    assert_eq!(
        error.message,
        "/ requires two values of the same numeric type"
    );
}

#[test]
fn float_division_by_zero_is_a_runtime_error() {
    let error = run_result(
        r#"[
          ["export","main",["fn",[],["/",1.0,0.0]]]
        ]"#,
    )
    .unwrap_err();

    assert_eq!(error.message, "division by zero");
}

#[test]
fn nan_is_not_equal_to_itself() {
    let value = run(r#"[
          ["export","main",["fn",[],
            ["=",["math.sqrt",-1.0],["math.sqrt",-1.0]]]]
        ]"#);

    assert!(matches!(value, Value::Bool(false)));
}

use jisp_core::{SourceId, SyntaxParser};
use jisp_eval::{Evaluator, Value};
use jisp_ir::Lowerer;
use jisp_syntax_json::JsonParser;

fn run(source: &str) -> Value {
    let nodes = JsonParser.parse_module(SourceId(0), source).unwrap();
    let module = Lowerer.lower_module(&nodes).unwrap();
    Evaluator::new().run_main(&module).unwrap()
}

#[test]
fn evaluates_recursive_functions() {
    let value = run(
        r#"[
          ["def","fact",
            ["fn",["n"],
              ["if",["=","n",0],
                1,
                ["*","n",["fact",["-","n",1]]]]]],
          ["export","main",["fn",[],["fact",5]]]
        ]"#,
    );
    assert!(matches!(value, Value::Int(120)));
}

#[test]
fn evaluates_enum_constructors_and_case() {
    let value = run(
        r#"[
          ["type","state",["loading"],["ready","str"],["failed","str"]],
          ["export","main",
            ["fn",[],
              ["case",["ready",["str","Ada"]],
                [["loading"],["str","wait"]],
                [["ready","name"],["str","Hello ",[",","name"]]],
                [["failed","reason"],"reason"]]]]
        ]"#,
    );
    assert_eq!(value.display_string(), "Hello Ada");
}

#[test]
fn use_desugars_result_propagation() {
    let value = run(
        r#"[
          ["def","load",["fn",[],["ok",41]]],
          ["export","main",
            ["fn",[],
              ["use","value",["result.try",["load"]],
                ["ok",["+","value",1]]]]]
        ]"#,
    );
    assert_eq!(value.display_string(), "[ok, 42]");
}

#[test]
fn strings_can_splice_a_list() {
    let value = run(
        r#"[
          ["export","main",
            ["fn",[],
              ["str.lines","one",[",@",["list",["str","two"],["str","three"]]]]]]
        ]"#,
    );
    assert_eq!(value.display_string(), "one\ntwo\nthree");
}

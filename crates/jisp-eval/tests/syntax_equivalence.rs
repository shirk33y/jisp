use jisp_core::{SourceId, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_yaml::YamlParser;

fn result(parser: &dyn SyntaxParser, source: &str) -> String {
    let nodes = parser.parse_module(SourceId(0), source).unwrap();
    let module = Lowerer.lower_module(&nodes).unwrap();
    Evaluator::new().run_main(&module).unwrap().display_string()
}

#[test]
fn all_three_syntaxes_share_semantics() {
    let json = r#"[
      ["export","main",["fn",[],["str.upper",["str","hello"]]]]
    ]"#;
    let yaml = r#"[
      [export, main, [fn, [], [str.upper, "hello"]]]
    ]"#;
    let lisp = r#"
      (export main (fn () (str.upper "hello")))
    "#;

    assert_eq!(result(&JsonParser, json), "HELLO");
    assert_eq!(result(&YamlParser, yaml), "HELLO");
    assert_eq!(result(&LispParser, lisp), "HELLO");
}

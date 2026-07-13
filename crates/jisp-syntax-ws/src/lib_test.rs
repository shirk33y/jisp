use jisp_core::{Node, NodeKind, SourceId, SyntaxParser};

use super::WsParser;

fn parse_one(source: &str) -> Node {
    let nodes = WsParser.parse_module(SourceId(0), source).unwrap();
    assert_eq!(nodes.len(), 1);
    nodes.into_iter().next().unwrap()
}

fn render(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => serde_json::to_string(value.as_ref()).unwrap(),
        NodeKind::Form(items) => {
            let rendered = items.iter().map(render).collect::<Vec<_>>().join(" ");
            format!("({rendered})")
        }
    }
}

#[test]
fn continuations_extend_immediate_parent_in_source_order() {
    let node = parse_one(
        r#"call-this a b
  ... c d
  foo 123
  ... e f
  bar 222"#,
    );

    assert_eq!(render(&node), "(call-this a b c d (foo 123) e f (bar 222))");
}

#[test]
fn inline_ellipsis_remains_available_for_rest_arguments() {
    let node = parse_one(
        r#"defn foo (a b ... rest)
  ... body"#,
    );

    assert_eq!(render(&node), "(defn foo (a b ... rest) body)");
}

#[test]
fn nested_line_with_inline_ellipsis_is_a_form() {
    let node = parse_one(
        r#"fn foo
  a b ... rest
  ... body"#,
    );

    assert_eq!(render(&node), "(fn foo (a b ... rest) body)");
}

#[test]
fn explicit_form_can_be_callee() {
    let node = parse_one(
        r#"(make-adder 11)
  7"#,
    );

    assert_eq!(render(&node), "((make-adder 11) 7)");
}

#[test]
fn object_pairs_can_use_continuations_around_nested_values() {
    let node = parse_one(
        r#"obj
  ... "sum"
  + 2 2
  ... "label" label"#,
    );

    assert_eq!(render(&node), r#"(obj "sum" (+ 2 2) "label" label)"#);
}

#[test]
fn parses_strings_and_comments_without_eating_hash_symbols() {
    let node = parse_one(
        r#"print "hello # world" # comment
  ... #tag"#,
    );

    assert_eq!(render(&node), r#"(print "hello # world" #tag)"#);
}

#[test]
fn parses_multiple_top_level_forms() {
    let nodes = WsParser
        .parse_module(
            SourceId(0),
            r#"def x 1
def y
  + x 2"#,
        )
        .unwrap();

    assert_eq!(
        nodes.iter().map(render).collect::<Vec<_>>(),
        ["(def x 1)", "(def y (+ x 2))"]
    );
}

#[test]
fn rejects_multiline_explicit_forms() {
    let error = WsParser
        .parse_module(
            SourceId(0),
            r#"defn foo (a b)
  ... (x y z
    ... k l"#,
        )
        .unwrap_err();

    assert!(error.to_string().contains("syntax error"));
}

#[test]
fn rejects_line_leading_ellipsis_typos() {
    let error = WsParser
        .parse_module(SourceId(0), "f\n  ...rest")
        .unwrap_err();

    assert!(error
        .diagnostics
        .first()
        .unwrap()
        .message
        .contains("ellipsis-like"));
}

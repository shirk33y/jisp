use jisp_core::{Node, NodeKind, SourceId, Span};

use crate::{expand_module, ExpansionMap};

fn span(start: usize, end: usize) -> Span {
    Span::new(SourceId(0), start, end)
}

fn sym(value: &str) -> Node {
    Node::symbol(value, span(0, value.len()))
}

fn sym_at(value: &str, origin: Span) -> Node {
    Node::symbol(value, origin)
}

fn int(value: i64) -> Node {
    Node::new(NodeKind::Int(value), span(0, 1))
}

fn int_at(value: i64, origin: Span) -> Node {
    Node::new(NodeKind::Int(value), origin)
}

fn form(items: Vec<Node>) -> Node {
    Node::form(items, span(0, 1))
}

fn form_at(items: Vec<Node>, origin: Span) -> Node {
    Node::form(items, origin)
}

#[test]
fn expands_quote_to_origin_tracked_syntax() {
    let origin = span(10, 20);
    let quoted = Node::form(vec![sym("quote"), form(vec![sym("list"), int(1)])], origin);

    let expanded = expand_module(&[quoted]).unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form_at(
            vec![sym_at("list", origin), int_at(1, origin)],
            origin
        )]
    );
    assert!(!expanded.expansion_map.is_empty());
    assert_eq!(
        expanded.expansion_map.origin(expanded.nodes[0].span),
        origin
    );
}

#[test]
fn expands_quasiquote_unquote_and_splicing() {
    let origin = span(10, 50);
    let quasiquoted = Node::form(
        vec![
            sym("`"),
            form(vec![
                sym("list"),
                int(1),
                form(vec![sym(","), form(vec![sym("+"), int(1), int(1)])]),
                form(vec![sym(",@"), form(vec![int(3), int(4)])]),
            ]),
        ],
        origin,
    );

    let expanded = expand_module(&[quasiquoted]).unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form_at(
            vec![
                sym_at("list", origin),
                int_at(1, origin),
                form_at(
                    vec![sym_at("+", origin), int_at(1, origin), int_at(1, origin)],
                    origin,
                ),
                int_at(3, origin),
                int_at(4, origin),
            ],
            origin,
        )]
    );
}

#[test]
fn rejects_unquote_outside_quasiquote() {
    let error = expand_module(&[form(vec![sym(","), sym("value")])]).unwrap_err();

    assert_eq!(error.diagnostics[0].code.as_deref(), Some("JISP-EXPAND"));
    assert!(error.diagnostics[0]
        .message
        .contains("unquote is only valid inside quasiquote"));
}

#[test]
fn follows_origin_chains_with_a_depth_limit() {
    let mut map = ExpansionMap::default();
    let generated = span(0, 1);
    let first = span(2, 3);
    let original = span(4, 5);
    map.record(generated, first);
    map.record(first, original);

    assert_eq!(map.origin(generated), original);
}

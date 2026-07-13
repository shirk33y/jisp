use jisp_core::{Node, NodeKind, SourceId, Span};

use crate::{expand_module, expand_module_with_imported_macros, ExpansionMap};

fn span(start: usize, end: usize) -> Span {
    Span::new(SourceId(0), start, end)
}

fn sym_at(value: &str, start: usize, end: usize) -> Node {
    Node::symbol(value, span(start, end))
}

fn sym(value: &str) -> Node {
    Node::symbol(value, span(0, value.len()))
}

fn int(value: i64) -> Node {
    Node::new(NodeKind::Int(value), span(0, 1))
}

fn string(value: &str) -> Node {
    Node::string(value, span(0, value.len()))
}

fn form(items: Vec<Node>) -> Node {
    Node::form(items, span(0, 1))
}

#[test]
fn expands_quote_to_origin_tracked_syntax() {
    let origin = span(10, 20);
    let quoted = Node::form(vec![sym("quote"), form(vec![sym("list"), int(1)])], origin);

    let expanded = expand_module(&[quoted]).unwrap();

    assert_eq!(expanded.nodes, vec![form(vec![sym("list"), int(1)])]);
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
        vec![form(vec![
            sym("list"),
            int(1),
            form(vec![sym("+"), int(1), int(1)]),
            int(3),
            int(4),
        ])]
    );
    assert_eq!(
        expanded.expansion_map.origin(expanded.nodes[0].span),
        origin
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
fn keeps_string_template_unquote_and_expands_inner_expression() {
    let expanded = expand_module(&[form(vec![
        sym("str"),
        string("value: "),
        form(vec![
            sym(","),
            form(vec![sym("quote"), form(vec![sym("list"), int(1)])]),
        ]),
    ])])
    .unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form(vec![
            sym("str"),
            string("value: "),
            form(vec![sym(","), form(vec![sym("list"), int(1)])]),
        ])]
    );
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
    assert_eq!(map.origin_chain(generated), vec![first, original]);
}

#[test]
fn expands_ordered_user_macro_quasiquote_and_removes_its_definition() {
    let definition = form(vec![
        sym("def"),
        sym("unless"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("condition"), sym("then"), sym("otherwise")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("if"),
                        form(vec![sym(","), sym("condition")]),
                        form(vec![sym(","), sym("otherwise")]),
                        form(vec![sym(","), sym("then")]),
                    ]),
                ]),
            ]),
        ]),
    ]);
    let call = Node::form(
        vec![sym("unless"), sym("ready"), int(1), int(2)],
        span(30, 50),
    );

    let expanded = expand_module(&[definition, call]).unwrap();

    assert_eq!(
        expanded.nodes,
        vec![Node::form(
            vec![sym("if"), sym("ready"), int(2), int(1)],
            span(0, 1),
        )]
    );
    assert_eq!(
        expanded.expansion_map.origin(expanded.nodes[0].span),
        span(30, 50)
    );
}

#[test]
fn expands_imported_macro_under_alias_and_removes_macro_import() {
    let imported = vec![form(vec![
        sym("def"),
        sym("wrap"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("expression")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("list"),
                        string("wrapped"),
                        form(vec![sym(","), sym("expression")]),
                    ]),
                ]),
            ]),
        ]),
    ])];
    let nodes = vec![
        form(vec![sym("macro-import"), sym("m"), string("macros.lisp")]),
        form(vec![sym("m.wrap"), int(7)]),
    ];

    let expanded =
        expand_module_with_imported_macros(&nodes, &[("m".to_owned(), imported)]).unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form(vec![sym("list"), string("wrapped"), int(7)])]
    );
}

#[test]
fn rejects_exporting_a_module_local_macro() {
    let definition = form(vec![
        sym("def"),
        sym("unless"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("condition"), sym("then"), sym("otherwise")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("if"),
                        form(vec![sym(","), sym("condition")]),
                        form(vec![sym(","), sym("otherwise")]),
                        form(vec![sym(","), sym("then")]),
                    ]),
                ]),
            ]),
        ]),
    ]);
    let export = form(vec![sym("export"), sym("unless")]);

    let error = expand_module(&[export, definition]).unwrap_err();

    assert_eq!(error.diagnostics[0].code.as_deref(), Some("JISP-EXPAND"));
    assert!(error.diagnostics[0]
        .message
        .contains("macro `unless` cannot be exported"));
}

#[test]
fn rejects_inline_exported_macro_definition() {
    let export = form(vec![
        sym("export"),
        sym("unless"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("condition")]),
                form(vec![sym("quote"), sym("condition")]),
            ]),
        ]),
    ]);

    let error = expand_module(&[export]).unwrap_err();

    assert_eq!(error.diagnostics[0].code.as_deref(), Some("JISP-EXPAND"));
    assert!(error.diagnostics[0]
        .message
        .contains("macros cannot be exported"));
}

#[test]
fn expands_nested_user_macro_calls_and_variadic_splices() {
    let wrap = form(vec![
        sym("def"),
        sym("wrap"),
        form(vec![
            sym("macro"),
            form(vec![
                sym("fn"),
                form(vec![sym("..."), sym("body")]),
                form(vec![
                    sym("`"),
                    form(vec![sym("do"), form(vec![sym(",@"), sym("body")])]),
                ]),
            ]),
        ]),
    ]);
    let twice = form(vec![
        sym("def"),
        sym("twice"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("value")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("wrap"),
                        form(vec![sym(","), sym("value")]),
                        form(vec![sym(","), sym("value")]),
                    ]),
                ]),
            ]),
        ]),
    ]);

    let expanded = expand_module(&[wrap, twice, form(vec![sym("twice"), int(7)])]).unwrap();

    assert_eq!(expanded.nodes, vec![form(vec![sym("do"), int(7), int(7)])]);
}

#[test]
fn hygienic_macro_let_binding_does_not_capture_caller_identifier() {
    let definition = form(vec![
        sym("def"),
        sym("wrap"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("expression")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("let"),
                        form(vec![sym_at("value", 10, 15), int(1)]),
                        form(vec![
                            sym("+"),
                            sym_at("value", 10, 15),
                            form(vec![sym(","), sym("expression")]),
                        ]),
                    ]),
                ]),
            ]),
        ]),
    ]);
    let caller_value = sym_at("value", 100, 105);

    let expanded =
        expand_module(&[definition, form(vec![sym("wrap"), caller_value.clone()])]).unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form(vec![
            sym("let"),
            form(vec![sym_at("__jisp_macro_0_value", 10, 15), int(1)]),
            form(vec![
                sym("+"),
                sym_at("__jisp_macro_0_value", 10, 15),
                caller_value
            ]),
        ])]
    );
}

#[test]
fn hygienic_macro_preserves_caller_supplied_bindings() {
    let definition = form(vec![
        sym("def"),
        sym("make-fn"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("binding"), sym("body")]),
                form(vec![
                    sym("`"),
                    form(vec![
                        sym("fn"),
                        form(vec![form(vec![sym(","), sym("binding")])]),
                        form(vec![sym(","), sym("body")]),
                    ]),
                ]),
            ]),
        ]),
    ]);
    let caller_binding = sym_at("value", 100, 105);
    let caller_body = form(vec![sym("+"), sym_at("value", 100, 105), int(1)]);

    let expanded = expand_module(&[
        definition,
        form(vec![
            sym("make-fn"),
            caller_binding.clone(),
            caller_body.clone(),
        ]),
    ])
    .unwrap();

    assert_eq!(
        expanded.nodes,
        vec![form(vec![
            sym("fn"),
            form(vec![caller_binding]),
            caller_body
        ])]
    );
}

#[test]
fn user_macro_reports_arity_and_template_errors() {
    let definition = form(vec![
        sym("def"),
        sym("identity"),
        form(vec![
            sym("~"),
            form(vec![
                sym("fn"),
                form(vec![sym("value")]),
                form(vec![sym("`"), form(vec![sym(","), sym("value")])]),
            ]),
        ]),
    ]);
    let error = expand_module(&[definition, form(vec![sym("identity")])]).unwrap_err();
    assert!(error.diagnostics[0].message.contains("expects 1 argument"));

    let invalid = form(vec![
        sym("def"),
        sym("bad"),
        form(vec![sym("~"), form(vec![sym("fn"), form(vec![]), int(1)])]),
    ]);
    let error = expand_module(&[invalid]).unwrap_err();
    assert!(error.diagnostics[0]
        .message
        .contains("macro body must be a quote or quasiquote"));
}

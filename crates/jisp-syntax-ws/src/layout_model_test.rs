use std::collections::{BTreeSet, HashMap};

use jisp_core::{Node, NodeKind, SourceId, SyntaxParser};

use super::WsParser;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Datum {
    Atom(&'static str),
    Form(Vec<Datum>),
}

#[test]
fn bounded_generated_layout_round_trips() {
    let mut memo = HashMap::new();
    let mut checked = 0usize;

    for nodes in 1..=8 {
        for datum in datums_with_nodes(nodes, &mut memo) {
            let layout = render_layout(&datum, 0);
            let parsed = parse_one(&layout);
            assert_eq!(
                render_node(&parsed),
                render_sexpr(&datum),
                "layout roundtrip failed for datum {}\n{}",
                render_sexpr(&datum),
                layout
            );
            checked += 1;
        }
    }

    assert_eq!(checked, 277_315);
}

#[test]
fn adversarial_layout_cases_match_contract() {
    for case in ADVERSARIAL_CASES {
        let result = parse_rendered(case.source);
        match (&case.expected, result) {
            (Expected::Ok(expected), Ok(actual)) => assert_eq!(
                &actual, expected,
                "case parsed differently: {}\n{}",
                case.name, case.source
            ),
            (Expected::Err(expected), Err(actual)) => assert!(
                actual.contains(expected),
                "case failed with a different error: {}\n{}\nexpected: {}\nactual: {}",
                case.name,
                case.source,
                expected,
                actual
            ),
            (Expected::Ok(expected), Err(actual)) => panic!(
                "case failed unexpectedly: {}\n{}\nexpected: {:?}\nactual: {}",
                case.name, case.source, expected, actual
            ),
            (Expected::Err(expected), Ok(actual)) => panic!(
                "case should have failed: {}\n{}\nexpected error: {}\nparsed: {:?}",
                case.name, case.source, expected, actual
            ),
        }
    }
}

fn parse_one(source: &str) -> Node {
    let nodes = WsParser.parse_module(SourceId(0), source).unwrap();
    assert_eq!(nodes.len(), 1);
    nodes.into_iter().next().unwrap()
}

fn parse_rendered(source: &str) -> Result<Vec<String>, String> {
    WsParser
        .parse_module(SourceId(0), source)
        .map(|nodes| nodes.iter().map(render_node).collect())
        .map_err(|error| {
            error
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.clone())
                .unwrap_or_else(|| error.to_string())
        })
}

fn render_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Null => "null".to_owned(),
        NodeKind::Bool(value) => value.to_string(),
        NodeKind::Int(value) => value.to_string(),
        NodeKind::Float(value) => value.to_string(),
        NodeKind::Symbol(value) => value.to_string(),
        NodeKind::String(value) => serde_json::to_string(value.as_ref()).unwrap(),
        NodeKind::Form(items) => {
            let rendered = items.iter().map(render_node).collect::<Vec<_>>().join(" ");
            format!("({rendered})")
        }
    }
}

fn render_sexpr(datum: &Datum) -> String {
    match datum {
        Datum::Atom(value) => (*value).to_owned(),
        Datum::Form(items) => {
            let rendered = items.iter().map(render_sexpr).collect::<Vec<_>>().join(" ");
            format!("({rendered})")
        }
    }
}

fn render_layout(datum: &Datum, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    let Datum::Form(items) = datum else {
        return format!("{prefix}{}", render_sexpr(datum));
    };

    if items.len() <= 1 {
        return format!("{prefix}{}", render_sexpr(datum));
    }

    let Datum::Atom(head) = &items[0] else {
        return format!("{prefix}{}", render_sexpr(datum));
    };

    let mut inline = vec![*head];
    let mut index = 1;
    while index < items.len() {
        let Datum::Atom(value) = &items[index] else {
            break;
        };
        inline.push(*value);
        index += 1;
    }

    let mut lines = vec![format!("{prefix}{}", inline.join(" "))];
    let mut continuation = vec![];
    for item in &items[index..] {
        if let Datum::Atom(value) = item {
            continuation.push(*value);
            continue;
        }
        if !continuation.is_empty() {
            lines.push(format!(
                "{}... {}",
                " ".repeat(indent + 2),
                continuation.join(" ")
            ));
            continuation.clear();
        }
        lines.push(render_layout(item, indent + 2));
    }
    if !continuation.is_empty() {
        lines.push(format!(
            "{}... {}",
            " ".repeat(indent + 2),
            continuation.join(" ")
        ));
    }
    lines.join("\n")
}

fn datums_with_nodes(nodes: usize, memo: &mut HashMap<usize, Vec<Datum>>) -> Vec<Datum> {
    if let Some(datums) = memo.get(&nodes) {
        return datums.clone();
    }

    let mut result = BTreeSet::new();
    if nodes == 1 {
        for atom in ["f", "g", "x", "y"] {
            result.insert(Datum::Atom(atom));
        }
        result.insert(Datum::Form(vec![]));
    }

    let max_arity = 3.min(nodes.saturating_sub(1));
    for arity in 1..=max_arity {
        for sizes in compositions(nodes - 1, arity) {
            let pools = sizes
                .iter()
                .map(|size| datums_with_nodes(*size, memo))
                .collect::<Vec<_>>();
            product_datums(&pools, 0, &mut vec![], &mut result);
        }
    }

    let datums = result.into_iter().collect::<Vec<_>>();
    memo.insert(nodes, datums.clone());
    datums
}

fn product_datums(
    pools: &[Vec<Datum>],
    index: usize,
    current: &mut Vec<Datum>,
    result: &mut BTreeSet<Datum>,
) {
    if index == pools.len() {
        result.insert(Datum::Form(current.clone()));
        return;
    }

    for datum in &pools[index] {
        current.push(datum.clone());
        product_datums(pools, index + 1, current, result);
        current.pop();
    }
}

fn compositions(total: usize, parts: usize) -> Vec<Vec<usize>> {
    if parts == 0 {
        return if total == 0 { vec![vec![]] } else { vec![] };
    }
    if parts == 1 {
        return if total > 0 { vec![vec![total]] } else { vec![] };
    }

    let mut result = vec![];
    for first in 1..=(total - parts + 1) {
        for mut rest in compositions(total - first, parts - 1) {
            let mut values = vec![first];
            values.append(&mut rest);
            result.push(values);
        }
    }
    result
}

struct Case {
    name: &'static str,
    source: &'static str,
    expected: Expected,
}

enum Expected {
    Ok(&'static [&'static str]),
    Err(&'static str),
}

const ADVERSARIAL_CASES: &[Case] = &[
    Case {
        name: "atom-form-atom",
        source: "f\n  ()\n  ... f",
        expected: Expected::Ok(&["(f () f)"]),
    },
    Case {
        name: "single-token-child-is-atom",
        source: "f\n  x",
        expected: Expected::Ok(&["(f x)"]),
    },
    Case {
        name: "single-token-child-with-children-is-form",
        source: "f\n  x\n    ... y",
        expected: Expected::Ok(&["(f (x y))"]),
    },
    Case {
        name: "multi-token-child-is-nested-form",
        source: "f\n  x y",
        expected: Expected::Ok(&["(f (x y))"]),
    },
    Case {
        name: "explicit-empty-form-with-child",
        source: "f ()\n  x",
        expected: Expected::Ok(&["(f () x)"]),
    },
    Case {
        name: "explicit-empty-form-as-callee",
        source: "()\n  x",
        expected: Expected::Ok(&["(() x)"]),
    },
    Case {
        name: "source-order-mixed-continuation",
        source: "f\n  g a\n  ... x\n  h b",
        expected: Expected::Ok(&["(f (g a) x (h b))"]),
    },
    Case {
        name: "multiple-continuations-preserve-order",
        source: "f\n  ... a\n  ... b\n  g c",
        expected: Expected::Ok(&["(f a b (g c))"]),
    },
    Case {
        name: "continuation-after-comment",
        source: "f\n  g a\n  # keep parent\n  ... b",
        expected: Expected::Ok(&["(f (g a) b)"]),
    },
    Case {
        name: "continuation-after-nested-dedent",
        source: "f\n  g\n    h\n  ... x",
        expected: Expected::Ok(&["(f (g h) x)"]),
    },
    Case {
        name: "nested-continuation",
        source: "outer\n  inner a\n    ... b",
        expected: Expected::Ok(&["(outer (inner a b))"]),
    },
    Case {
        name: "dedent-continuation",
        source: "outer\n  inner a\n  ... b",
        expected: Expected::Ok(&["(outer (inner a) b)"]),
    },
    Case {
        name: "continuation-after-form-preserves-order",
        source: "outer\n  inner a\n  ... b c\n  tail d",
        expected: Expected::Ok(&["(outer (inner a) b c (tail d))"]),
    },
    Case {
        name: "object-pair-with-nested-value",
        source: "obj\n  ... \"sum\"\n  + 2 2\n  ... \"label\" label",
        expected: Expected::Ok(&[r#"(obj "sum" (+ 2 2) "label" label)"#]),
    },
    Case {
        name: "object-two-nested-values",
        source: "obj\n  ... \"a\"\n  f 1\n  ... \"b\"\n  g 2",
        expected: Expected::Ok(&[r#"(obj "a" (f 1) "b" (g 2))"#]),
    },
    Case {
        name: "string-with-space-token",
        source: "print \"hello world\"",
        expected: Expected::Ok(&[r#"(print "hello world")"#]),
    },
    Case {
        name: "string-with-comment-marker",
        source: "print \"hello # world\" # trailing comment",
        expected: Expected::Ok(&[r#"(print "hello # world")"#]),
    },
    Case {
        name: "string-with-escaped-quote-and-comment",
        source: r##"print "a \"#\" b" # trailing comment"##,
        expected: Expected::Ok(&[r##"(print "a \"#\" b")"##]),
    },
    Case {
        name: "string-with-escaped-tab-is-allowed",
        source: r#"print "hello\tworld""#,
        expected: Expected::Ok(&[r#"(print "hello\tworld")"#]),
    },
    Case {
        name: "comment-only-child-line",
        source: "f\n  # comment\n  x",
        expected: Expected::Ok(&["(f x)"]),
    },
    Case {
        name: "hash-prefixed-token-is-not-comment",
        source: "f #tag",
        expected: Expected::Ok(&["(f #tag)"]),
    },
    Case {
        name: "hash-in-symbol-is-not-comment",
        source: "f foo#bar",
        expected: Expected::Ok(&["(f foo#bar)"]),
    },
    Case {
        name: "blank-line-inside-form",
        source: "f\n\n  x",
        expected: Expected::Ok(&["(f x)"]),
    },
    Case {
        name: "crlf-line-endings",
        source: "f\r\n  x\r\n",
        expected: Expected::Ok(&["(f x)"]),
    },
    Case {
        name: "empty-form-escape",
        source: "()",
        expected: Expected::Ok(&["()"]),
    },
    Case {
        name: "singleton-form-escape",
        source: "(x)",
        expected: Expected::Ok(&["(x)"]),
    },
    Case {
        name: "explicit-form-as-child",
        source: "f\n  (x)",
        expected: Expected::Ok(&["(f (x))"]),
    },
    Case {
        name: "explicit-form-as-continuation-token",
        source: "f\n  ... ()",
        expected: Expected::Ok(&["(f ())"]),
    },
    Case {
        name: "explicit-form-with-nested-string",
        source: r#"f (g "a (b) c")"#,
        expected: Expected::Ok(&[r#"(f (g "a (b) c"))"#]),
    },
    Case {
        name: "explicit-form-then-comment",
        source: "f (g h) # trailing",
        expected: Expected::Ok(&["(f (g h))"]),
    },
    Case {
        name: "form-head-escape",
        source: "((make-adder 11) 7)",
        expected: Expected::Ok(&["((make-adder 11) 7)"]),
    },
    Case {
        name: "form-head-inline-layout",
        source: "(make-adder 11) 7",
        expected: Expected::Ok(&["((make-adder 11) 7)"]),
    },
    Case {
        name: "form-head-with-layout-child",
        source: "(make-adder 11)\n  7",
        expected: Expected::Ok(&["((make-adder 11) 7)"]),
    },
    Case {
        name: "reader-unquote-form-token",
        source: "str \"x\" ,(f y)",
        expected: Expected::Ok(&[r#"(str "x" (, (f y)))"#]),
    },
    Case {
        name: "reader-splice-form-token",
        source: "str \"x\" ,@(f y)",
        expected: Expected::Ok(&[r#"(str "x" (,@ (f y)))"#]),
    },
    Case {
        name: "defn-rest-params-with-flat-body",
        source: "defn foo (a b ... rest)\n  ... body",
        expected: Expected::Ok(&["(defn foo (a b ... rest) body)"]),
    },
    Case {
        name: "fn-layout-rest-params-with-flat-body",
        source: "fn foo\n  a b ... rest\n  ... body",
        expected: Expected::Ok(&["(fn foo (a b ... rest) body)"]),
    },
    Case {
        name: "ellipsis-rest-token-inline",
        source: "fn foo\n  a b ... rest",
        expected: Expected::Ok(&["(fn foo (a b ... rest))"]),
    },
    Case {
        name: "ellipsis-token-after-continuation-marker",
        source: "f\n  ... ... rest",
        expected: Expected::Ok(&["(f ... rest)"]),
    },
    Case {
        name: "ellipsis-prefixed-token-after-marker-is-atom",
        source: "f\n  ... ...rest",
        expected: Expected::Ok(&["(f ...rest)"]),
    },
    Case {
        name: "ellipsis-inside-explicit-island",
        source: "f (... k l)",
        expected: Expected::Ok(&["(f (... k l))"]),
    },
    Case {
        name: "multiple-top-level-datums",
        source: "f\n\ng",
        expected: Expected::Ok(&["f", "g"]),
    },
    Case {
        name: "continuation-without-parent",
        source: "... x",
        expected: Expected::Err("has no parent form"),
    },
    Case {
        name: "empty-continuation",
        source: "f\n  ...",
        expected: Expected::Err("must provide at least one token"),
    },
    Case {
        name: "empty-continuation-before-comment",
        source: "f\n  ... # comment",
        expected: Expected::Err("must provide at least one token"),
    },
    Case {
        name: "continuation-with-child",
        source: "f\n  ... x\n    y",
        expected: Expected::Err("cannot jump more than one level"),
    },
    Case {
        name: "nested-continuation-under-continuation",
        source: "f\n  ... x\n    ... y",
        expected: Expected::Err("cannot jump more than one level"),
    },
    Case {
        name: "top-level-continuation-after-form",
        source: "f\n... x",
        expected: Expected::Err("has no parent form"),
    },
    Case {
        name: "line-leading-ellipsis-prefix",
        source: "f\n  ...rest",
        expected: Expected::Err("ellipsis-like"),
    },
    Case {
        name: "line-leading-four-dots",
        source: "f\n  .... rest",
        expected: Expected::Err("ellipsis-like"),
    },
    Case {
        name: "odd-indent",
        source: "f\n x",
        expected: Expected::Err("multiples of two spaces"),
    },
    Case {
        name: "indent-jump",
        source: "f\n    x",
        expected: Expected::Err("cannot jump more than one level"),
    },
    Case {
        name: "tab-indent",
        source: "f\n\tx",
        expected: Expected::Err("tabs are not allowed"),
    },
    Case {
        name: "tab-between-tokens",
        source: "f\tx",
        expected: Expected::Err("tabs are not allowed"),
    },
    Case {
        name: "tab-before-comment-is-rejected",
        source: "f\t# comment",
        expected: Expected::Err("tabs are not allowed"),
    },
    Case {
        name: "nbsp-between-tokens",
        source: "f\u{00a0}x",
        expected: Expected::Err("only ASCII spaces"),
    },
    Case {
        name: "standalone-close-paren",
        source: "f )",
        expected: Expected::Err("parentheses inside ws atoms"),
    },
    Case {
        name: "close-paren-in-symbol",
        source: "f x)",
        expected: Expected::Err("parentheses inside ws atoms"),
    },
    Case {
        name: "open-paren-in-symbol",
        source: "f x(y)",
        expected: Expected::Err("parentheses inside ws atoms"),
    },
    Case {
        name: "extra-close-after-explicit",
        source: "f (x))",
        expected: Expected::Err("parentheses inside ws atoms"),
    },
    Case {
        name: "unclosed-explicit-line",
        source: "f (x y",
        expected: Expected::Err("unterminated explicit form"),
    },
    Case {
        name: "multiline-explicit-even-if-later-closed",
        source: "f (x\n  y)",
        expected: Expected::Err("unterminated explicit form"),
    },
    Case {
        name: "multiline-explicit-island-is-rejected",
        source: "defn foo (a b)\n  ... (x y z\n    ... k l",
        expected: Expected::Err("unterminated explicit form"),
    },
    Case {
        name: "user-sample-with-one-space-indent-is-rejected-earlier",
        source: "defn foo (a b)\n ... (x y z\n   ... k l",
        expected: Expected::Err("multiples of two spaces"),
    },
    Case {
        name: "unterminated-string",
        source: "print \"hello",
        expected: Expected::Err("unterminated string literal"),
    },
    Case {
        name: "bare-marker-atom-needs-escape",
        source: "...",
        expected: Expected::Err("has no parent form"),
    },
];

use jisp_core::{Node, SourceId, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;

struct PortableTest {
    name: String,
    expected: String,
    actual: String,
}

#[test]
fn portable_lisp_language_tests_pass() {
    for (file, source) in [
        (
            "tests/language/list-pipeline.lisp",
            include_str!("../../../tests/language/list-pipeline.lisp"),
        ),
        (
            "tests/language/result-case-pipeline.lisp",
            include_str!("../../../tests/language/result-case-pipeline.lisp"),
        ),
    ] {
        run_fixture(file, source);
    }
}

fn run_fixture(file: &str, source: &str) {
    let nodes = LispParser
        .parse_module(SourceId(0), source)
        .unwrap_or_else(|error| panic!("{file}: parse failed: {error}"));
    let (module_nodes, tests) = collect_tests(nodes);
    assert!(!tests.is_empty(), "{file}: expected at least one test");

    let module = Lowerer
        .lower_module(&module_nodes)
        .unwrap_or_else(|error| panic!("{file}: lower failed: {error}"));
    let loaded = Evaluator::new()
        .load_module(&module)
        .unwrap_or_else(|error| panic!("{file}: evaluation failed: {error}"));

    for test in tests {
        let expected = loaded
            .exports
            .get(&test.expected)
            .unwrap_or_else(|| panic!("{file}: missing export {}", test.expected));
        let actual = loaded
            .exports
            .get(&test.actual)
            .unwrap_or_else(|| panic!("{file}: missing export {}", test.actual));
        let equal = expected
            .structurally_equal(actual)
            .unwrap_or_else(|error| panic!("{file}: {} failed: {error}", test.name));
        assert!(
            equal,
            "{file}: {} failed: expected {}, got {}",
            test.name,
            expected.display_string(),
            actual.display_string()
        );
    }
}

fn collect_tests(nodes: Vec<Node>) -> (Vec<Node>, Vec<PortableTest>) {
    let mut module_nodes = vec![];
    let mut tests = vec![];

    for node in nodes {
        if is_test(&node) {
            let test = lower_test_form(&node, tests.len(), &mut module_nodes);
            tests.push(test);
        } else {
            module_nodes.push(node);
        }
    }

    (module_nodes, tests)
}

fn is_test(node: &Node) -> bool {
    node.as_form()
        .and_then(|items| items.first())
        .and_then(Node::as_symbol)
        == Some("test")
}

fn lower_test_form(node: &Node, index: usize, module_nodes: &mut Vec<Node>) -> PortableTest {
    let items = node.as_form().expect("test must be a form");
    assert_eq!(items.len(), 3, "test expects a name and assertion");
    let name = items[1]
        .as_string()
        .expect("test name must be a string")
        .to_owned();
    let assertion = items[2].as_form().expect("test body must be an assertion");
    assert_eq!(
        assertion.first().and_then(Node::as_symbol),
        Some("assert.equal"),
        "test body must be assert.equal"
    );
    assert_eq!(
        assertion.len(),
        3,
        "assert.equal expects expected and actual"
    );

    let expected = format!("__jisp_test_{index}_expected");
    let actual = format!("__jisp_test_{index}_actual");
    module_nodes.push(export_node(&expected, assertion[1].clone(), node));
    module_nodes.push(export_node(&actual, assertion[2].clone(), node));

    PortableTest {
        name,
        expected,
        actual,
    }
}

fn export_node(name: &str, value: Node, source: &Node) -> Node {
    Node::form(
        vec![
            Node::symbol("export", source.span),
            Node::symbol(name, source.span),
            value,
        ],
        source.span,
    )
}

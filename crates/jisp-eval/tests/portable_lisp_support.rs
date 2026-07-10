use jisp_core::{Node, SourceId, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;

struct PortableTest {
    name: String,
    expected: String,
    actual: String,
}

pub fn run_lisp_test(file: &str, source: &str, test_index: usize, test_name: &str) {
    let nodes = LispParser
        .parse_module(SourceId(0), source)
        .unwrap_or_else(|error| panic!("{file}: parse failed: {error}"));
    let (module_nodes, test) = collect_test(nodes, test_index)
        .unwrap_or_else(|| panic!("{file}: missing generated test {test_index}: {test_name}"));

    let module = Lowerer
        .lower_module(&module_nodes)
        .unwrap_or_else(|error| panic!("{file}: lower failed: {error}"));
    let loaded = Evaluator::new()
        .load_module(&module)
        .unwrap_or_else(|error| panic!("{file}: evaluation failed: {error}"));

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

fn collect_test(nodes: Vec<Node>, test_index: usize) -> Option<(Vec<Node>, PortableTest)> {
    let mut module_nodes = vec![];
    let mut selected = None;
    let mut tests_seen = 0;

    for node in nodes {
        if is_test(&node) {
            if tests_seen == test_index {
                selected = Some(lower_test_form(&node, test_index, &mut module_nodes));
            }
            tests_seen += 1;
        } else {
            module_nodes.push(node);
        }
    }

    selected.map(|test| (module_nodes, test))
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

use jisp_core::{Node, SourceId, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_lisp::LispParser;
use jisp_types::Inferencer;

struct PortableTest {
    name: String,
    expected: String,
    actual: String,
}

pub fn run_lisp_test(file: &str, source: &str, test_index: usize, test_name: &str) {
    let generated_context = format!("{file}: {test_name}");
    let nodes = LispParser
        .parse_module(SourceId(0), source)
        .unwrap_or_else(|error| panic!("{generated_context}: parse failed: {error}"));
    let (module_nodes, test) = collect_test(nodes, test_index)
        .unwrap_or_else(|error| panic!("{generated_context}: {error}"));
    let context = format!("{file}: {}", test.name);

    let module = Lowerer
        .lower_module(&module_nodes)
        .unwrap_or_else(|error| panic!("{context}: lower failed: {error}"));
    Inferencer::with_prelude()
        .infer_module(&module)
        .unwrap_or_else(|error| panic!("{context}: type check failed: {error}"));
    let loaded = Evaluator::new()
        .load_module(&module)
        .unwrap_or_else(|error| panic!("{context}: evaluation failed: {error}"));

    let expected = loaded
        .exports
        .get(&test.expected)
        .unwrap_or_else(|| panic!("{context}: missing export {}", test.expected));
    let actual = loaded
        .exports
        .get(&test.actual)
        .unwrap_or_else(|| panic!("{context}: missing export {}", test.actual));
    let equal = expected
        .structurally_equal(actual)
        .unwrap_or_else(|error| panic!("{context}: assertion failed: {error}"));
    assert!(
        equal,
        "{context}: assertion failed: expected {}, got {}",
        expected.display_string(),
        actual.display_string()
    );
}

fn collect_test(nodes: Vec<Node>, test_index: usize) -> Result<(Vec<Node>, PortableTest), String> {
    let mut module_nodes = vec![];
    let mut selected = None;
    let mut tests_seen = 0;

    for node in nodes {
        if is_test(&node) {
            if tests_seen == test_index {
                selected = Some(lower_test_form(&node, test_index, &mut module_nodes)?);
            }
            tests_seen += 1;
        } else {
            module_nodes.push(node);
        }
    }

    selected
        .map(|test| (module_nodes, test))
        .ok_or_else(|| format!("missing generated test {test_index}"))
}

fn is_test(node: &Node) -> bool {
    node.as_form()
        .and_then(|items| items.first())
        .and_then(Node::as_symbol)
        == Some("test")
}

fn lower_test_form(
    node: &Node,
    index: usize,
    module_nodes: &mut Vec<Node>,
) -> Result<PortableTest, String> {
    let items = node
        .as_form()
        .ok_or_else(|| "test must be a form".to_owned())?;
    if items.len() != 3 {
        return Err(format!(
            "test expects a name and assertion, got {} item(s)",
            items.len()
        ));
    }
    let name = items[1]
        .as_string()
        .ok_or_else(|| "test name must be a string".to_owned())?
        .to_owned();
    let assertion = items[2]
        .as_form()
        .ok_or_else(|| format!("{name}: test body must be an assertion"))?;
    if assertion.first().and_then(Node::as_symbol) != Some("assert.equal") {
        return Err(format!("{name}: test body must be assert.equal"));
    }
    if assertion.len() != 3 {
        return Err(format!(
            "{name}: assert.equal expects expected and actual, got {} item(s)",
            assertion.len()
        ));
    }

    let expected = format!("__jisp_test_{index}_expected");
    let actual = format!("__jisp_test_{index}_actual");
    module_nodes.push(export_node(&expected, assertion[1].clone(), node));
    module_nodes.push(export_node(&actual, assertion[2].clone(), node));

    Ok(PortableTest {
        name,
        expected,
        actual,
    })
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

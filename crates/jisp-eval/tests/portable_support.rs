use jisp_core::{detect_syntax, Node, SourceId, Syntax, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_ws::WsParser;
use jisp_syntax_yaml::YamlParser;
use jisp_types::Inferencer;

enum PortableTest {
    Equal {
        name: String,
        expected: String,
        actual: String,
    },
    Error {
        name: String,
        expected_message: String,
    },
}

pub fn run_portable_test(file: &str, source: &str, test_index: usize, test_name: &str) {
    let generated_context = format!("{file}: {test_name}");
    let nodes = parse_fixture(file, source)
        .unwrap_or_else(|error| panic!("{generated_context}: parse failed: {error}"));
    let expanded = jisp_expand::expand_module(&nodes)
        .unwrap_or_else(|error| panic!("{generated_context}: expand failed: {error}"));
    let (module_nodes, test) = collect_test(expanded.nodes, test_index)
        .unwrap_or_else(|error| panic!("{generated_context}: {error}"));
    let context = format!("{file}: {}", test.name());

    match test {
        PortableTest::Equal {
            expected, actual, ..
        } => run_equal_test(&context, &module_nodes, &expected, &actual),
        PortableTest::Error {
            expected_message, ..
        } => run_error_test(&context, &module_nodes, &expected_message),
    }
}

fn parse_fixture(file: &str, source: &str) -> Result<Vec<Node>, String> {
    match detect_syntax(file) {
        Some(Syntax::Json) => JsonParser
            .parse_module(SourceId(0), source)
            .map_err(|error| error.to_string()),
        Some(Syntax::Yaml) => YamlParser
            .parse_module(SourceId(0), source)
            .map_err(|error| error.to_string()),
        Some(Syntax::Lisp) => LispParser
            .parse_module(SourceId(0), source)
            .map_err(|error| error.to_string()),
        Some(Syntax::Ws) => WsParser
            .parse_module(SourceId(0), source)
            .map_err(|error| error.to_string()),
        None => Err("portable fixture has an unsupported extension".to_owned()),
    }
}

impl PortableTest {
    fn name(&self) -> &str {
        match self {
            PortableTest::Equal { name, .. } | PortableTest::Error { name, .. } => name,
        }
    }
}

fn run_equal_test(context: &str, module_nodes: &[Node], expected: &str, actual: &str) {
    let module = Lowerer
        .lower_module(module_nodes)
        .unwrap_or_else(|error| panic!("{context}: lower failed: {error}"));
    Inferencer::with_prelude()
        .infer_module(&module)
        .unwrap_or_else(|error| panic!("{context}: type check failed: {error}"));
    let loaded = Evaluator::new()
        .load_module(&module)
        .unwrap_or_else(|error| panic!("{context}: evaluation failed: {error}"));

    let expected = loaded
        .exports
        .get(expected)
        .unwrap_or_else(|| panic!("{context}: missing export {expected}"));
    let actual = loaded
        .exports
        .get(actual)
        .unwrap_or_else(|| panic!("{context}: missing export {actual}"));
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

fn run_error_test(context: &str, module_nodes: &[Node], expected_message: &str) {
    let module = match Lowerer.lower_module(module_nodes) {
        Ok(module) => module,
        Err(error) => {
            assert_error_contains(context, &lower_error_messages(&error), expected_message);
            return;
        }
    };

    match Inferencer::with_prelude().infer_module(&module) {
        Ok(_) => panic!("{context}: expected lower/type error containing `{expected_message}`"),
        Err(error) => assert_error_contains(context, &error.to_string(), expected_message),
    }
}

fn assert_error_contains(context: &str, actual: &str, expected: &str) {
    assert!(
        actual.contains(expected),
        "{context}: expected error containing `{expected}`, got `{actual}`"
    );
}

fn lower_error_messages(error: &jisp_ir::LowerError) -> String {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_test(nodes: Vec<Node>, test_index: usize) -> Result<(Vec<Node>, PortableTest), String> {
    let mut module_nodes = vec![];
    let mut selected = None;
    let mut tests_seen = 0;

    for node in nodes {
        if is_test_form(&node) {
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

fn is_test_form(node: &Node) -> bool {
    matches!(
        node.as_form()
            .and_then(|items| items.first())
            .and_then(Node::as_symbol),
        Some("test" | "test-error")
    )
}

fn lower_test_form(
    node: &Node,
    index: usize,
    module_nodes: &mut Vec<Node>,
) -> Result<PortableTest, String> {
    let items = node
        .as_form()
        .ok_or_else(|| "test must be a form".to_owned())?;
    let form_name = items
        .first()
        .and_then(Node::as_symbol)
        .ok_or_else(|| "test must start with a symbol".to_owned())?;
    if form_name == "test-error" {
        return lower_test_error_form(items, index, module_nodes);
    }

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

    Ok(PortableTest::Equal {
        name,
        expected,
        actual,
    })
}

fn lower_test_error_form(
    items: &[Node],
    index: usize,
    module_nodes: &mut Vec<Node>,
) -> Result<PortableTest, String> {
    if items.len() != 4 {
        return Err(format!(
            "test-error expects a name, expected message substring, and expression, got {} item(s)",
            items.len()
        ));
    }
    let name = items[1]
        .as_string()
        .ok_or_else(|| "test-error name must be a string".to_owned())?
        .to_owned();
    let expected_message = items[2]
        .as_string()
        .ok_or_else(|| format!("{name}: test-error expected message must be a string"))?
        .to_owned();
    if let Some(module_items) = test_error_module_items(&items[3]) {
        module_nodes.extend(module_items.iter().cloned());
    } else {
        let actual = format!("__jisp_test_{index}_error");
        module_nodes.push(export_node(&actual, items[3].clone(), &items[0]));
    }

    Ok(PortableTest::Error {
        name,
        expected_message,
    })
}

fn test_error_module_items(node: &Node) -> Option<&[Node]> {
    let items = node.as_form()?;
    if items.first().and_then(Node::as_symbol) == Some("module") {
        Some(&items[1..])
    } else {
        None
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

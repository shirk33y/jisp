use jisp_core::{detect_syntax, Node, NodeKind, SourceId, Syntax, SyntaxParser};
use jisp_eval::Evaluator;
use jisp_ir::Lowerer;
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_ws::WsParser;
use jisp_syntax_yaml::YamlParser;
use jisp_types::Inferencer;

enum PortableTest {
    Assert {
        name: String,
        condition: String,
    },
    Error {
        name: String,
        expected_message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortableTestKind {
    Assert,
    Error,
}

#[derive(Clone, Copy)]
pub struct ExpectedPortableTest {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: PortableTestKind,
}

#[derive(Clone, Copy)]
pub struct FixtureSource {
    pub file: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
enum PortableOutcome {
    AssertPassed,
    ExpectedError {
        stage: PortableFailureStage,
        code: &'static str,
    },
}

#[derive(Debug, Eq, PartialEq)]
enum PortableFailureStage {
    Lower,
    Type,
}

pub fn run_portable_test(
    file: &str,
    source: &str,
    test_index: usize,
    test_name: &str,
    test_id: &str,
) {
    let generated_context = format!("{test_id} ({file}: {test_name})");
    run_portable_test_outcome(file, source, test_index)
        .unwrap_or_else(|error| panic!("{generated_context}: {error}"));
}

pub fn assert_fixture_parity(
    canonical_fixture: &str,
    fixtures: &[FixtureSource],
    expected: &[ExpectedPortableTest],
) {
    assert_eq!(
        fixtures.len(),
        4,
        "{canonical_fixture}: expected four syntaxes"
    );
    let canonical_nodes =
        expand_fixture(fixtures[0].file, fixtures[0].source).unwrap_or_else(|error| {
            panic!(
                "{canonical_fixture}: {} did not expand for structural parity: {error}",
                fixtures[0].file
            )
        });
    for fixture in fixtures {
        let nodes = expand_fixture(fixture.file, fixture.source).unwrap_or_else(|error| {
            panic!(
                "{canonical_fixture}: {} did not expand for structural parity: {error}",
                fixture.file
            )
        });
        if let Some(difference) = first_node_difference(&canonical_nodes, &nodes, "module") {
            panic!(
                "{canonical_fixture}: {} expanded differently from {} at {difference}",
                fixture.file, fixtures[0].file
            );
        }
        let actual = fixture_test_registry(fixture.file, fixture.source).unwrap_or_else(|error| {
            panic!(
                "{canonical_fixture}: {} has invalid test registry: {error}",
                fixture.file
            )
        });
        assert_eq!(
            actual.len(),
            expected.len(),
            "{canonical_fixture}: {}",
            fixture.file
        );
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                actual.0, expected.name,
                "{canonical_fixture}: {} test {index} has a different name",
                fixture.file
            );
            assert_eq!(
                actual.1, expected.kind,
                "{canonical_fixture}: {} test {} (`{}`) has a different kind",
                fixture.file, index, expected.name
            );
        }
    }

    for (index, expected) in expected.iter().enumerate() {
        let baseline = run_portable_test_outcome(fixtures[0].file, fixtures[0].source, index)
            .unwrap_or_else(|error| {
                panic!(
                    "{}: {} ({}) failed: {error}",
                    expected.id, fixtures[0].file, expected.name
                )
            });
        for fixture in &fixtures[1..] {
            let actual = run_portable_test_outcome(fixture.file, fixture.source, index)
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: {} ({}) failed: {error}",
                        expected.id, fixture.file, expected.name
                    )
                });
            assert_eq!(
                actual, baseline,
                "{}: {} ({}) diverged from {}: {actual:?} vs {baseline:?}",
                expected.id, fixture.file, expected.name, fixtures[0].file
            );
        }
    }
}

fn first_node_difference(left: &[Node], right: &[Node], path: &str) -> Option<String> {
    if left.len() != right.len() {
        return Some(format!(
            "{path}: item count {} != {}",
            left.len(),
            right.len()
        ));
    }
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        let item_path = format!("{path}[{index}]");
        if let Some(difference) = first_node_kind_difference(left, right, &item_path) {
            return Some(difference);
        }
    }
    None
}

fn first_node_kind_difference(left: &Node, right: &Node, path: &str) -> Option<String> {
    match (&left.kind, &right.kind) {
        (NodeKind::Null, NodeKind::Null) => None,
        (NodeKind::Bool(left), NodeKind::Bool(right)) if left == right => None,
        (NodeKind::Int(left), NodeKind::Int(right)) if left == right => None,
        (NodeKind::Float(left), NodeKind::Float(right)) if left == right => None,
        (NodeKind::Symbol(left), NodeKind::Symbol(right)) if left == right => None,
        (NodeKind::String(left), NodeKind::String(right)) if left == right => None,
        (NodeKind::Form(left), NodeKind::Form(right)) => first_node_difference(left, right, path),
        _ => Some(format!("{path}: {:?} != {:?}", left.kind, right.kind)),
    }
}

fn run_portable_test_outcome(
    file: &str,
    source: &str,
    test_index: usize,
) -> Result<PortableOutcome, String> {
    let nodes = parse_fixture(file, source).map_err(|error| format!("parse failed: {error}"))?;
    let expanded =
        jisp_expand::expand_module(&nodes).map_err(|error| format!("expand failed: {error}"))?;
    let (module_nodes, test) = collect_test(expanded.nodes, test_index)
        .map_err(|error| format!("test discovery failed: {error}"))?;
    let context = format!("{file}: {}", test.name());

    match test {
        PortableTest::Assert { condition, .. } => {
            run_assert_test(&context, &module_nodes, &condition)
        }
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

fn expand_fixture(file: &str, source: &str) -> Result<Vec<Node>, String> {
    let nodes = parse_fixture(file, source).map_err(|error| format!("parse failed: {error}"))?;
    let nodes = nodes
        .iter()
        .map(normalize_quote_aliases)
        .collect::<Vec<_>>();
    jisp_expand::expand_module(&nodes)
        .map(|expanded| expanded.nodes)
        .map_err(|error| format!("expand failed: {error}"))
}

fn normalize_quote_aliases(node: &Node) -> Node {
    let NodeKind::Form(items) = &node.kind else {
        return node.clone();
    };
    let mut items = items
        .iter()
        .map(normalize_quote_aliases)
        .collect::<Vec<_>>();
    let canonical_head = items
        .first()
        .and_then(Node::as_symbol)
        .and_then(|head| match head {
            "quasiquote" => Some("`"),
            "unquote" => Some(","),
            "unquote-splicing" => Some(",@"),
            _ => None,
        });
    if let Some(canonical_head) = canonical_head {
        items[0] = Node::symbol(canonical_head, items[0].span);
    }
    Node::form(items, node.span)
}

impl PortableTest {
    fn name(&self) -> &str {
        match self {
            PortableTest::Assert { name, .. } | PortableTest::Error { name, .. } => name,
        }
    }

    fn kind(&self) -> PortableTestKind {
        match self {
            PortableTest::Assert { .. } => PortableTestKind::Assert,
            PortableTest::Error { .. } => PortableTestKind::Error,
        }
    }
}

fn run_assert_test(
    context: &str,
    module_nodes: &[Node],
    condition: &str,
) -> Result<PortableOutcome, String> {
    let module = Lowerer
        .lower_module(module_nodes)
        .map_err(|error| format!("{context}: lower failed: {error}"))?;
    Inferencer::with_prelude()
        .infer_module(&module)
        .map_err(|error| format!("{context}: type check failed: {error}"))?;
    let loaded = Evaluator::new()
        .load_module(&module)
        .map_err(|error| format!("{context}: evaluation failed: {error}"))?;
    let condition = loaded
        .exports
        .get(condition)
        .ok_or_else(|| format!("{context}: missing assertion export {condition}"))?;
    if !matches!(condition, jisp_eval::Value::Bool(true)) {
        return Err(format!(
            "{context}: assertion failed: expected true, got {}",
            condition.display_string()
        ));
    }
    Ok(PortableOutcome::AssertPassed)
}

fn run_error_test(
    context: &str,
    module_nodes: &[Node],
    expected_message: &str,
) -> Result<PortableOutcome, String> {
    let module = match Lowerer.lower_module(module_nodes) {
        Ok(module) => module,
        Err(error) => {
            assert_error_contains(context, &lower_error_messages(&error), expected_message)?;
            return Ok(PortableOutcome::ExpectedError {
                stage: PortableFailureStage::Lower,
                code: "JISP-LOWER",
            });
        }
    };

    match Inferencer::with_prelude().infer_module(&module) {
        Ok(_) => Err(format!(
            "{context}: expected lower/type error containing `{expected_message}`"
        )),
        Err(error) => {
            assert_error_contains(context, &error.to_string(), expected_message)?;
            Ok(PortableOutcome::ExpectedError {
                stage: PortableFailureStage::Type,
                code: "JISP-TYPE",
            })
        }
    }
}

fn assert_error_contains(context: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual.contains(expected) {
        Ok(())
    } else {
        Err(format!(
            "{context}: expected error containing `{expected}`, got `{actual}`"
        ))
    }
}

fn fixture_test_registry(
    file: &str,
    source: &str,
) -> Result<Vec<(String, PortableTestKind)>, String> {
    let nodes = parse_fixture(file, source)?;
    nodes
        .iter()
        .filter(|node| is_test_form(node))
        .map(|node| {
            let mut ignored = vec![];
            let test = lower_test_form(node, ignored.len(), &mut ignored)?;
            Ok((test.name().to_owned(), test.kind()))
        })
        .collect()
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
    if assertion.first().and_then(Node::as_symbol) == Some("assert") {
        if assertion.len() != 2 {
            return Err(format!("{name}: assert expects one boolean condition"));
        }
        let condition = format!("__jisp_test_{index}_assertion");
        module_nodes.push(export_node(&condition, assertion[1].clone(), node));
        return Ok(PortableTest::Assert { name, condition });
    }

    Err(format!("{name}: test body must be `(assert condition)`"))
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

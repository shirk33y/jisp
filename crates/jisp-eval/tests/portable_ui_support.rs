use jisp_core::{detect_syntax, SourceId, Syntax, SyntaxParser};
use jisp_syntax_json::JsonParser;
use jisp_syntax_lisp::LispParser;
use jisp_syntax_ws::WsParser;
use jisp_syntax_yaml::YamlParser;
use jisp_ui::testing::{run_ui_tests, split_ui_tests, UiTestSuite};

pub fn run_portable_test(file: &str, source: &str, test_index: usize, test_name: &str) {
    let context = format!("{file}: {test_name}");
    let nodes = parse_fixture(file, source)
        .unwrap_or_else(|error| panic!("{context}: parse failed: {error}"));
    let expanded = jisp_expand::expand_module(&nodes)
        .unwrap_or_else(|error| panic!("{context}: expand failed: {error}"));
    let suite = split_ui_tests(expanded.nodes)
        .unwrap_or_else(|error| panic!("{context}: invalid ui.test: {error}"));
    let test = suite
        .tests
        .get(test_index)
        .cloned()
        .unwrap_or_else(|| panic!("{context}: missing generated ui.test {test_index}"));
    assert_eq!(
        test.name, test_name,
        "{context}: generated test name drifted"
    );
    let outcomes = run_ui_tests(UiTestSuite {
        module_nodes: suite.module_nodes,
        tests: vec![test],
    })
    .unwrap_or_else(|error| panic!("{context}: setup failed: {error}"));
    let outcome = &outcomes[0];
    assert!(
        outcome.passed(),
        "{context}: {}",
        outcome
            .failure
            .as_deref()
            .unwrap_or("unknown ui.test failure")
    );
}

fn parse_fixture(file: &str, source: &str) -> Result<Vec<jisp_core::Node>, String> {
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

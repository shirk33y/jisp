use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use jisp_core::{Node, SourceId, SyntaxParser};
use jisp_syntax_lisp::LispParser;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryRow {
    id: String,
    runner: String,
    area: String,
    source_fixture: String,
    portable_test_id: Option<String>,
    backend_obligation: String,
    interpreter_result: String,
    native_status: String,
    native_expectation: String,
    notes: String,
    test_file: String,
    test_name: String,
}

#[test]
fn native_inventory_is_backed_by_fixtures_tests_and_docs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory: Vec<InventoryRow> =
        serde_json::from_str(include_str!("../../../docs/native-support.json"))
            .expect("native support inventory must be valid JSON");
    let docs = fs::read_to_string(root.join("docs/NATIVE.md")).unwrap();

    assert!(!inventory.is_empty());
    for heading in [
        "id",
        "source fixture",
        "portable test id",
        "backend obligation",
        "native expectation",
    ] {
        assert!(
            docs.contains(heading),
            "docs/NATIVE.md is missing `{heading}`"
        );
    }
    let portable_tests = portable_test_ids(&root);
    let mut linked_portable_tests = HashSet::new();

    for row in inventory {
        assert!(
            !row.runner.is_empty(),
            "{} has no conformance runner",
            row.id
        );
        assert!(!row.area.is_empty(), "{} has no area", row.id);
        assert!(
            !row.interpreter_result.is_empty(),
            "{} has no interpreter result",
            row.id
        );
        assert!(
            !row.native_expectation.is_empty(),
            "{} has no native expectation",
            row.id
        );
        assert!(!row.notes.is_empty(), "{} has no notes", row.id);
        match row.backend_obligation.as_str() {
            "supported" => assert_eq!(row.native_status, "supported", "{}", row.id),
            "intentionally-rejected" => assert_eq!(row.native_status, "rejected", "{}", row.id),
            "interpreter-only" => assert_eq!(row.native_status, "not-applicable", "{}", row.id),
            other => panic!("{} has an invalid backend obligation `{other}`", row.id),
        }
        assert!(
            root.join(&row.source_fixture).is_file(),
            "{} fixture is missing",
            row.id
        );
        if row.backend_obligation != "interpreter-only" {
            let test = fs::read_to_string(root.join(&row.test_file)).unwrap();
            assert!(
                test.contains(&format!("fn {}", row.test_name)),
                "{} does not name its owning test {}",
                row.id,
                row.test_name
            );
        }
        if let Some(portable_test_id) = row.portable_test_id {
            assert!(
                portable_tests.contains(&portable_test_id),
                "{} links missing portable test `{portable_test_id}`",
                row.id
            );
            assert!(
                linked_portable_tests.insert(portable_test_id.clone()),
                "portable test `{portable_test_id}` is linked by more than one inventory row"
            );
        } else {
            assert!(
                row.notes.starts_with("native-only:"),
                "{} has no portable test ID and must explain its native-only coverage",
                row.id
            );
        }
        assert!(
            docs.contains(&format!("| {} |", row.id)),
            "{} is absent from docs/NATIVE.md",
            row.id
        );
    }
}

fn portable_test_ids(root: &Path) -> HashSet<String> {
    let fixtures = root.join("tests/language");
    let mut ids = HashSet::new();
    for entry in fs::read_dir(&fixtures).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("lisp") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let nodes = LispParser.parse_module(SourceId(0), &source).unwrap();
        for node in nodes {
            let Some(items) = node.as_form() else {
                continue;
            };
            if !matches!(
                items.first().and_then(Node::as_symbol),
                Some("test" | "test-error")
            ) {
                continue;
            }
            let name = items
                .get(1)
                .and_then(Node::as_string)
                .expect("portable test must have a string name");
            let relative = path.strip_prefix(root).unwrap().to_string_lossy();
            let id = format!("{}::{name}", relative.replace('\\', "/"));
            assert!(ids.insert(id.clone()), "duplicate portable test ID `{id}`");
        }
    }
    assert!(!ids.is_empty(), "portable test registry is empty");
    ids
}

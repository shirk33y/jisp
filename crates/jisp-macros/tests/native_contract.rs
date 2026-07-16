use std::{fs, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryRow {
    id: String,
    area: String,
    source_fixture: String,
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
        "area",
        "source fixture",
        "interpreter result",
        "native status",
        "native expectation",
    ] {
        assert!(
            docs.contains(heading),
            "docs/NATIVE.md is missing `{heading}`"
        );
    }
    for row in inventory {
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
        assert!(
            matches!(row.native_status.as_str(), "supported" | "rejected"),
            "{} has an invalid native status",
            row.id
        );
        assert!(
            root.join(&row.source_fixture).is_file(),
            "{} fixture is missing",
            row.id
        );
        let test = fs::read_to_string(root.join(&row.test_file)).unwrap();
        assert!(
            test.contains(&format!("fn {}", row.test_name)),
            "{} does not name its owning test {}",
            row.id,
            row.test_name
        );
        assert!(
            docs.contains(&format!("| {} |", row.id)),
            "{} is absent from docs/NATIVE.md",
            row.id
        );
    }
}

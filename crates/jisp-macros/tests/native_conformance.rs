use std::collections::HashSet;

use serde::Deserialize;

#[path = "native_compile_fail.rs"]
mod native_compile_fail;
#[path = "native_differential.rs"]
mod native_differential;
#[path = "native_examples.rs"]
mod native_examples;
#[path = "native_import.rs"]
mod native_import;

#[derive(Deserialize)]
struct InventoryRow {
    id: String,
    runner: String,
    backend_obligation: String,
}

#[test]
fn native_inventory_runs_declared_obligations() {
    let inventory: Vec<InventoryRow> =
        serde_json::from_str(include_str!("../../../docs/native-support.json"))
            .expect("native support inventory must be valid JSON");
    let mut runners = HashSet::new();

    for row in inventory {
        if row.backend_obligation == "interpreter-only" {
            continue;
        }
        assert!(
            runners.insert(row.runner.clone()),
            "conformance runner `{}` is assigned more than once",
            row.runner
        );
        run_obligation(&row);
    }
}

fn run_obligation(row: &InventoryRow) {
    match row.runner.as_str() {
        "scalars" => native_differential::native_scalars_match_the_interpreter(),
        "strings-lists" => native_differential::native_strings_and_lists_match_the_interpreter(),
        "list-callbacks" => {
            native_differential::native_callbacks_and_list_higher_order_helpers_match_the_interpreter()
        }
        "closed-objects" => native_differential::native_static_object_get_matches_the_interpreter(),
        "collection-snapshots" => native_differential::native_collection_updates_preserve_their_inputs(),
        "homogeneous-dynamic-objects" => {
            native_differential::native_dynamic_reads_on_homogeneous_closed_objects_match_the_interpreter()
        }
        "maps" => native_differential::native_homogeneous_maps_match_the_interpreter(),
        "list-get-boundaries" => {
            native_differential::native_list_get_boundary_matches_the_interpreter()
        }
        "list-slice-boundaries" => {
            native_differential::native_list_slice_boundary_matches_the_interpreter()
        }
        "empty-list-callbacks" => {
            native_differential::native_empty_list_callbacks_match_the_interpreter()
        }
        "object-view-helpers" => {
            native_differential::native_object_view_helpers_match_the_interpreter()
        }
        "map-view-helpers" => {
            native_differential::native_map_view_helpers_match_the_interpreter()
        }
        "pattern-fallback" => native_differential::native_pattern_fallback_matches_the_interpreter(),
        "functions-closures" => native_differential::native_local_closures_match_the_interpreter(),
        "variadics" => native_differential::native_variadic_functions_match_the_interpreter(),
        "imports" => native_import::imported_native_output_matches_the_interpreter(),
        "macros" => native_examples::macro_normalizer_matches_the_interpreter(),
        "result-helpers" => native_differential::native_result_helpers_match_the_interpreter(),
        "option-case" => native_differential::native_option_cases_match_the_interpreter(),
        "enum-case" => native_differential::native_enum_cases_match_the_interpreter(),
        "patterns" => native_differential::native_nested_alternative_patterns_match_the_interpreter(),
        "bigint" => native_differential::native_bigints_match_the_interpreter(),
        "ui-values" => native_compile_fail::ui_html_native_values_fail_during_downstream_macro_expansion(),
        "heterogeneous-dynamic-objects" => {
            native_compile_fail::heterogeneous_dynamic_object_access_fails_during_downstream_macro_expansion()
        }
        "open-row-polymorphism" => {
            native_compile_fail::polymorphic_open_row_definition_fails_during_downstream_macro_expansion()
        }
        runner => panic!("{} has unknown conformance runner `{runner}`", row.id),
    }
}

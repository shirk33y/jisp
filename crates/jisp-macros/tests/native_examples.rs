use std::{fs, path::PathBuf};

use jisp::{
    jisp_core::{SourceId, Span},
    jisp_eval::{Evaluator, Value},
};

mod task_pipeline {
    jisp_macros::lisp_file!("../../examples/task-pipeline/main.lisp");
}

mod pricing_rules {
    jisp_macros::lisp_file!("../../examples/pricing-rules/main.lisp");
}

mod rust_embedded_report {
    jisp_macros::lisp_file!("../../examples/rust-embedded-report/main.lisp");
}

mod macro_normalizer {
    jisp_macros::lisp_file!("../../examples/macro-normalizer/main.lisp");
}

mod collection_toolbox {
    jisp_macros::lisp_file!("../../examples/collection-toolbox/main.lisp");
}

#[test]
fn task_pipeline_matches_the_interpreter() {
    assert_matches_interpreter(
        "examples/task-pipeline/main.lisp",
        Value::Int(task_pipeline::main()),
    );
}

#[test]
fn pricing_rules_matches_the_interpreter() {
    assert_matches_interpreter(
        "examples/pricing-rules/main.lisp",
        Value::Int(pricing_rules::main()),
    );
}

#[test]
fn rust_embedded_report_matches_the_interpreter() {
    assert_matches_interpreter(
        "examples/rust-embedded-report/main.lisp",
        Value::Int(rust_embedded_report::main()),
    );
}

#[test]
pub fn macro_normalizer_matches_the_interpreter() {
    assert_matches_interpreter(
        "examples/macro-normalizer/main.lisp",
        Value::Int(macro_normalizer::main()),
    );
}

#[test]
fn collection_toolbox_matches_the_interpreter() {
    assert_matches_interpreter(
        "examples/collection-toolbox/main.lisp",
        Value::Int(collection_toolbox::main()),
    );
}

fn assert_matches_interpreter(relative_path: &str, native: Value) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(relative_path);
    let text = fs::read_to_string(&path).unwrap();
    let module = jisp::evaluate(&path, &text).unwrap();
    let entry = module.exports.get("main").unwrap().clone();
    let interpreted = Evaluator::new()
        .apply(entry, &[], Span::empty(SourceId(0), 0))
        .unwrap();

    assert!(
        interpreted.structurally_equal(&native).unwrap(),
        "{relative_path} diverged: interpreter={}, native={}",
        interpreted.display_string(),
        native.display_string(),
    );
}

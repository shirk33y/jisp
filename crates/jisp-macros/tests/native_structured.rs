use std::{fs, path::PathBuf};

use jisp::{
    jisp_core::{SourceId, Span},
    jisp_eval::{Evaluator, Value},
};

jisp_macros::lisp_file!("tests/fixtures/structured.lisp");

#[test]
fn lisp_file_compiles_structural_native_programs() {
    assert_eq!(structured_entry(), 45);
}

#[test]
fn structural_native_output_matches_the_interpreter() {
    assert_eq!(
        structured_entry(),
        interpreted_entry("tests/fixtures/structured.lisp", "structured-entry")
    );
}

fn interpreted_entry(relative_path: &str, export: &str) -> i64 {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let text = fs::read_to_string(&path).unwrap();
    let module = jisp::evaluate(&path, &text).unwrap();
    let entry = module.exports.get(export).unwrap().clone();
    let value = Evaluator::new()
        .apply(entry, &[], Span::empty(SourceId(0), 0))
        .unwrap();
    let Value::Int(value) = value else {
        panic!("expected {export} to return int");
    };
    value
}

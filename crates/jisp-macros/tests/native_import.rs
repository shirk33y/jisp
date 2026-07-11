use std::{fs, path::PathBuf};

use jisp::{
    jisp_core::{SourceId, Span},
    jisp_eval::{Evaluator, Value},
};

jisp_macros::lisp_file!("tests/fixtures/imports/main.lisp");

#[test]
fn lisp_file_compiles_native_imports() {
    assert_eq!(imported_entry(), 42);
}

#[test]
fn imported_native_output_matches_the_interpreter() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/imports/main.lisp");
    let text = fs::read_to_string(&path).unwrap();
    let module = jisp::evaluate(&path, &text).unwrap();
    let entry = module.exports.get("imported-entry").unwrap().clone();
    let value = Evaluator::new()
        .apply(entry, &[], Span::empty(SourceId(0), 0))
        .unwrap();

    assert!(matches!(value, Value::Int(42)));
    assert_eq!(imported_entry(), 42);
}

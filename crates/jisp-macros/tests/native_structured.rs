jisp_macros::lisp_file!("tests/fixtures/structured.lisp");

#[test]
fn lisp_file_compiles_structural_native_programs() {
    assert_eq!(structured_entry(), 45);
}

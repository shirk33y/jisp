jisp_macros::lisp_file!("tests/fixtures/imports/main.lisp");

#[test]
fn lisp_file_compiles_native_imports() {
    assert_eq!(imported_entry(), 42);
}

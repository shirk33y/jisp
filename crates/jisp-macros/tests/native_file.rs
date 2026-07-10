jisp_macros::lisp_file!("tests/fixtures/native.lisp");

#[test]
fn lisp_file_emits_native_items() {
    assert_eq!(entry(), 42);
}

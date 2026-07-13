#[test]
fn lisp_expr_emits_a_typed_rust_expression() {
    let value: i64 = jisp_macros::lisp_expr!("tests/fixtures/expression.lisp");

    assert_eq!(value, 42);
}

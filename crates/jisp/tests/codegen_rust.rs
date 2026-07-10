use jisp::jisp_core::Syntax;

#[test]
fn emit_rust_detailed_emits_native_tokens_for_typed_functions() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def answer (fn () 42))
(export main (fn () (answer)))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("fn answer () -> i64"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("answer ()"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_native_prelude_operators() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export between
  (fn (low high value)
    (and (<= low value) (< value (+ high 1)))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn between (low : i64 , high : i64 , value : i64) -> bool"));
    assert!(tokens.contains("(low <= value)"));
    assert!(tokens.contains("(value < (high + 1i64))"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_rejects_unsupported_shapes_without_runtime_fallback() {
    let error = match jisp::emit_rust_as_detailed("main.lisp", Syntax::Lisp, "(def xs (list 1))") {
        Ok(_) => panic!("expected unsupported native codegen shape"),
        Err(error) => error.error,
    };

    assert!(matches!(error, jisp::Error::Codegen(_)), "{error}");
    assert!(error.to_string().contains("list expressions"), "{error}");
}

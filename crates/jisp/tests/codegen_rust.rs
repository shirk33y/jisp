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
fn emit_rust_detailed_emits_list_literals_as_vecs() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main (fn () (list (+ 1 1) 3)))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main () -> Vec < i64 >"));
    assert!(tokens.contains("vec ! [(1i64 + 1i64) , 3i64]"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_closed_objects_as_native_structs() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def stats (obj (str "active") true (str "age") 42))
(export main (fn () (. stats (str "age"))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub struct JispObject0"));
    assert!(tokens.contains("pub active : bool"));
    assert!(tokens.contains("pub age : i64"));
    assert!(tokens.contains("fn stats () -> JispObject0"));
    assert!(tokens.contains("JispObject0 { active : true , age : 42i64 }"));
    assert!(tokens.contains("stats () . age"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_string_templates() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main (fn () (str "Hello " ,(str "Ada"))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main () -> String"));
    assert!(tokens.contains("fragments . push (String :: from (\"Hello \"))"));
    assert!(tokens.contains("fragments . push"));
    assert!(tokens.contains("fragments . concat ()"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_simple_case_patterns() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main
  (fn (flag)
    (case flag
      (true 1)
      (false 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main (flag : bool) -> i64"));
    assert!(tokens.contains("let __jisp_case_subject = flag"));
    assert!(tokens.contains("if __jisp_case_subject == true"));
    assert!(tokens.contains("else { if __jisp_case_subject == false"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_rejects_unsupported_shapes_without_runtime_fallback() {
    let error = match jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        "(export main (fn () (fn () 1)))",
    ) {
        Ok(_) => panic!("expected unsupported native codegen shape"),
        Err(error) => error.error,
    };

    assert!(matches!(error, jisp::Error::Codegen(_)), "{error}");
    assert!(
        error.to_string().contains("function value types"),
        "{error}"
    );
}

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
fn emit_rust_detailed_emits_native_prelude_helpers() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def words (str.split (str "a,b,c") (str ",")))
(def padded (str.trim (str "  hi  ")))
(def label (str.cat padded (str ":") (str.join (str "-") words)))
(def items (list.append (list.prepend (str "z") words) (str "d")))

(export main
  (fn ()
    (if (and (str.starts label (str "hi"))
             (list.has items (str "b")))
      (+ (str.len label) (list.len (list.rest items)))
      0)))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains(". split"));
    assert!(tokens.contains(". trim () . to_owned ()"));
    assert!(tokens.contains(". concat ()"));
    assert!(tokens.contains(". join"));
    assert!(tokens.contains(". insert (0usize"));
    assert!(tokens.contains(". push"));
    assert!(tokens.contains(". starts_with"));
    assert!(tokens.contains(". contains"));
    assert!(tokens.contains(". get (1usize ..) . unwrap_or_default () . to_vec ()"));
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
fn emit_rust_detailed_emits_native_enum_case() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(type result
  (ok int)
  (err str))

(export main
  (fn ()
    (case (ok 41)
      ((ok value) (+ value 1))
      ((err _) 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub enum JispEnum0"));
    assert!(tokens.contains("Ok (i64)"));
    assert!(tokens.contains("Err (String)"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("match __jisp_case_subject"));
    assert!(tokens.contains("JispEnum0 :: Ok (value) => { (value + 1i64) }"));
    assert!(tokens.contains("JispEnum0 :: Err (_) => { 0i64 }"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_native_list_case_patterns() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main
  (fn ()
    (case (list 1 41 99)
      ((list 1 value ... tail) (+ value 1))
      (_ 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("__jisp_case_subject . len () >= 2usize"));
    assert!(tokens.contains("__jisp_case_subject [0usize] == 1i64"));
    assert!(tokens.contains("let value = __jisp_case_subject [1usize] . clone ()"));
    assert!(tokens.contains("let tail = __jisp_case_subject [2usize ..] . to_vec ()"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_native_object_case_patterns() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def stats (obj (str "active") true (str "age") 41))

(export main
  (fn ()
    (case stats
      ((obj "active" true "age" age) (+ age 1))
      (_ 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub struct JispObject0"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("__jisp_case_subject . active == true"));
    assert!(tokens.contains("let age = __jisp_case_subject . age . clone ()"));
    assert!(tokens.contains("(age + 1i64)"));
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

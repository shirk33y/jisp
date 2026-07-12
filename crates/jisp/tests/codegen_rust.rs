use std::fs;
use std::path::PathBuf;

use jisp::jisp_core::Syntax;
use jisp::RustItemKind;

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
fn emit_rust_detailed_emits_native_variadic_function_abi() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def sum-rest
  (fn (head ... tail)
    (+ head (list.fold (fn (total value) (+ total value)) 0 tail))))
(export main (fn () (sum-rest 40 1 1)))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("fn sum_rest (head : i64 , tail : Vec < i64 >) -> i64"));
    assert!(tokens.contains("sum_rest (40i64 , vec ! [1i64 , 1i64])"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_maps_generated_functions_to_source_spans() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(def answer (fn () 42))
(export main (fn () (answer)))
"#,
    )
    .unwrap();

    let item = generated
        .source_map
        .item(RustItemKind::Function, "main")
        .unwrap();
    let source_text = generated.sources.span_text(item.source_span).unwrap();

    assert_eq!(item.rust_name, "main");
    assert!(source_text.contains("export main"));
    assert!(source_text.contains("answer"));
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
(def words (str.split "a,b,c" ","))
(def padded (str.trim "  hi  "))
(def label (str.cat padded ":" (str.join "-" words)))
(def items (list.append (list.prepend "z" words) "d"))

(export main
  (fn ()
    (if (and (str.starts label "hi")
             (list.has items "b"))
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
(def stats (obj "active" true "age" 42))
(export main (fn () (. stats "age")))
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
fn emit_rust_detailed_maps_generated_structs_and_enums_to_source_spans() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(type result
  (ok int)
  (err str))

(def stats (obj "active" true "age" 42))
(export main (fn () (. stats "age")))
"#,
    )
    .unwrap();

    let object = generated
        .source_map
        .item(RustItemKind::Struct, "JispObject0")
        .unwrap();
    let object_text = generated.sources.span_text(object.source_span).unwrap();
    assert!(object_text.contains("def stats"));
    assert!(object_text.contains("active"));

    let result = generated
        .source_map
        .item(RustItemKind::Enum, "JispEnum0")
        .unwrap();
    let enum_text = generated.sources.span_text(result.source_span).unwrap();
    assert!(enum_text.contains("type result"));
    assert!(enum_text.contains("err str"));
}

#[test]
fn emit_rust_detailed_emits_string_templates() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main (fn () (str "Hello " ,"Ada")))
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
fn emit_rust_detailed_emits_native_enum_alias_case() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(type response
  (ok int)
  (err int))

(export main
  (fn ()
    (case (ok 7)
      ((as (ok value) whole) value)
      ((err _) 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("whole @ JispEnum0 :: Ok (value)"));
    assert!(!tokens.contains("Value"));
}

#[test]
fn emit_rust_detailed_emits_native_case_guards() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main
  (fn ()
    (case 7
      ((when value (> value 10)) 1)
      (_ 2))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("let value = __jisp_case_subject . clone ()"));
    assert!(tokens.contains("if (value > 10i64)"));
}

#[test]
fn emit_rust_detailed_emits_native_variant_or_patterns() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(type response
  (ok int)
  (pending int)
  (err int))

(export main
  (fn ()
    (case (pending 7)
      ((or (ok value) (pending value)) (+ value 1))
      ((err _) 0))))
"#,
    )
    .unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("JispEnum0 :: Ok (value) | JispEnum0 :: Pending (value)"));
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
(def stats (obj "active" true "age" 41))

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
fn emit_rust_detailed_emits_static_object_helpers() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/object_helpers.lisp");
    let text = fs::read_to_string(&path).unwrap();
    let generated = jisp::emit_rust_detailed(&path, &text).unwrap();

    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("fn renamed () -> JispObject"));
    assert!(tokens.contains("name :"));
    assert!(tokens.contains("String :: from (\"Grace\")"));
    assert!(tokens.contains("fn public_profile () -> JispObject"));
    assert!(tokens.contains("age : __jisp_object . age . clone ()"));
    assert!(tokens.contains("fn combined () -> JispObject"));
    assert!(tokens.contains("active : __jisp_object_1 . active . clone ()"));
    assert!(tokens.contains("vec ! [String :: from (\"active\")"));
    assert!(tokens.contains("vec ! [__jisp_object . end . clone ()"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_ui_data_shape_as_static_structs() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/ui_button.lisp");
    let text = fs::read_to_string(&path).unwrap();
    let generated = jisp::emit_rust_detailed(&path, &text).unwrap();
    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main () -> JispObject"));
    assert!(tokens.contains("pub children : Vec < JispObject"));
    assert!(tokens.contains("pub classes : JispObject"));
    assert!(tokens.contains("pub bg_emerald_600 : bool"));
    assert!(tokens.contains("pub opacity_50 : bool"));
    assert!(tokens.contains("pub px_4 : bool"));
    assert!(tokens.contains("pub py_2 : bool"));
    assert!(tokens.contains("pub tag : String"));
    assert!(tokens.contains("pub value : String"));
    assert!(tokens.contains("id :"));
    assert!(tokens.contains("String :: from (\"save-button\")"));
    assert!(tokens.contains("title : blog_title ()"));
    assert!(tokens.contains("bg_emerald_600 : (user_active () && ! saving ())"));
    assert!(tokens.contains("children : vec ! [JispObject"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_captured_closures_without_runtime_fallback() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        "(export main (fn () (let (offset 1) ((fn (value) (+ value offset)) 41))))",
    )
    .unwrap();
    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("Rc :: new (move | value : i64 | -> i64"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_bigint_without_runtime_fallback() {
    let generated = jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"(export main (fn () (+ (bigint "32849384983498230592309502398509388908203986232306") (bigint "2"))))"#,
    )
    .unwrap();
    let tokens = generated.tokens.to_string();

    assert!(tokens.contains("pub fn main () -> :: num_bigint :: BigInt"));
    assert!(tokens.contains("BigInt :: parse_bytes"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_rejects_dynamic_reads_on_heterogeneous_objects() {
    let error = match jisp::emit_rust_as_detailed(
        "main.lisp",
        Syntax::Lisp,
        r#"
(export main
  (fn ()
    (let (key (str.cat "a" ""))
      (+ (. (obj "a" 1 "b" true) key) 1))))
"#,
    ) {
        Ok(_) => panic!("expected heterogeneous dynamic object read to be rejected"),
        Err(error) => error.error,
    };

    assert!(matches!(error, jisp::Error::Codegen(_)), "{error}");
    assert!(
        error
            .to_string()
            .contains("dynamic native access on heterogeneous object"),
        "{error}"
    );
}

#[test]
fn emit_rust_detailed_emits_native_file_imports() {
    let dir = fixture_dir("native-file-imports");
    let main = dir.join("main.lisp");
    let math = dir.join("math.lisp");
    fs::write(
        &math,
        r#"
(def double (fn (value) (* value 2)))
(export inc (fn (value) (+ (double value) 1)))
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import math "math")
(export main (fn () (math.inc 20)))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let generated = jisp::emit_rust_detailed(&main, &text).unwrap();
    let tokens = generated.tokens.to_string();

    assert_eq!(generated.dependencies, vec![math.canonicalize().unwrap()]);
    assert!(tokens.contains("fn math_double (value : i64) -> i64"));
    assert!(tokens.contains("fn math_inc (value : i64) -> i64"));
    assert!(tokens.contains("math_double (value)"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("math_inc (20i64)"));
    assert!(!tokens.contains("pub fn math_inc"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

#[test]
fn emit_rust_detailed_emits_native_transitive_imports() {
    let dir = fixture_dir("native-transitive-imports");
    let main = dir.join("main.lisp");
    let app = dir.join("app.lisp");
    let math = dir.join("math.lisp");
    fs::write(&math, "(export inc (fn (value) (+ value 1)))").unwrap();
    fs::write(
        &app,
        r#"
(import math "math")
(def shifted (fn (value) (math.inc value)))
(export answer (shifted 41))
"#,
    )
    .unwrap();
    fs::write(
        &main,
        r#"
(import app "app")
(export main (fn () app.answer))
"#,
    )
    .unwrap();

    let text = fs::read_to_string(&main).unwrap();
    let generated = jisp::emit_rust_detailed(&main, &text).unwrap();
    let tokens = generated.tokens.to_string();

    assert_eq!(
        generated.dependencies,
        vec![app.canonicalize().unwrap(), math.canonicalize().unwrap()]
    );
    assert!(tokens.contains("fn app_math_inc (value : i64) -> i64"));
    assert!(tokens.contains("fn app_shifted (value : i64) -> i64"));
    assert!(tokens.contains("fn app_answer () -> i64"));
    assert!(tokens.contains("app_math_inc (value)"));
    assert!(tokens.contains("pub fn main () -> i64"));
    assert!(tokens.contains("app_answer ()"));
    assert!(!tokens.contains("pub fn app_answer"));
    assert!(!tokens.contains("Value"));
    assert!(!tokens.contains("jisp_eval"));
}

fn fixture_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/jisp-codegen-fixtures")
        .join(format!("{}-{}", name, std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

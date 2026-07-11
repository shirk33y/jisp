use jisp::jisp_types::{InferError, Type};

#[test]
fn run_main_rejects_type_errors_in_dead_code() {
    let error = jisp::run_main(
        "main.lisp",
        r#"
(export main
  (fn ()
    (if false
      (+ 1 true)
      42)))
"#,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        jisp::Error::Type(InferError::NoMatchingOverload { .. })
    ));
}

#[test]
fn run_main_requires_main_to_be_exported() {
    let error = jisp::run_main("main.lisp", "(def main (fn () 42))").unwrap_err();

    assert!(matches!(error, jisp::Error::MainNotExported));
}

#[test]
fn run_main_requires_exported_main_to_be_defined() {
    let error = jisp::run_main("main.lisp", "(export main)").unwrap_err();

    assert!(matches!(error, jisp::Error::MainNotDefined));
}

#[test]
fn run_main_requires_main_to_be_a_function() {
    let error = jisp::run_main("main.lisp", "(export main 42)").unwrap_err();

    assert!(matches!(error, jisp::Error::InvalidMainType(Type::Int)));
}

#[test]
fn run_main_requires_main_to_take_no_parameters() {
    let error = jisp::run_main("main.lisp", "(export main (fn (value) value))").unwrap_err();

    assert!(matches!(
        error,
        jisp::Error::InvalidMainType(Type::Function { parameters, rest: None, .. })
            if parameters.len() == 1
    ));
}

#[test]
fn type_errors_render_against_the_failing_expression() {
    let error = match jisp::check_detailed(
        "main.lisp",
        r#"
(export main
  (fn ()
    (+ 1 true)))
"#,
    ) {
        Ok(_) => panic!("expected a type error"),
        Err(error) => error,
    };

    let rendered = error.render_diagnostics().unwrap();
    assert!(rendered.contains("error[JISP-TYPE]"), "{rendered}");
    assert!(rendered.contains("--> main.lisp:4:5"), "{rendered}");
    assert!(rendered.contains("no overload of `+`"), "{rendered}");
    assert!(rendered.contains("(fn (int int) int)"), "{rendered}");
}

#[test]
fn runtime_errors_render_source_and_evaluation_frames() {
    let error = jisp::run_main_detailed(
        "main.lisp",
        r#"
(def divide-by-zero
  (fn () (/ 1 0)))

(export main
  (fn () (divide-by-zero)))
"#,
    )
    .unwrap_err();

    let rendered = error.render_diagnostics().unwrap();
    assert!(rendered.contains("error[JISP-RUNTIME]"), "{rendered}");
    assert!(rendered.contains("--> main.lisp:3:10"), "{rendered}");
    assert!(rendered.contains("division by zero"), "{rendered}");
    assert!(
        rendered.contains("while evaluating this expression"),
        "{rendered}"
    );
}

#[test]
fn emit_rust_type_errors_render_against_jisp_source() {
    let error = match jisp::emit_rust_as_detailed(
        "main.lisp",
        jisp::jisp_core::Syntax::Lisp,
        "(export main (fn () (+ 1 true)))",
    ) {
        Ok(_) => panic!("expected a type error"),
        Err(error) => error,
    };

    let rendered = error.render_diagnostics().unwrap();
    assert!(rendered.contains("error[JISP-TYPE]"), "{rendered}");
    assert!(rendered.contains("--> main.lisp:1:21"), "{rendered}");
    assert!(rendered.contains("no overload of `+`"), "{rendered}");
}

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

    assert!(matches!(error, jisp::Error::Type(InferError::Unify(_))));
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

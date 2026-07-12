#[test]
fn run_main_expands_lisp_quasiquote_before_lowering() {
    let value = jisp::run_main(
        "quasiquote.lisp",
        r#"
(export main
  (fn ()
    `(list 1 ,(+ 1 1) ,@(quote (3 4)))))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "[1, 2, 3, 4]");
}

#[test]
fn parse_tracks_quote_expansion_origins() {
    let parsed = jisp::parse(
        "quote.lisp",
        r#"
(export main
  (fn ()
    (quote (list 1))))
"#,
    )
    .unwrap();

    assert!(!parsed.expansion_map.is_empty());
}

#[test]
fn detailed_errors_render_quote_expansion_origin() {
    let error = match jisp::parse_detailed(
        "bad-quote.lisp",
        r#"
(export main
  (fn ()
    (quote (let))))
"#,
    ) {
        Ok(_) => panic!("expected quoted invalid syntax to fail after expansion"),
        Err(error) => error,
    };

    let rendered = error.render_diagnostics().unwrap();

    assert!(rendered.contains("let expects"));
    assert!(rendered.contains("expanded from here"));
}

#[test]
fn run_main_expands_user_macro_before_lowering() {
    let value = jisp::run_main(
        "unless.lisp",
        r#"
(def unless
  (~ (fn (condition then otherwise)
       `(if ,condition ,otherwise ,then))))

(export main
  (fn ()
    (unless false 1 2)))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "1");
}

#[test]
fn detailed_errors_render_user_macro_expansion_origin() {
    let error = match jisp::check_detailed(
        "bad-macro.lisp",
        r#"
(def add-true
  (~ (fn (value)
       `(+ ,value true))))

(export main
  (fn ()
    (add-true 1)))
"#,
    ) {
        Ok(_) => panic!("expected macro-expanded type error"),
        Err(error) => error,
    };

    let rendered = error.render_diagnostics().unwrap();

    assert!(rendered.contains("no overload of `+`"), "{rendered}");
    assert!(rendered.contains("expanded from here"), "{rendered}");
}

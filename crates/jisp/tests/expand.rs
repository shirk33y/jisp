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

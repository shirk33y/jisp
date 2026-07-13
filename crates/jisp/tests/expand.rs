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
fn parse_rejects_macro_exports_in_all_source_syntaxes() {
    let cases = [
        (
            "macro-export.lisp",
            r#"
(def unless
  (~ (fn ()
       (quote 1))))
(export unless)
"#,
        ),
        (
            "macro-export.json",
            r#"
[
  ["def", "unless",
    ["~", ["fn", [], ["quote", 1]]]],
  ["export", "unless"]
]
"#,
        ),
        (
            "macro-export.yaml",
            r#"
[
  [def, unless,
    [~, [fn, [], [quote, 1]]]],
  [export, unless]
]
"#,
        ),
    ];

    for (path, text) in cases {
        let error = match jisp::parse_detailed(path, text) {
            Ok(_) => panic!("{path} unexpectedly parsed"),
            Err(error) => error,
        };
        let rendered = error.render_diagnostics().unwrap();

        assert!(
            rendered.contains("macro `unless` cannot be exported"),
            "{path}: {rendered}"
        );
    }
}

#[test]
fn user_macro_template_bindings_do_not_capture_caller_identifiers() {
    let value = jisp::run_main(
        "hygienic-macro.lisp",
        r#"
(def wrap
  (~ (fn (expression)
       `(let (value 1)
          ,expression))))

(export main
  (fn ()
    (let (value 42)
      (wrap value))))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "42");
}

#[test]
fn hygienic_macro_let_binding_value_uses_outer_scope() {
    let value = jisp::run_main(
        "hygienic-macro-let-rhs.lisp",
        r#"
(def bind
  (~ (fn ()
       `(let (value value)
          value))))

(export main
  (fn ()
    (let (value 42)
      (bind))))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "42");
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

#[test]
fn run_main_binds_the_whole_value_with_an_alias_pattern() {
    let value = jisp::run_main(
        "case-alias.lisp",
        r#"
(type response
  (ok int)
  (err int))

(export main
  (fn ()
    (case (ok 7)
      ((as (ok value) whole)
        (case whole
          ((ok repeated) (+ value repeated))
          ((err _) 0)))
      ((err _) 0))))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "14");
}

#[test]
fn alias_patterns_reject_duplicate_bindings() {
    let error = match jisp::check(
        "case-alias-duplicate.lisp",
        r#"
(export main
  (fn ()
    (case 1
      ((as value value) value))))
"#,
    ) {
        Ok(_) => panic!("expected duplicate pattern binding error"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        jisp::Error::Type(InferError::Located { error, .. })
            if matches!(error.as_ref(), InferError::DuplicatePatternBinding(name) if name == "value")
    ));
}

#[test]
fn run_main_uses_or_pattern_with_consistent_bindings() {
    let value = jisp::run_main(
        "case-or.lisp",
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

    assert_eq!(value.display_string(), "8");
}

#[test]
fn or_patterns_require_consistent_bindings() {
    let error = jisp::check(
        "case-or-bindings.lisp",
        r#"
(export main
  (fn ()
    (case true
      ((or true value) 1))))
"#,
    );

    assert!(matches!(
        error,
        Err(jisp::Error::Type(InferError::Located { error, .. }))
            if matches!(error.as_ref(), InferError::InconsistentAlternativeBindings)
    ));
}

#[test]
fn run_main_evaluates_case_guards_after_pattern_bindings() {
    let value = jisp::run_main(
        "case-guard.lisp",
        r#"
(export main
  (fn ()
    (case 7
      ((when value (> value 10)) 1)
      (_ 2))))
"#,
    )
    .unwrap();

    assert_eq!(value.display_string(), "2");
}
use jisp::jisp_types::InferError;

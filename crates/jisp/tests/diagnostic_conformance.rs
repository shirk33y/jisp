struct FrontendCase {
    name: &'static str,
    source: &'static str,
    code: &'static str,
    line: usize,
    column: usize,
    message: &'static str,
    expansion_origin: bool,
}

#[test]
fn frontend_diagnostics_keep_codes_sources_and_primary_ranges() {
    let cases = [
        FrontendCase {
            name: "diagnostics/type-error.lisp",
            source: "(export main\n  (fn ()\n    (+ 1 true)))\n",
            code: "JISP-TYPE",
            line: 3,
            column: 5,
            message: "no overload of `+`",
            expansion_origin: false,
        },
        FrontendCase {
            name: "diagnostics/macro-origin.lisp",
            source: "(def invalid\n  (~ (fn (value)\n       `(+ ,value true))))\n\n(export main\n  (fn ()\n    (invalid 1)))\n",
            code: "JISP-TYPE",
            line: 3,
            column: 9,
            message: "no overload of `+`",
            expansion_origin: true,
        },
    ];

    for case in cases {
        let error = match jisp::check_detailed(case.name, case.source) {
            Ok(_) => panic!("{} unexpectedly passed", case.name),
            Err(error) => error,
        };
        let diagnostics = error
            .diagnostics()
            .unwrap_or_else(|| panic!("{} did not expose a structured diagnostic", case.name));
        assert_eq!(diagnostics.len(), 1, "{}", case.name);
        let diagnostic = &diagnostics[0];
        let file = error.sources.get(diagnostic.primary.span.source).unwrap();
        let (line, column) = file.line_col(diagnostic.primary.span.start);

        assert_eq!(diagnostic.code.as_deref(), Some(case.code), "{}", case.name);
        assert_eq!(file.name(), case.name, "{}", case.name);
        assert_eq!((line, column), (case.line, case.column), "{}", case.name);

        let rendered = error.render_diagnostics().unwrap();
        assert!(rendered.contains(case.message), "{rendered}");
        assert_eq!(
            rendered.contains("expanded from here"),
            case.expansion_origin,
            "{rendered}"
        );
    }
}

#[test]
fn imported_diagnostics_keep_the_dependency_path_and_range() {
    let directory = std::env::temp_dir().join(format!(
        "jisp-diagnostic-import-conformance-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let broken = directory.join("broken.lisp");
    fs::write(&broken, "(export value (+ 1 true))\n").unwrap();
    let source = "(import broken \"broken.lisp\")\n(export main (fn () broken.value))\n";
    fs::write(&main, source).unwrap();

    let error = match jisp::check_detailed(&main, source) {
        Ok(_) => panic!("imported type error unexpectedly passed"),
        Err(error) => error,
    };
    let diagnostic = error.diagnostics().unwrap().first().unwrap();
    let file = error.sources.get(diagnostic.primary.span.source).unwrap();

    assert_eq!(diagnostic.code.as_deref(), Some("JISP-TYPE"));
    assert_eq!(Path::new(file.name()), broken.canonicalize().unwrap());
    assert_eq!(file.line_col(diagnostic.primary.span.start), (1, 15));
    assert!(error
        .render_diagnostics()
        .unwrap()
        .contains("no overload of `+`"));

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn runtime_diagnostics_keep_code_range_and_evaluation_context() {
    let error = jisp::run_main_detailed(
        "diagnostics/runtime.lisp",
        r#"
(def divide-by-zero
  (fn () (/ 1 0)))

(export main
  (fn () (divide-by-zero)))
"#,
    )
    .unwrap_err();
    let diagnostic = error.diagnostics().unwrap().first().unwrap();
    let file = error.sources.get(diagnostic.primary.span.source).unwrap();

    assert_eq!(diagnostic.code.as_deref(), Some("JISP-RUNTIME"));
    assert_eq!(file.name(), "diagnostics/runtime.lisp");
    assert_eq!(file.line_col(diagnostic.primary.span.start), (3, 10));
    let rendered = error.render_diagnostics().unwrap();
    assert!(rendered.contains("division by zero"), "{rendered}");
    assert!(
        rendered.contains("while evaluating this expression"),
        "{rendered}"
    );
}
use std::{fs, path::Path};

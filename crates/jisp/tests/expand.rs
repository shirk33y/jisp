use jisp::jisp_ir::{Expr, ExprKind, Literal, Module, Pattern, StringPart};

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
fn run_main_expands_macro_import_from_file_module() {
    let directory =
        std::env::temp_dir().join(format!("jisp-macro-import-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let main = directory.join("main.lisp");
    let macros = directory.join("macros.lisp");
    std::fs::write(
        &macros,
        r#"
(def wrap
  (~ (fn (value)
       `(list 99 ,value))))
"#,
    )
    .unwrap();
    let text = r#"
(macro-import m "macros.lisp")

(export main
  (fn ()
    (m.wrap 7)))
"#;
    std::fs::write(&main, text).unwrap();

    let value = jisp::run_main(&main, text).unwrap();

    assert_eq!(value.display_string(), "[99, 7]");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn macro_hygiene_normalizes_to_the_same_ir_across_source_syntaxes() {
    let cases = [
        (
            "hygienic-macro.lisp",
            r#"
(def wrap
  (~ (fn (expression)
       (quasiquote (let (value 1)
          (unquote expression))))))

(export main
  (fn ()
    (let (value 42)
      (wrap value))))
"#,
        ),
        (
            "hygienic-macro.json",
            r#"
[
  ["def", "wrap",
    ["~", ["fn", ["expression"],
      ["quasiquote", ["let", ["value", 1], ["unquote", "expression"]]]]]],
  ["export", "main",
    ["fn", [],
      ["let", ["value", 42],
        ["wrap", "value"]]]]
]
"#,
        ),
        (
            "hygienic-macro.yaml",
            r#"
[
  [def, wrap,
    [~, [fn, [expression],
      [quasiquote, [let, [value, 1], [unquote, expression]]]]]],
  [export, main,
    [fn, [],
      [let, [value, 42],
        [wrap, value]]]]
]
"#,
        ),
    ];

    let mut shapes = vec![];
    for (path, text) in cases {
        let parsed = jisp::parse(path, text).unwrap();
        let value = jisp::run_main(path, text).unwrap();

        assert_eq!(value.display_string(), "42");
        shapes.push(module_shape(&parsed.module));
    }

    assert_eq!(shapes[0], shapes[1]);
    assert_eq!(shapes[1], shapes[2]);
    assert!(
        shapes[0].contains("__jisp_macro_0_value"),
        "expected normalized IR to include the deterministic hygienic binding: {}",
        shapes[0]
    );
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

fn module_shape(module: &Module) -> String {
    let imports = module
        .imports
        .iter()
        .map(|import| format!("import:{}={}", import.alias, import.path))
        .collect::<Vec<_>>()
        .join(",");
    let types = module
        .types
        .iter()
        .map(|ty| {
            let variants = ty
                .variants
                .iter()
                .map(|variant| format!("{}({})", variant.name, variant.field_types.join(",")))
                .collect::<Vec<_>>()
                .join("|");
            format!("type:{}={variants}", ty.name)
        })
        .collect::<Vec<_>>()
        .join(",");
    let definitions = module
        .definitions
        .iter()
        .map(|definition| {
            format!(
                "def:{}:{}={}",
                definition.public,
                definition.name,
                expr_shape(&definition.value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "imports[{imports}] types[{types}] definitions[{definitions}] exports[{}]",
        module.exports.join(",")
    )
}

fn expr_shape(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Literal(literal) => format!("literal:{}", literal_shape(literal)),
        ExprKind::Name(name) => format!("name:{name}"),
        ExprKind::Lambda { params, rest, body } => {
            format!("fn({};{:?})=>{}", params.join(","), rest, expr_shape(body))
        }
        ExprKind::Let { bindings, body } => {
            let bindings = bindings
                .iter()
                .map(|(name, value)| format!("{name}={}", expr_shape(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("let({bindings})=>{}", expr_shape(body))
        }
        ExprKind::Do(expressions) => format!("do({})", exprs_shape(expressions)),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if({},{},{})",
            expr_shape(condition),
            expr_shape(then_branch),
            expr_shape(else_branch)
        ),
        ExprKind::And(expressions) => format!("and({})", exprs_shape(expressions)),
        ExprKind::Or(expressions) => format!("or({})", exprs_shape(expressions)),
        ExprKind::Not(expression) => format!("not({})", expr_shape(expression)),
        ExprKind::Call { callee, arguments } => {
            format!("call({};{})", expr_shape(callee), exprs_shape(arguments))
        }
        ExprKind::List(items) => format!("list({})", exprs_shape(items)),
        ExprKind::Object(fields) => {
            let fields = fields
                .iter()
                .map(|(key, value)| format!("{}={}", expr_shape(key), expr_shape(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("obj({fields})")
        }
        ExprKind::Field { object, key } => {
            format!("field({},{})", expr_shape(object), expr_shape(key))
        }
        ExprKind::StringTemplate { lines, parts } => {
            let parts = parts
                .iter()
                .map(string_part_shape)
                .collect::<Vec<_>>()
                .join(",");
            format!("str:{lines}({parts})")
        }
        ExprKind::Case { subject, branches } => {
            let branches = branches
                .iter()
                .map(|branch| {
                    format!(
                        "{} when {:?} => {}",
                        pattern_shape(&branch.pattern),
                        branch.guard.as_ref().map(expr_shape),
                        expr_shape(&branch.body)
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("case({})[{branches}]", expr_shape(subject))
        }
    }
}

fn exprs_shape(expressions: &[Expr]) -> String {
    expressions
        .iter()
        .map(expr_shape)
        .collect::<Vec<_>>()
        .join(",")
}

fn literal_shape(literal: &Literal) -> String {
    match literal {
        Literal::Null => "null".to_owned(),
        Literal::Bool(value) => value.to_string(),
        Literal::Int(value) => value.to_string(),
        Literal::Float(value) => value.to_string(),
        Literal::String(value) => format!("{value:?}"),
    }
}

fn string_part_shape(part: &StringPart) -> String {
    match part {
        StringPart::Literal(value) => format!("literal:{value:?}"),
        StringPart::Expr(expr) => format!("expr:{}", expr_shape(expr)),
        StringPart::Splice(expr) => format!("splice:{}", expr_shape(expr)),
    }
}

fn pattern_shape(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "_".to_owned(),
        Pattern::Bind(name) => format!("bind:{name}"),
        Pattern::Alias { pattern, name } => format!("as({},{name})", pattern_shape(pattern)),
        Pattern::Or(alternatives) => format!(
            "or({})",
            alternatives
                .iter()
                .map(pattern_shape)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Pattern::Literal(literal) => format!("literal:{}", literal_shape(literal)),
        Pattern::Variant { tag, fields } => format!(
            "{tag}({})",
            fields
                .iter()
                .map(pattern_shape)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Pattern::List { prefix, rest } => format!(
            "list({};{:?})",
            prefix
                .iter()
                .map(pattern_shape)
                .collect::<Vec<_>>()
                .join(","),
            rest
        ),
        Pattern::Object(fields) => format!(
            "obj({})",
            fields
                .iter()
                .map(|(name, pattern)| format!("{name}:{}", pattern_shape(pattern)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
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

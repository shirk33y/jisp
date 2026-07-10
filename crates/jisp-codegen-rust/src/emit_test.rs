use jisp_core::{SourceId, Span};
use jisp_ir::{Definition, Expr, ExprKind, Literal, Module};
use jisp_types::{ObjectRow, Scheme, Type, TypedModule};

use crate::{generate, CodegenError};

fn span() -> Span {
    Span::empty(SourceId(0), 0)
}

fn expr(kind: ExprKind) -> Expr {
    Expr::new(kind, span())
}

fn literal(literal: Literal) -> Expr {
    expr(ExprKind::Literal(literal))
}

fn name(value: &str) -> Expr {
    expr(ExprKind::Name(value.to_owned()))
}

fn object_type(fields: impl IntoIterator<Item = (&'static str, Type)>) -> Type {
    Type::Object(ObjectRow {
        fields: fields
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), ty))
            .collect(),
        rest: None,
    })
}

fn definition(name: &str, public: bool, value: Expr) -> Definition {
    Definition {
        name: name.to_owned(),
        public,
        value,
        span: span(),
    }
}

fn typed_module(definitions: Vec<Definition>, schemes: Vec<(&str, Type)>) -> TypedModule {
    TypedModule {
        module: Module {
            imports: vec![],
            types: vec![],
            definitions,
            exports: vec![],
        },
        schemes: schemes
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), Scheme::mono(ty)))
            .collect(),
    }
}

#[test]
fn emits_public_scalar_definition_as_zero_arg_function() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            literal(Literal::String("hello".to_owned())),
        )],
        vec![("main", Type::Str)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub fn main () -> String"));
    assert!(generated.contains("String :: from (\"hello\")"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_top_level_function_and_direct_call() {
    let id = definition(
        "id",
        false,
        expr(ExprKind::Lambda {
            params: vec!["value".to_owned()],
            rest: None,
            body: Box::new(name("value")),
        }),
    );
    let main = definition(
        "main",
        true,
        expr(ExprKind::Call {
            callee: Box::new(name("id")),
            arguments: vec![literal(Literal::Int(42))],
        }),
    );
    let module = typed_module(
        vec![id, main],
        vec![
            (
                "id",
                Type::Function {
                    parameters: vec![Type::Int],
                    rest: None,
                    result: Box::new(Type::Int),
                },
            ),
            ("main", Type::Int),
        ],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("fn id (value : i64) -> i64"));
    assert!(generated.contains("pub fn main () -> i64"));
    assert!(generated.contains("id (42i64)"));
}

#[test]
fn emits_let_if_and_bool_expressions() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Let {
                bindings: vec![("flag".to_owned(), literal(Literal::Bool(true)))],
                body: Box::new(expr(ExprKind::If {
                    condition: Box::new(expr(ExprKind::And(vec![
                        name("flag"),
                        expr(ExprKind::Not(Box::new(literal(Literal::Bool(false))))),
                    ]))),
                    then_branch: Box::new(literal(Literal::Int(1))),
                    else_branch: Box::new(literal(Literal::Int(0))),
                })),
            }),
        )],
        vec![("main", Type::Int)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("let flag = true"));
    assert!(generated.contains("if (flag && ! false)"));
}

#[test]
fn emits_binary_prelude_intrinsics_as_native_operators() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Call {
                callee: Box::new(name("+")),
                arguments: vec![
                    literal(Literal::Int(40)),
                    expr(ExprKind::Call {
                        callee: Box::new(name("*")),
                        arguments: vec![literal(Literal::Int(1)), literal(Literal::Int(2))],
                    }),
                ],
            }),
        )],
        vec![("main", Type::Int)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub fn main () -> i64"));
    assert!(generated.contains("(40i64 + (1i64 * 2i64))"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_list_literals_as_vecs() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::List(vec![
                expr(ExprKind::Call {
                    callee: Box::new(name("+")),
                    arguments: vec![literal(Literal::Int(1)), literal(Literal::Int(1))],
                }),
                literal(Literal::Int(3)),
            ])),
        )],
        vec![("main", Type::List(Box::new(Type::Int)))],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub fn main () -> Vec < i64 >"));
    assert!(generated.contains("vec ! [(1i64 + 1i64) , 3i64]"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_closed_object_literals_as_native_structs() {
    let stats_type = object_type([("active", Type::Bool), ("age", Type::Int)]);
    let stats = definition(
        "stats",
        false,
        expr(ExprKind::Object(vec![
            (
                literal(Literal::String("active".to_owned())),
                literal(Literal::Bool(true)),
            ),
            (
                literal(Literal::String("age".to_owned())),
                literal(Literal::Int(42)),
            ),
        ])),
    );
    let main = definition(
        "main",
        true,
        expr(ExprKind::Field {
            object: Box::new(name("stats")),
            key: Box::new(literal(Literal::String("age".to_owned()))),
        }),
    );
    let module = typed_module(
        vec![stats, main],
        vec![("stats", stats_type), ("main", Type::Int)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub struct JispObject0"));
    assert!(generated.contains("pub active : bool"));
    assert!(generated.contains("pub age : i64"));
    assert!(generated.contains("fn stats () -> JispObject0"));
    assert!(generated.contains("JispObject0 { active : true , age : 42i64 }"));
    assert!(generated.contains("pub fn main () -> i64"));
    assert!(generated.contains("stats () . age"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn propagates_expected_object_type_through_let_body() {
    let stats_type = object_type([("age", Type::Int)]);
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Let {
                bindings: vec![("age".to_owned(), literal(Literal::Int(42)))],
                body: Box::new(expr(ExprKind::Object(vec![(
                    literal(Literal::String("age".to_owned())),
                    name("age"),
                )]))),
            }),
        )],
        vec![("main", stats_type)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub fn main () -> JispObject0"));
    assert!(generated.contains("JispObject0 { age : age }"));
}

#[test]
fn rejects_non_binary_native_intrinsics() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Call {
                callee: Box::new(name("+")),
                arguments: vec![literal(Literal::Int(1))],
            }),
        )],
        vec![("main", Type::Int)],
    );

    assert_eq!(
        generate(&module).unwrap_err(),
        CodegenError::Unsupported("non-binary native intrinsics")
    );
}

#[test]
fn rejects_unsupported_native_shapes_without_value_fallback() {
    let module = typed_module(
        vec![definition("main", true, expr(ExprKind::Object(vec![])))],
        vec![("main", Type::Int)],
    );

    assert_eq!(
        generate(&module).unwrap_err(),
        CodegenError::Unsupported("object expressions without expected native type")
    );
}

#[test]
fn rejects_names_outside_native_module_without_rust_fallback() {
    let module = typed_module(
        vec![definition("main", true, name("missing"))],
        vec![("main", Type::Int)],
    );

    assert_eq!(
        generate(&module).unwrap_err(),
        CodegenError::Unsupported("names outside native module")
    );
}

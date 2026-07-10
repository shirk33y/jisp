use jisp_core::{SourceId, Span};
use jisp_ir::{
    CaseBranch, Definition, Expr, ExprKind, Literal, Module, Pattern, StringPart, TypeDecl,
    VariantDecl,
};
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

fn named_type(name: &str) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: vec![],
    }
}

fn definition(name: &str, public: bool, value: Expr) -> Definition {
    Definition {
        name: name.to_owned(),
        public,
        value,
        span: span(),
    }
}

fn type_decl(name: &str, variants: Vec<(&str, Vec<&str>)>) -> TypeDecl {
    TypeDecl {
        name: name.to_owned(),
        variants: variants
            .into_iter()
            .map(|(name, field_types)| VariantDecl {
                name: name.to_owned(),
                field_types: field_types.into_iter().map(ToOwned::to_owned).collect(),
                span: span(),
            })
            .collect(),
        span: span(),
    }
}

fn branch(pattern: Pattern, body: Expr) -> CaseBranch {
    CaseBranch {
        pattern,
        body,
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
fn emits_string_templates_with_splices() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::StringTemplate {
                lines: true,
                parts: vec![
                    StringPart::Literal("first".to_owned()),
                    StringPart::Expr(literal(Literal::String("second".to_owned()))),
                    StringPart::Splice(expr(ExprKind::List(vec![
                        literal(Literal::String("third".to_owned())),
                        literal(Literal::String("fourth".to_owned())),
                    ]))),
                ],
            }),
        )],
        vec![("main", Type::Str)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("let mut fragments : Vec < String > = Vec :: new ()"));
    assert!(generated.contains("fragments . push (String :: from (\"first\"))"));
    assert!(generated.contains("fragments . push (String :: from (\"second\"))"));
    assert!(generated.contains("fragments . extend (vec ! [String :: from (\"third\")"));
    assert!(generated.contains("fragments . join (\"\\n\")"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_literal_case_as_native_if_chain() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Case {
                subject: Box::new(literal(Literal::Bool(true))),
                branches: vec![
                    branch(
                        Pattern::Literal(Literal::Bool(true)),
                        literal(Literal::Int(1)),
                    ),
                    branch(
                        Pattern::Literal(Literal::Bool(false)),
                        literal(Literal::Int(0)),
                    ),
                ],
            }),
        )],
        vec![("main", Type::Int)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("let __jisp_case_subject = true"));
    assert!(generated.contains("if __jisp_case_subject == true"));
    assert!(generated.contains("else { if __jisp_case_subject == false"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_bind_and_wildcard_case_patterns_without_value_fallback() {
    let module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Case {
                subject: Box::new(literal(Literal::Int(41))),
                branches: vec![
                    branch(Pattern::Literal(Literal::Int(0)), literal(Literal::Int(0))),
                    branch(
                        Pattern::Bind("value".to_owned()),
                        expr(ExprKind::Call {
                            callee: Box::new(name("+")),
                            arguments: vec![name("value"), literal(Literal::Int(1))],
                        }),
                    ),
                    branch(Pattern::Wildcard, literal(Literal::Int(-1))),
                ],
            }),
        )],
        vec![("main", Type::Int)],
    );

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("if __jisp_case_subject == 0i64"));
    assert!(generated.contains("let value = __jisp_case_subject . clone ()"));
    assert!(generated.contains("(value + 1i64)"));
    assert!(generated.contains("if true"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_native_enum_constructors() {
    let mut module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Call {
                callee: Box::new(name("ok")),
                arguments: vec![literal(Literal::Int(42))],
            }),
        )],
        vec![("main", named_type("result"))],
    );
    module.module.types.push(type_decl(
        "result",
        vec![("ok", vec!["int"]), ("err", vec!["str"])],
    ));

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("pub enum JispEnum0"));
    assert!(generated.contains("Ok (i64)"));
    assert!(generated.contains("Err (String)"));
    assert!(generated.contains("pub fn main () -> JispEnum0"));
    assert!(generated.contains("JispEnum0 :: Ok (42i64)"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
}

#[test]
fn emits_variant_case_as_native_match() {
    let mut module = typed_module(
        vec![definition(
            "main",
            true,
            expr(ExprKind::Case {
                subject: Box::new(expr(ExprKind::Call {
                    callee: Box::new(name("ok")),
                    arguments: vec![literal(Literal::Int(41))],
                })),
                branches: vec![
                    branch(
                        Pattern::Variant {
                            tag: "ok".to_owned(),
                            fields: vec![Pattern::Bind("value".to_owned())],
                        },
                        expr(ExprKind::Call {
                            callee: Box::new(name("+")),
                            arguments: vec![name("value"), literal(Literal::Int(1))],
                        }),
                    ),
                    branch(
                        Pattern::Variant {
                            tag: "err".to_owned(),
                            fields: vec![Pattern::Wildcard],
                        },
                        literal(Literal::Int(0)),
                    ),
                ],
            }),
        )],
        vec![("main", Type::Int)],
    );
    module.module.types.push(type_decl(
        "result",
        vec![("ok", vec!["int"]), ("err", vec!["str"])],
    ));

    let generated = generate(&module).unwrap().to_string();

    assert!(generated.contains("match __jisp_case_subject"));
    assert!(generated.contains("JispEnum0 :: Ok (value) => { (value + 1i64) }"));
    assert!(generated.contains("JispEnum0 :: Err (_) => { 0i64 }"));
    assert!(!generated.contains("Value"));
    assert!(!generated.contains("jisp_eval"));
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

use super::*;
use jisp_core::{SourceId, Span};
use jisp_ir::{Definition, Import, TypeDecl, VariantDecl};

fn span() -> Span {
    Span::empty(SourceId(0), 0)
}

fn expr(kind: ExprKind) -> Expr {
    Expr::new(kind, span())
}

fn name(value: &str) -> Expr {
    expr(ExprKind::Name(value.to_owned()))
}

fn int(value: i64) -> Expr {
    expr(ExprKind::Literal(Literal::Int(value)))
}

fn string(value: &str) -> Expr {
    expr(ExprKind::Literal(Literal::String(value.to_owned())))
}

fn definition(name: &str, value: Expr) -> Definition {
    Definition {
        name: name.to_owned(),
        public: false,
        value,
        span: span(),
    }
}

fn module(definitions: Vec<Definition>) -> Module {
    Module {
        imports: vec![],
        types: vec![],
        definitions,
        exports: vec![],
    }
}

#[test]
fn instantiation_creates_fresh_variables() {
    let mut inferencer = Inferencer::default();
    let scheme = Scheme {
        variables: vec![TypeVar(99)],
        body: Type::Function {
            parameters: vec![Type::Var(TypeVar(99))],
            result: Box::new(Type::Var(TypeVar(99))),
        },
    };
    let first = inferencer.instantiate(&scheme);
    let second = inferencer.instantiate(&scheme);
    assert_ne!(first, second);
}

#[test]
fn infers_function_calls() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Call {
        callee: Box::new(expr(ExprKind::Lambda {
            params: vec!["value".to_owned()],
            rest: None,
            body: Box::new(name("value")),
        })),
        arguments: vec![int(1)],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn generalizes_let_bindings() {
    let mut inferencer = Inferencer::default();
    let identity = expr(ExprKind::Lambda {
        params: vec!["value".to_owned()],
        rest: None,
        body: Box::new(name("value")),
    });
    let expression = expr(ExprKind::Let {
        bindings: vec![("id".to_owned(), identity)],
        body: Box::new(expr(ExprKind::Do(vec![
            expr(ExprKind::Call {
                callee: Box::new(name("id")),
                arguments: vec![int(1)],
            }),
            expr(ExprKind::Call {
                callee: Box::new(name("id")),
                arguments: vec![string("ok")],
            }),
        ]))),
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
}

#[test]
fn infers_static_object_fields() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Field {
        object: Box::new(expr(ExprKind::Object(vec![(
            string("name"),
            string("Ada"),
        )]))),
        key: Box::new(string("name")),
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
}

#[test]
fn generalizes_top_level_definitions() {
    let mut inferencer = Inferencer::default();
    let identity = expr(ExprKind::Lambda {
        params: vec!["value".to_owned()],
        rest: None,
        body: Box::new(name("value")),
    });
    let schemes = inferencer
        .infer_module(&module(vec![definition("id", identity)]))
        .unwrap();

    assert_eq!(schemes["id"].variables.len(), 1);
}

#[test]
fn installs_enum_constructor_schemes() {
    let mut inferencer = Inferencer::default();
    let result_decl = TypeDecl {
        name: "result".to_owned(),
        variants: vec![
            VariantDecl {
                name: "ok".to_owned(),
                field_types: vec!["value".to_owned()],
                span: span(),
            },
            VariantDecl {
                name: "err".to_owned(),
                field_types: vec!["error".to_owned()],
                span: span(),
            },
        ],
        span: span(),
    };
    let call_ok = expr(ExprKind::Call {
        callee: Box::new(name("ok")),
        arguments: vec![int(1)],
    });
    let mut module = module(vec![definition("main", call_ok)]);
    module.types.push(result_decl);

    let schemes = inferencer.infer_module(&module).unwrap();
    let Type::Named { name, arguments } = &schemes["main"].body else {
        panic!("main should infer to a result type");
    };
    assert_eq!(name, "result");
    assert_eq!(arguments[0], Type::Int);
    assert!(matches!(arguments[1], Type::Var(_)));
    assert_eq!(schemes["main"].variables.len(), 1);
}

#[test]
fn rejects_imports_until_module_resolution_has_type_environments() {
    let mut inferencer = Inferencer::default();
    let module = Module {
        imports: vec![Import {
            alias: "math".to_owned(),
            path: "std/math".to_owned(),
            span: span(),
        }],
        types: vec![],
        definitions: vec![],
        exports: vec![],
    };

    assert!(matches!(
        inferencer.infer_module(&module),
        Err(InferError::NotImplemented("import type environments"))
    ));
}

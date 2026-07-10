use super::*;
use jisp_core::{SourceId, Span};
use jisp_ir::{CaseBranch, Definition, Import, Pattern, TypeDecl, VariantDecl};

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

fn float(value: f64) -> Expr {
    expr(ExprKind::Literal(Literal::Float(value)))
}

fn string(value: &str) -> Expr {
    expr(ExprKind::Literal(Literal::String(value.to_owned())))
}

fn bool_(value: bool) -> Expr {
    expr(ExprKind::Literal(Literal::Bool(value)))
}

fn null() -> Expr {
    expr(ExprKind::Literal(Literal::Null))
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

fn branch(pattern: Pattern, body: Expr) -> CaseBranch {
    CaseBranch {
        pattern,
        body,
        span: span(),
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
fn generalizes_top_level_dependencies_before_dependents() {
    let mut inferencer = Inferencer::default();
    let identity = expr(ExprKind::Lambda {
        params: vec!["value".to_owned()],
        rest: None,
        body: Box::new(name("value")),
    });
    let main = expr(ExprKind::Do(vec![
        expr(ExprKind::Call {
            callee: Box::new(name("id")),
            arguments: vec![int(1)],
        }),
        expr(ExprKind::Call {
            callee: Box::new(name("id")),
            arguments: vec![string("ok")],
        }),
    ]));
    let schemes = inferencer
        .infer_module(&module(vec![
            definition("main", main),
            definition("id", identity),
        ]))
        .unwrap();

    assert_eq!(schemes["main"].body, Type::Str);
    assert_eq!(schemes["id"].variables.len(), 1);
}

#[test]
fn preserves_recursive_top_level_placeholders() {
    let mut inferencer = Inferencer::with_prelude();
    let fact = expr(ExprKind::Lambda {
        params: vec!["n".to_owned()],
        rest: None,
        body: Box::new(expr(ExprKind::If {
            condition: Box::new(expr(ExprKind::Call {
                callee: Box::new(name("=")),
                arguments: vec![name("n"), int(0)],
            })),
            then_branch: Box::new(int(1)),
            else_branch: Box::new(expr(ExprKind::Call {
                callee: Box::new(name("*")),
                arguments: vec![
                    name("n"),
                    expr(ExprKind::Call {
                        callee: Box::new(name("fact")),
                        arguments: vec![expr(ExprKind::Call {
                            callee: Box::new(name("-")),
                            arguments: vec![name("n"), int(1)],
                        })],
                    }),
                ],
            })),
        })),
    });
    let schemes = inferencer
        .infer_module(&module(vec![definition("fact", fact)]))
        .unwrap();

    assert_eq!(
        schemes["fact"].body,
        Type::Function {
            parameters: vec![Type::Int],
            result: Box::new(Type::Int),
        }
    );
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
fn infers_case_branch_result_type() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(int(1)),
        branches: vec![
            branch(Pattern::Literal(Literal::Int(1)), string("one")),
            branch(Pattern::Wildcard, string("many")),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
}

#[test]
fn infers_case_pattern_bindings() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![int(1), int(2)]))),
        branches: vec![branch(
            Pattern::List {
                prefix: vec![Pattern::Bind("head".to_owned())],
                rest: Some("tail".to_owned()),
            },
            expr(ExprKind::Do(vec![name("tail"), name("head")])),
        )],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn infers_enum_case_pattern_bindings() {
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
    let subject = expr(ExprKind::Call {
        callee: Box::new(name("ok")),
        arguments: vec![int(1)],
    });
    let case = expr(ExprKind::Case {
        subject: Box::new(subject),
        branches: vec![
            branch(
                Pattern::Variant {
                    tag: "ok".to_owned(),
                    fields: vec![Pattern::Bind("value".to_owned())],
                },
                name("value"),
            ),
            branch(
                Pattern::Variant {
                    tag: "err".to_owned(),
                    fields: vec![Pattern::Bind("message".to_owned())],
                },
                int(0),
            ),
        ],
    });
    let mut module = module(vec![definition("main", case)]);
    module.types.push(result_decl);

    let schemes = inferencer.infer_module(&module).unwrap();
    assert_eq!(schemes["main"].body, Type::Int);
}

#[test]
fn rejects_case_branch_type_mismatch() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![
            branch(Pattern::Literal(Literal::Bool(true)), int(1)),
            branch(Pattern::Wildcard, string("no")),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::Unify(UnifyError::Mismatch { .. }))
    ));
}

#[test]
fn rejects_non_exhaustive_bool_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![branch(Pattern::Literal(Literal::Bool(true)), int(1))],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "bool" && missing == vec!["false".to_owned()]
    ));
}

#[test]
fn accepts_exhaustive_null_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(null()),
        branches: vec![branch(Pattern::Literal(Literal::Null), int(1))],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_non_exhaustive_enum_case() {
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
    let subject = expr(ExprKind::Call {
        callee: Box::new(name("ok")),
        arguments: vec![int(1)],
    });
    let case = expr(ExprKind::Case {
        subject: Box::new(subject),
        branches: vec![branch(
            Pattern::Variant {
                tag: "ok".to_owned(),
                fields: vec![Pattern::Bind("value".to_owned())],
            },
            name("value"),
        )],
    });
    let mut module = module(vec![definition("main", case)]);
    module.types.push(result_decl);

    assert!(matches!(
        inferencer.infer_module(&module),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "result" && missing == vec!["err".to_owned()]
    ));
}

#[test]
fn rejects_duplicate_pattern_bindings() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![int(1), int(2)]))),
        branches: vec![branch(
            Pattern::List {
                prefix: vec![
                    Pattern::Bind("item".to_owned()),
                    Pattern::Bind("item".to_owned()),
                ],
                rest: None,
            },
            name("item"),
        )],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::DuplicatePatternBinding(name)) if name == "item"
    ));
}

#[test]
fn prelude_infers_fixed_arity_numeric_builtins() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Call {
        callee: Box::new(name("+")),
        arguments: vec![int(1), int(2)],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn prelude_infers_float_numeric_overloads() {
    let mut inferencer = Inferencer::with_prelude();

    let add = expr(ExprKind::Call {
        callee: Box::new(name("+")),
        arguments: vec![float(1.0), float(2.0)],
    });
    assert_eq!(inferencer.infer_expr(&add).unwrap(), Type::Float);

    let less = expr(ExprKind::Call {
        callee: Box::new(name("<")),
        arguments: vec![float(1.0), float(2.0)],
    });
    assert_eq!(inferencer.infer_expr(&less).unwrap(), Type::Bool);

    let abs = expr(ExprKind::Call {
        callee: Box::new(name("math.abs")),
        arguments: vec![float(-1.0)],
    });
    assert_eq!(inferencer.infer_expr(&abs).unwrap(), Type::Float);
}

#[test]
fn prelude_rejects_mixed_numeric_overloads() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Call {
        callee: Box::new(name("+")),
        arguments: vec![int(1), float(2.0)],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::Unify(_))
    ));
}

#[test]
fn local_bindings_shadow_prelude_overloads() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Let {
        bindings: vec![(
            "+".to_owned(),
            expr(ExprKind::Lambda {
                params: vec!["value".to_owned()],
                rest: None,
                body: Box::new(bool_(true)),
            }),
        )],
        body: Box::new(expr(ExprKind::Call {
            callee: Box::new(name("+")),
            arguments: vec![float(1.0)],
        })),
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Bool);
}

#[test]
fn prelude_infers_list_map() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Call {
        callee: Box::new(name("list.map")),
        arguments: vec![
            expr(ExprKind::Lambda {
                params: vec!["value".to_owned()],
                rest: None,
                body: Box::new(expr(ExprKind::Call {
                    callee: Box::new(name("+")),
                    arguments: vec![name("value"), int(1)],
                })),
            }),
            expr(ExprKind::List(vec![int(1), int(2)])),
        ],
    });

    assert_eq!(
        inferencer.infer_expr(&expression).unwrap(),
        Type::List(Box::new(Type::Int))
    );
}

#[test]
fn prelude_infers_runtime_predicates_and_conversions() {
    let mut inferencer = Inferencer::with_prelude();

    let str_is = expr(ExprKind::Call {
        callee: Box::new(name("str.is")),
        arguments: vec![int(1)],
    });
    assert_eq!(inferencer.infer_expr(&str_is).unwrap(), Type::Bool);

    let str_from = expr(ExprKind::Call {
        callee: Box::new(name("str.from")),
        arguments: vec![bool_(true)],
    });
    assert_eq!(inferencer.infer_expr(&str_from).unwrap(), Type::Str);

    let list_is = expr(ExprKind::Call {
        callee: Box::new(name("list.is")),
        arguments: vec![string("not a list")],
    });
    assert_eq!(inferencer.infer_expr(&list_is).unwrap(), Type::Bool);
}

#[test]
fn prelude_infers_basic_object_helpers() {
    let mut inferencer = Inferencer::with_prelude();

    let keys = expr(ExprKind::Call {
        callee: Box::new(name("obj.keys")),
        arguments: vec![expr(ExprKind::Object(vec![(
            string("name"),
            string("Ada"),
        )]))],
    });

    assert_eq!(
        inferencer.infer_expr(&keys).unwrap(),
        Type::List(Box::new(Type::Str))
    );
}

#[test]
fn prelude_infers_result_case() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Call {
            callee: Box::new(name("list.get")),
            arguments: vec![expr(ExprKind::List(vec![int(1), int(2)])), int(0)],
        })),
        branches: vec![
            branch(
                Pattern::Variant {
                    tag: "ok".to_owned(),
                    fields: vec![Pattern::Bind("value".to_owned())],
                },
                name("value"),
            ),
            branch(
                Pattern::Variant {
                    tag: "err".to_owned(),
                    fields: vec![Pattern::Wildcard],
                },
                int(0),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn prelude_infers_result_recover() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Call {
            callee: Box::new(name("result.recover")),
            arguments: vec![
                expr(ExprKind::Call {
                    callee: Box::new(name("err")),
                    arguments: vec![string("bad")],
                }),
                expr(ExprKind::Lambda {
                    params: vec!["error".to_owned()],
                    rest: None,
                    body: Box::new(expr(ExprKind::Call {
                        callee: Box::new(name("ok")),
                        arguments: vec![expr(ExprKind::Call {
                            callee: Box::new(name("str.len")),
                            arguments: vec![name("error")],
                        })],
                    })),
                }),
            ],
        })),
        branches: vec![
            branch(
                Pattern::Variant {
                    tag: "ok".to_owned(),
                    fields: vec![Pattern::Bind("value".to_owned())],
                },
                name("value"),
            ),
            branch(
                Pattern::Variant {
                    tag: "err".to_owned(),
                    fields: vec![Pattern::Wildcard],
                },
                int(0),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn prelude_rejects_non_exhaustive_result_case() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Call {
            callee: Box::new(name("list.get")),
            arguments: vec![expr(ExprKind::List(vec![int(1), int(2)])), int(0)],
        })),
        branches: vec![branch(
            Pattern::Variant {
                tag: "ok".to_owned(),
                fields: vec![Pattern::Bind("value".to_owned())],
            },
            name("value"),
        )],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "result" && missing == vec!["err".to_owned()]
    ));
}

#[test]
fn infers_qualified_values_from_import_type_environments() {
    let mut inferencer = Inferencer::default();
    let module = Module {
        imports: vec![Import {
            alias: "math".to_owned(),
            path: "math".to_owned(),
            span: span(),
        }],
        types: vec![],
        definitions: vec![definition(
            "main",
            expr(ExprKind::Call {
                callee: Box::new(name("math.inc")),
                arguments: vec![int(41)],
            }),
        )],
        exports: vec![],
    };
    let mut math = BTreeMap::new();
    math.insert(
        "inc".to_owned(),
        Scheme::mono(Type::Function {
            parameters: vec![Type::Int],
            result: Box::new(Type::Int),
        }),
    );
    let imports = BTreeMap::from([("math".to_owned(), math)]);

    let schemes = inferencer
        .infer_module_with_imports(&module, &imports)
        .unwrap();

    assert_eq!(schemes["main"].body, Type::Int);
}

#[test]
fn rejects_unresolved_import_type_environments() {
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
        Err(InferError::UnresolvedImport { alias, path })
            if alias == "math" && path == "std/math"
    ));
}

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
            rest: None,
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
fn infers_variadic_lambda_rest_arguments() {
    let mut inferencer = Inferencer::with_prelude();
    let expression = expr(ExprKind::Call {
        callee: Box::new(expr(ExprKind::Lambda {
            params: vec!["head".to_owned()],
            rest: Some("tail".to_owned()),
            body: Box::new(expr(ExprKind::Call {
                callee: Box::new(name("list.prepend")),
                arguments: vec![name("head"), name("tail")],
            })),
        })),
        arguments: vec![int(1), int(2), int(3)],
    });

    assert_eq!(
        inferencer.infer_expr(&expression).unwrap(),
        Type::List(Box::new(Type::Int))
    );
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
            rest: None,
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
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![],
                    rest: None,
                },
                int(0),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("head".to_owned())],
                    rest: Some("tail".to_owned()),
                },
                expr(ExprKind::Do(vec![name("tail"), name("head")])),
            ),
        ],
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
fn rejects_pattern_type_mismatch_between_subject_and_literal() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![branch(Pattern::Literal(Literal::Int(1)), int(1))],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::Unify(UnifyError::Mismatch { .. }))
    ));
}

#[test]
fn rejects_empty_bool_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "bool" && missing == vec!["false".to_owned(), "true".to_owned()]
    ));
}

#[test]
fn rejects_empty_open_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(int(1)),
        branches: vec![],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "int" && missing == vec!["_".to_owned()]
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
fn rejects_redundant_bool_case_pattern() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![
            branch(Pattern::Literal(Literal::Bool(true)), int(1)),
            branch(Pattern::Literal(Literal::Bool(true)), int(2)),
            branch(Pattern::Literal(Literal::Bool(false)), int(0)),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "true"
    ));
}

#[test]
fn rejects_case_branch_after_catch_all() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(bool_(true)),
        branches: vec![
            branch(Pattern::Wildcard, int(1)),
            branch(Pattern::Literal(Literal::Bool(false)), int(0)),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "false"
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
fn accepts_exhaustive_list_case_with_empty_and_rest_patterns() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![int(1), int(2)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![],
                    rest: None,
                },
                int(0),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("head".to_owned())],
                    rest: Some("tail".to_owned()),
                },
                name("head"),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn infers_nested_list_case_pattern_bindings() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![expr(ExprKind::List(vec![
            int(1),
            int(2),
        ]))]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![],
                    rest: None,
                },
                int(0),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::List {
                        prefix: vec![Pattern::Bind("head".to_owned())],
                        rest: Some("tail".to_owned()),
                    }],
                    rest: Some("outer_tail".to_owned()),
                },
                expr(ExprKind::Do(vec![
                    name("outer_tail"),
                    name("tail"),
                    name("head"),
                ])),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_non_exhaustive_list_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![int(1), int(2)]))),
        branches: vec![branch(
            Pattern::List {
                prefix: vec![],
                rest: None,
            },
            int(0),
        )],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "list" && missing == vec!["list length >= 1".to_owned()]
    ));
}

#[test]
fn rejects_redundant_list_case_pattern() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![int(1), int(2)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("head".to_owned())],
                    rest: Some("tail".to_owned()),
                },
                name("head"),
            ),
            branch(
                Pattern::List {
                    prefix: vec![
                        Pattern::Bind("first".to_owned()),
                        Pattern::Bind("second".to_owned()),
                    ],
                    rest: None,
                },
                name("first"),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "list pattern"
    ));
}

#[test]
fn accepts_refined_exhaustive_bool_list_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![bool_(true)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![],
                    rest: None,
                },
                int(0),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(true))],
                    rest: None,
                },
                int(1),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(false))],
                    rest: None,
                },
                int(2),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("head".to_owned())],
                    rest: Some("tail".to_owned()),
                },
                int(3),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_redundant_refined_bool_list_case_pattern() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![bool_(true)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(true))],
                    rest: None,
                },
                int(1),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(true))],
                    rest: None,
                },
                int(2),
            ),
            branch(
                Pattern::List {
                    prefix: vec![],
                    rest: Some("items".to_owned()),
                },
                int(0),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "list pattern"
    ));
}

#[test]
fn rejects_refined_bool_list_case_pattern_covered_by_exact_length() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![bool_(true)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("item".to_owned())],
                    rest: None,
                },
                int(0),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(true))],
                    rest: None,
                },
                int(1),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "list pattern"
    ));
}

#[test]
fn rejects_exact_bool_list_case_pattern_covered_by_refinements() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![bool_(true)]))),
        branches: vec![
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(true))],
                    rest: None,
                },
                int(1),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Literal(Literal::Bool(false))],
                    rest: None,
                },
                int(2),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Bind("item".to_owned())],
                    rest: None,
                },
                int(0),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "list pattern"
    ));
}

#[test]
fn accepts_exhaustive_object_case_with_required_field_bindings() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("name"), string("Ada")),
            (string("age"), int(37)),
        ]))),
        branches: vec![branch(
            Pattern::Object(vec![("name".to_owned(), Pattern::Bind("name".to_owned()))]),
            name("name"),
        )],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
}

#[test]
fn infers_nested_object_case_pattern_bindings() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (
                string("user"),
                expr(ExprKind::Object(vec![(string("name"), string("Ada"))])),
            ),
            (string("active"), bool_(true)),
        ]))),
        branches: vec![branch(
            Pattern::Object(vec![(
                "user".to_owned(),
                Pattern::Object(vec![("name".to_owned(), Pattern::Bind("name".to_owned()))]),
            )]),
            name("name"),
        )],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Str);
}

#[test]
fn accepts_refined_exhaustive_bool_field_object_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("name"), string("Ada")),
        ]))),
        branches: vec![
            branch(
                Pattern::Object(vec![(
                    "active".to_owned(),
                    Pattern::Literal(Literal::Bool(true)),
                )]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![(
                    "active".to_owned(),
                    Pattern::Literal(Literal::Bool(false)),
                )]),
                int(0),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_payload_refined_enum_field_as_exhaustive_object_case() {
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
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![(
            string("status"),
            expr(ExprKind::Call {
                callee: Box::new(name("ok")),
                arguments: vec![int(1)],
            }),
        )]))),
        branches: vec![
            branch(
                Pattern::Object(vec![(
                    "status".to_owned(),
                    Pattern::Variant {
                        tag: "ok".to_owned(),
                        fields: vec![Pattern::Literal(Literal::Int(1))],
                    },
                )]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![(
                    "status".to_owned(),
                    Pattern::Variant {
                        tag: "err".to_owned(),
                        fields: vec![Pattern::Wildcard],
                    },
                )]),
                int(0),
            ),
        ],
    });
    let mut module = module(vec![definition("main", expression)]);
    module.types.push(result_decl);

    assert!(matches!(
        inferencer.infer_module(&module),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "object" && missing == vec!["object pattern".to_owned()]
    ));
}

#[test]
fn accepts_refined_exhaustive_nested_bool_field_object_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![(
            string("user"),
            expr(ExprKind::Object(vec![
                (string("active"), bool_(true)),
                (string("name"), string("Ada")),
            ])),
        )]))),
        branches: vec![
            branch(
                Pattern::Object(vec![(
                    "user".to_owned(),
                    Pattern::Object(vec![(
                        "active".to_owned(),
                        Pattern::Literal(Literal::Bool(true)),
                    )]),
                )]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![(
                    "user".to_owned(),
                    Pattern::Object(vec![(
                        "active".to_owned(),
                        Pattern::Literal(Literal::Bool(false)),
                    )]),
                )]),
                int(0),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_redundant_refined_object_case_pattern() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![(
            string("active"),
            bool_(true),
        )]))),
        branches: vec![
            branch(
                Pattern::Object(vec![(
                    "active".to_owned(),
                    Pattern::Literal(Literal::Bool(true)),
                )]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![(
                    "active".to_owned(),
                    Pattern::Literal(Literal::Bool(true)),
                )]),
                int(2),
            ),
            branch(Pattern::Object(vec![]), int(0)),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "object pattern"
    ));
}

#[test]
fn rejects_non_exhaustive_object_case() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![(
            string("name"),
            string("Ada"),
        )]))),
        branches: vec![branch(
            Pattern::Object(vec![(
                "name".to_owned(),
                Pattern::Literal(Literal::String("Ada".to_owned())),
            )]),
            int(1),
        )],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "object" && missing == vec!["object pattern".to_owned()]
    ));
}

#[test]
fn rejects_redundant_object_case_pattern() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![(
            string("name"),
            string("Ada"),
        )]))),
        branches: vec![
            branch(Pattern::Object(vec![]), int(1)),
            branch(
                Pattern::Object(vec![("name".to_owned(), Pattern::Bind("name".to_owned()))]),
                int(2),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "object pattern"
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
    let object = || {
        expr(ExprKind::Object(vec![
            (string("name"), string("Ada")),
            (string("age"), int(37)),
        ]))
    };

    let keys = expr(ExprKind::Call {
        callee: Box::new(name("obj.keys")),
        arguments: vec![object()],
    });

    assert_eq!(
        inferencer.infer_expr(&keys).unwrap(),
        Type::List(Box::new(Type::Str))
    );

    let get = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Call {
            callee: Box::new(name("obj.get")),
            arguments: vec![object(), string("age")],
        })),
        branches: vec![
            branch(
                Pattern::Variant {
                    tag: "ok".to_owned(),
                    fields: vec![Pattern::Bind("age".to_owned())],
                },
                expr(ExprKind::Call {
                    callee: Box::new(name("+")),
                    arguments: vec![name("age"), int(1)],
                }),
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
    assert_eq!(inferencer.infer_expr(&get).unwrap(), Type::Int);

    let set_len = expr(ExprKind::Call {
        callee: Box::new(name("obj.len")),
        arguments: vec![expr(ExprKind::Call {
            callee: Box::new(name("obj.set")),
            arguments: vec![object(), string("active"), bool_(true)],
        })],
    });
    assert_eq!(inferencer.infer_expr(&set_len).unwrap(), Type::Int);

    let del_keys = expr(ExprKind::Call {
        callee: Box::new(name("obj.keys")),
        arguments: vec![expr(ExprKind::Call {
            callee: Box::new(name("obj.del")),
            arguments: vec![object(), string("name")],
        })],
    });
    assert_eq!(
        inferencer.infer_expr(&del_keys).unwrap(),
        Type::List(Box::new(Type::Str))
    );

    let values_len = expr(ExprKind::Call {
        callee: Box::new(name("list.len")),
        arguments: vec![expr(ExprKind::Call {
            callee: Box::new(name("obj.values")),
            arguments: vec![object()],
        })],
    });
    assert_eq!(inferencer.infer_expr(&values_len).unwrap(), Type::Int);

    let cat_len = expr(ExprKind::Call {
        callee: Box::new(name("obj.len")),
        arguments: vec![expr(ExprKind::Call {
            callee: Box::new(name("obj.cat")),
            arguments: vec![
                expr(ExprKind::Object(vec![(string("name"), string("Ada"))])),
                expr(ExprKind::Object(vec![(string("active"), bool_(true))])),
            ],
        })],
    });
    assert_eq!(inferencer.infer_expr(&cat_len).unwrap(), Type::Int);
}

#[test]
fn prelude_refines_static_object_helpers() {
    let mut inferencer = Inferencer::with_prelude();

    let set = expr(ExprKind::Call {
        callee: Box::new(name("obj.set")),
        arguments: vec![
            expr(ExprKind::Object(vec![(string("name"), string("Ada"))])),
            string("active"),
            bool_(true),
        ],
    });
    assert_eq!(
        inferencer.infer_expr(&set).unwrap(),
        Type::Object(ObjectRow {
            fields: BTreeMap::from([
                ("active".to_owned(), Type::Bool),
                ("name".to_owned(), Type::Str),
            ]),
            rest: None,
        })
    );

    let del = expr(ExprKind::Call {
        callee: Box::new(name("obj.del")),
        arguments: vec![
            expr(ExprKind::Object(vec![
                (string("name"), string("Ada")),
                (string("age"), int(37)),
            ])),
            string("name"),
        ],
    });
    assert_eq!(
        inferencer.infer_expr(&del).unwrap(),
        Type::Object(ObjectRow {
            fields: BTreeMap::from([("age".to_owned(), Type::Int)]),
            rest: None,
        })
    );

    let cat = expr(ExprKind::Call {
        callee: Box::new(name("obj.cat")),
        arguments: vec![
            expr(ExprKind::Object(vec![(string("name"), string("Ada"))])),
            expr(ExprKind::Object(vec![(string("active"), bool_(true))])),
        ],
    });
    assert_eq!(
        inferencer.infer_expr(&cat).unwrap(),
        Type::Object(ObjectRow {
            fields: BTreeMap::from([
                ("active".to_owned(), Type::Bool),
                ("name".to_owned(), Type::Str),
            ]),
            rest: None,
        })
    );
}

#[test]
fn prelude_refines_static_object_get_and_values() {
    let mut inferencer = Inferencer::with_prelude();

    let get = expr(ExprKind::Call {
        callee: Box::new(name("obj.get")),
        arguments: vec![
            expr(ExprKind::Object(vec![
                (string("name"), string("Ada")),
                (string("age"), int(37)),
            ])),
            string("age"),
        ],
    });
    assert_eq!(
        inferencer.infer_expr(&get).unwrap(),
        Type::Named {
            name: "result".to_owned(),
            arguments: vec![Type::Int, Type::Str],
        }
    );

    let values = expr(ExprKind::Call {
        callee: Box::new(name("obj.values")),
        arguments: vec![expr(ExprKind::Object(vec![
            (string("start"), int(1)),
            (string("end"), int(3)),
        ]))],
    });
    assert_eq!(
        inferencer.infer_expr(&values).unwrap(),
        Type::List(Box::new(Type::Int))
    );
}

#[test]
fn prelude_keeps_dynamic_object_helper_fallback() {
    let mut inferencer = Inferencer::with_prelude();

    let dynamic_set = expr(ExprKind::Call {
        callee: Box::new(name("obj.set")),
        arguments: vec![
            expr(ExprKind::Object(vec![(string("name"), string("Ada"))])),
            expr(ExprKind::Call {
                callee: Box::new(name("str.cat")),
                arguments: vec![string("act"), string("ive")],
            }),
            bool_(true),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&dynamic_set).unwrap(),
        Type::Object(ObjectRow { rest: Some(_), .. })
    ));
}

#[test]
fn prelude_infers_variadic_runtime_helpers() {
    let mut inferencer = Inferencer::with_prelude();

    let empty_str_cat = expr(ExprKind::Call {
        callee: Box::new(name("str.cat")),
        arguments: vec![],
    });
    assert_eq!(inferencer.infer_expr(&empty_str_cat).unwrap(), Type::Str);

    let one_str_cat = expr(ExprKind::Call {
        callee: Box::new(name("str.cat")),
        arguments: vec![string("hello")],
    });
    assert_eq!(inferencer.infer_expr(&one_str_cat).unwrap(), Type::Str);

    let str_cat = expr(ExprKind::Call {
        callee: Box::new(name("str.cat")),
        arguments: vec![string("hello"), string(" "), string("world")],
    });
    assert_eq!(inferencer.infer_expr(&str_cat).unwrap(), Type::Str);

    let one_list_cat = expr(ExprKind::Call {
        callee: Box::new(name("list.cat")),
        arguments: vec![expr(ExprKind::List(vec![int(1)]))],
    });
    assert_eq!(
        inferencer.infer_expr(&one_list_cat).unwrap(),
        Type::List(Box::new(Type::Int))
    );

    let list_cat = expr(ExprKind::Call {
        callee: Box::new(name("list.cat")),
        arguments: vec![
            expr(ExprKind::List(vec![int(1)])),
            expr(ExprKind::List(vec![int(2)])),
            expr(ExprKind::List(vec![int(3)])),
        ],
    });
    assert_eq!(
        inferencer.infer_expr(&list_cat).unwrap(),
        Type::List(Box::new(Type::Int))
    );

    let println = expr(ExprKind::Call {
        callee: Box::new(name("io.println")),
        arguments: vec![string("hello")],
    });
    assert_eq!(inferencer.infer_expr(&println).unwrap(), Type::Null);
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
            rest: None,
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

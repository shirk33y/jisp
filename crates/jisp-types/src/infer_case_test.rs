use super::*;
use jisp_ir::{Pattern, TypeDecl, VariantDecl};

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
fn reports_missing_refined_bool_list_case_pattern() {
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
                    prefix: vec![Pattern::Wildcard, Pattern::Wildcard],
                    rest: Some("tail".to_owned()),
                },
                int(2),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "list" && missing == vec!["list [false]".to_owned()]
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
fn guarded_list_refinements_do_not_establish_exhaustiveness() {
    let mut inferencer = Inferencer::default();
    let guarded_branch = |pattern| CaseBranch {
        pattern,
        guard: Some(bool_(true)),
        body: int(1),
        span: span(),
    };
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::List(vec![bool_(true)]))),
        branches: vec![
            guarded_branch(Pattern::List {
                prefix: vec![Pattern::Literal(Literal::Bool(true))],
                rest: None,
            }),
            guarded_branch(Pattern::List {
                prefix: vec![Pattern::Literal(Literal::Bool(false))],
                rest: None,
            }),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, .. }) if type_name == "list"
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
            if type_name == "object" && missing == vec!["object {status: ok}".to_owned()]
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
fn accepts_exhaustive_list_case_with_nested_bool_alternative() {
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
                    prefix: vec![Pattern::Or(vec![
                        Pattern::Literal(Literal::Bool(true)),
                        Pattern::Literal(Literal::Bool(false)),
                    ])],
                    rest: None,
                },
                int(1),
            ),
            branch(
                Pattern::List {
                    prefix: vec![Pattern::Wildcard, Pattern::Wildcard],
                    rest: Some("tail".to_owned()),
                },
                int(2),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn accepts_exhaustive_object_case_with_nested_bool_alternative() {
    let mut inferencer = Inferencer::default();
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("name"), string("Ada")),
        ]))),
        branches: vec![branch(
            Pattern::Object(vec![(
                "active".to_owned(),
                Pattern::Or(vec![
                    Pattern::Literal(Literal::Bool(true)),
                    Pattern::Literal(Literal::Bool(false)),
                ]),
            )]),
            int(1),
        )],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn accepts_exhaustive_object_case_covering_two_boolean_fields() {
    let mut inferencer = Inferencer::default();
    let field = |name: &str, value| (name.to_owned(), Pattern::Literal(Literal::Bool(value)));
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("visible"), bool_(true)),
        ]))),
        branches: vec![
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(3),
            ),
            branch(
                Pattern::Object(vec![field("active", true), field("visible", false)]),
                int(2),
            ),
            branch(
                Pattern::Object(vec![field("active", false), field("visible", true)]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![field("active", false), field("visible", false)]),
                int(0),
            ),
        ],
    });

    assert_eq!(inferencer.infer_expr(&expression).unwrap(), Type::Int);
}

#[test]
fn rejects_redundant_object_product_case_pattern() {
    let mut inferencer = Inferencer::default();
    let field = |name: &str, value| (name.to_owned(), Pattern::Literal(Literal::Bool(value)));
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("visible"), bool_(true)),
        ]))),
        branches: vec![
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(3),
            ),
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(4),
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
fn reports_missing_object_product_case_pattern() {
    let mut inferencer = Inferencer::default();
    let field = |name: &str, value| (name.to_owned(), Pattern::Literal(Literal::Bool(value)));
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("visible"), bool_(true)),
        ]))),
        branches: vec![
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(3),
            ),
            branch(
                Pattern::Object(vec![field("active", true), field("visible", false)]),
                int(2),
            ),
            branch(
                Pattern::Object(vec![field("active", false), field("visible", true)]),
                int(1),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "object"
                && missing == vec!["object {active: false, visible: false}".to_owned()]
    ));
}

#[test]
fn rejects_object_product_case_pattern_after_full_product_coverage() {
    let mut inferencer = Inferencer::default();
    let field = |name: &str, value| (name.to_owned(), Pattern::Literal(Literal::Bool(value)));
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("visible"), bool_(true)),
        ]))),
        branches: vec![
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(3),
            ),
            branch(
                Pattern::Object(vec![field("active", true), field("visible", false)]),
                int(2),
            ),
            branch(
                Pattern::Object(vec![field("active", false), field("visible", true)]),
                int(1),
            ),
            branch(
                Pattern::Object(vec![field("active", false), field("visible", false)]),
                int(0),
            ),
            branch(
                Pattern::Object(vec![field("active", true), field("visible", true)]),
                int(4),
            ),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::RedundantCasePattern(pattern)) if pattern == "object pattern"
    ));
}

#[test]
fn guarded_object_product_patterns_do_not_establish_exhaustiveness() {
    let mut inferencer = Inferencer::default();
    let field = |name: &str, value| (name.to_owned(), Pattern::Literal(Literal::Bool(value)));
    let guarded_branch = |pattern| CaseBranch {
        pattern,
        guard: Some(bool_(true)),
        body: int(1),
        span: span(),
    };
    let expression = expr(ExprKind::Case {
        subject: Box::new(expr(ExprKind::Object(vec![
            (string("active"), bool_(true)),
            (string("visible"), bool_(true)),
        ]))),
        branches: vec![
            guarded_branch(Pattern::Object(vec![
                field("active", true),
                field("visible", true),
            ])),
            guarded_branch(Pattern::Object(vec![
                field("active", true),
                field("visible", false),
            ])),
            guarded_branch(Pattern::Object(vec![
                field("active", false),
                field("visible", true),
            ])),
            guarded_branch(Pattern::Object(vec![
                field("active", false),
                field("visible", false),
            ])),
        ],
    });

    assert!(matches!(
        inferencer.infer_expr(&expression),
        Err(InferError::NonExhaustiveCase { type_name, missing })
            if type_name == "object" && missing == vec!["object pattern".to_owned()]
    ));
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

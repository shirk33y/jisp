use std::collections::BTreeMap;

use jisp_types::{ObjectRow, Scheme, Type, TypeVar, TypedModule};

use crate::{generate, generate_detailed, CodegenError, RustItemKind};

#[test]
fn generate_rejects_open_object_rows_before_emitting_rust() {
    let module = TypedModule {
        module: jisp_ir::Module::empty(),
        schemes: BTreeMap::from([(
            "main".to_owned(),
            Scheme::mono(Type::Object(ObjectRow {
                fields: BTreeMap::from([("name".to_owned(), Type::Str)]),
                rest: Some(TypeVar(1)),
            })),
        )]),
        expression_types: Default::default(),
    };

    let error = generate(&module).unwrap_err();

    assert!(matches!(error, CodegenError::Layout(message) if message.contains("open object rows")));
}

#[test]
fn generate_reaches_emitter_after_layout_classification() {
    let module = TypedModule {
        module: jisp_ir::Module::empty(),
        schemes: BTreeMap::from([("main".to_owned(), Scheme::mono(Type::Int))]),
        expression_types: Default::default(),
    };

    assert_eq!(generate(&module).unwrap().to_string(), "");
}

#[test]
fn generate_detailed_maps_rust_functions_to_jisp_definitions() {
    let source = jisp_core::SourceId(9);
    let definition_span = jisp_core::Span::new(source, 4, 22);
    let expression_span = jisp_core::Span::new(source, 15, 16);
    let module = TypedModule {
        module: jisp_ir::Module {
            imports: vec![],
            types: vec![],
            definitions: vec![jisp_ir::Definition {
                name: "main".to_owned(),
                public: true,
                value: jisp_ir::Expr::new(
                    jisp_ir::ExprKind::Literal(jisp_ir::Literal::Int(1)),
                    expression_span,
                ),
                span: definition_span,
            }],
            exports: vec![],
        },
        schemes: BTreeMap::from([("main".to_owned(), Scheme::mono(Type::Int))]),
        expression_types: Default::default(),
    };

    let generated = generate_detailed(&module).unwrap();

    assert!(generated.tokens.to_string().contains("pub fn main"));
    let item = generated
        .source_map
        .item(RustItemKind::Function, "main")
        .unwrap();
    assert_eq!(item.source_span, definition_span);
    let range = item.generated_range.as_ref().unwrap();
    let rendered = generated.tokens.to_string();
    assert_eq!(&rendered[range.clone()][..7], "fn main");
    assert_eq!(
        generated
            .source_map
            .item_at(range.start)
            .unwrap()
            .source_span,
        definition_span
    );
    let expression = generated
        .source_map
        .items
        .iter()
        .find(|item| item.kind == RustItemKind::Expression)
        .unwrap();
    let expression_range = expression.generated_range.as_ref().unwrap();
    assert_eq!(expression.source_span, expression_span);
    assert!(rendered[expression_range.clone()].contains("let __jisp_expr_"));
    assert_eq!(
        generated
            .source_map
            .item_at(expression_range.start)
            .unwrap()
            .source_span,
        expression_span
    );
}

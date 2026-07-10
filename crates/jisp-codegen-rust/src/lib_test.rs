use std::collections::BTreeMap;

use jisp_types::{ObjectRow, Scheme, Type, TypeVar, TypedModule};

use crate::{generate, CodegenError};

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
    };

    let error = generate(&module).unwrap_err();

    assert!(matches!(error, CodegenError::Layout(message) if message.contains("open object rows")));
}

#[test]
fn generate_reaches_emitter_after_layout_classification() {
    let module = TypedModule {
        module: jisp_ir::Module::empty(),
        schemes: BTreeMap::from([("main".to_owned(), Scheme::mono(Type::Int))]),
    };

    assert_eq!(generate(&module).unwrap().to_string(), "");
}

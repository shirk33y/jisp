use std::collections::BTreeMap;

use jisp_types::{ObjectRow, Scheme, Type, TypeVar, TypedModule};

use crate::layout::*;

fn closed_object(fields: impl IntoIterator<Item = (&'static str, Type)>) -> Type {
    Type::Object(ObjectRow {
        fields: fields
            .into_iter()
            .map(|(name, ty)| (name.to_owned(), ty))
            .collect(),
        rest: None,
    })
}

#[test]
fn classifies_closed_structural_objects_as_struct_layouts() {
    let layout = classify_type(&closed_object([
        ("active", Type::Bool),
        ("name", Type::Str),
    ]))
    .unwrap();

    let Layout::ClosedObject(object) = layout else {
        panic!("closed object row should classify as a closed object layout");
    };
    assert_eq!(object.fields["active"], Layout::Bool);
    assert_eq!(object.fields["name"], Layout::Str);
}

#[test]
fn recursively_classifies_nested_closed_structural_objects() {
    let layout = classify_type(&closed_object([(
        "user",
        closed_object([("age", Type::Int), ("name", Type::Str)]),
    )]))
    .unwrap();

    let Layout::ClosedObject(object) = layout else {
        panic!("closed object row should classify as a closed object layout");
    };
    assert!(matches!(
        object.fields["user"],
        Layout::ClosedObject(ClosedObjectLayout { .. })
    ));
}

#[test]
fn rejects_open_object_rows_in_native_layouts() {
    let error = classify_type(&Type::Object(ObjectRow {
        fields: BTreeMap::from([("name".to_owned(), Type::Str)]),
        rest: Some(TypeVar(7)),
    }))
    .unwrap_err();

    assert_eq!(error, LayoutError::OpenObjectRow { rest: TypeVar(7) });
}

#[test]
fn rejects_polymorphic_top_level_definitions() {
    let module = TypedModule {
        module: jisp_ir::Module::empty(),
        schemes: BTreeMap::from([(
            "id".to_owned(),
            Scheme {
                variables: vec![TypeVar(1)],
                body: Type::Function {
                    parameters: vec![Type::Var(TypeVar(1))],
                    rest: None,
                    result: Box::new(Type::Var(TypeVar(1))),
                },
            },
        )]),
        expression_types: Default::default(),
    };

    let error = classify_module(&module).unwrap_err();

    assert_eq!(
        error,
        LayoutError::PolymorphicDefinition {
            name: "id".to_owned(),
            variables: vec![TypeVar(1)],
        }
    );
}

#[test]
fn classifies_monomorphic_module_definitions() {
    let module = TypedModule {
        module: jisp_ir::Module::empty(),
        schemes: BTreeMap::from([(
            "main".to_owned(),
            Scheme::mono(Type::Function {
                parameters: vec![],
                rest: None,
                result: Box::new(Type::Str),
            }),
        )]),
        expression_types: Default::default(),
    };

    let layout = classify_module(&module).unwrap();

    assert_eq!(
        layout.definitions["main"],
        Layout::Function {
            parameters: vec![],
            rest: None,
            result: Box::new(Layout::Str),
        }
    );
}

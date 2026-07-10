use super::*;

#[test]
fn binds_a_variable_inside_a_list() {
    let var = TypeVar(0);
    let mut unifier = Unifier::default();
    let result = unifier
        .unify(
            Type::List(Box::new(Type::Var(var))),
            Type::List(Box::new(Type::Int)),
        )
        .unwrap();
    assert_eq!(result, Type::List(Box::new(Type::Int)));
    assert_eq!(unifier.substitution.get(var), Some(&Type::Int));
}

#[test]
fn rejects_recursive_types() {
    let var = TypeVar(0);
    let mut unifier = Unifier::default();
    assert!(matches!(
        unifier.unify(Type::Var(var), Type::List(Box::new(Type::Var(var)))),
        Err(UnifyError::Occurs { .. })
    ));
}

#[test]
fn unifies_function_types() {
    let mut unifier = Unifier::default();
    let variable = Type::Var(TypeVar(0));
    let result = unifier
        .unify(
            Type::Function {
                parameters: vec![variable.clone()],
                rest: None,
                result: Box::new(variable),
            },
            Type::Function {
                parameters: vec![Type::Str],
                rest: None,
                result: Box::new(Type::Str),
            },
        )
        .unwrap();
    assert_eq!(
        result,
        Type::Function {
            parameters: vec![Type::Str],
            rest: None,
            result: Box::new(Type::Str)
        }
    );
}

#[test]
fn unifies_variadic_function_with_extra_fixed_arguments() {
    let mut unifier = Unifier::default();
    let result_type = Type::Var(TypeVar(0));
    let result = unifier
        .unify(
            Type::Function {
                parameters: vec![Type::Str],
                rest: Some(Box::new(Type::Str)),
                result: Box::new(result_type.clone()),
            },
            Type::Function {
                parameters: vec![Type::Str, Type::Str, Type::Str],
                rest: None,
                result: Box::new(Type::Int),
            },
        )
        .unwrap();

    assert_eq!(
        result,
        Type::Function {
            parameters: vec![Type::Str, Type::Str, Type::Str],
            rest: Some(Box::new(Type::Str)),
            result: Box::new(Type::Int)
        }
    );
    assert_eq!(unifier.substitution.get(TypeVar(0)), Some(&Type::Int));
}

#[test]
fn rejects_too_few_arguments_for_variadic_function() {
    let mut unifier = Unifier::default();
    assert!(matches!(
        unifier.unify(
            Type::Function {
                parameters: vec![Type::Str],
                rest: Some(Box::new(Type::Str)),
                result: Box::new(Type::Str),
            },
            Type::Function {
                parameters: vec![],
                rest: None,
                result: Box::new(Type::Str),
            },
        ),
        Err(UnifyError::Arity { left: 1, right: 0 })
    ));
}

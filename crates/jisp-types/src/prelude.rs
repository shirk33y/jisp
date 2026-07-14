use std::collections::{BTreeMap, BTreeSet};

use crate::{ObjectRow, Scheme, Type, TypeVar};

pub fn environment() -> BTreeMap<String, Scheme> {
    let mut env = BTreeMap::new();

    add(
        &mut env,
        "ok",
        scheme(vec![0, 1], fun(vec![var(0)], result(var(0), var(1)))),
    );
    add(
        &mut env,
        "err",
        scheme(vec![0, 1], fun(vec![var(1)], result(var(0), var(1)))),
    );
    add(
        &mut env,
        "some",
        scheme(vec![0], fun(vec![var(0)], option(var(0)))),
    );
    add(&mut env, "none", scheme(vec![0], option(var(0))));
    add(&mut env, "bigint", mono(fun(vec![Type::Str], Type::BigInt)));

    for name in ["+", "-", "*", "/", "//", "%"] {
        add(
            &mut env,
            name,
            mono(fun(vec![Type::Int, Type::Int], Type::Int)),
        );
    }
    add(
        &mut env,
        "=",
        scheme(vec![0], fun(vec![var(0), var(0)], Type::Bool)),
    );
    for name in ["<", ">", "<=", ">="] {
        add(
            &mut env,
            name,
            mono(fun(vec![Type::Int, Type::Int], Type::Bool)),
        );
    }
    add(&mut env, "math.abs", mono(fun(vec![Type::Int], Type::Int)));
    for name in ["math.min", "math.max", "math.pow"] {
        add(
            &mut env,
            name,
            mono(fun(vec![Type::Int, Type::Int], Type::Int)),
        );
    }
    for name in ["math.sqrt", "math.floor", "math.ceil", "math.round"] {
        add(&mut env, name, mono(fun(vec![Type::Float], Type::Float)));
    }

    add(
        &mut env,
        "str.is",
        scheme(vec![0], fun(vec![var(0)], Type::Bool)),
    );
    add(
        &mut env,
        "str.from",
        scheme(vec![0], fun(vec![var(0)], Type::Str)),
    );
    add(&mut env, "str.len", mono(fun(vec![Type::Str], Type::Int)));
    add(
        &mut env,
        "str.cat",
        mono(variadic_fun(vec![], Type::Str, Type::Str)),
    );
    add(
        &mut env,
        "str.join",
        mono(fun(vec![Type::Str, list(Type::Str)], Type::Str)),
    );
    add(
        &mut env,
        "str.split",
        mono(fun(vec![Type::Str, Type::Str], list(Type::Str))),
    );
    for name in ["str.trim", "str.upper", "str.lower"] {
        add(&mut env, name, mono(fun(vec![Type::Str], Type::Str)));
    }
    for name in ["str.has", "str.starts", "str.ends"] {
        add(
            &mut env,
            name,
            mono(fun(vec![Type::Str, Type::Str], Type::Bool)),
        );
    }
    add(
        &mut env,
        "str.replace",
        mono(fun(vec![Type::Str, Type::Str, Type::Str], Type::Str)),
    );
    add(
        &mut env,
        "str.slice",
        mono(fun(
            vec![Type::Str, Type::Int, Type::Int],
            result(Type::Str, Type::Str),
        )),
    );

    add(
        &mut env,
        "list.len",
        scheme(vec![0], fun(vec![list(var(0))], Type::Int)),
    );
    add(
        &mut env,
        "list.is",
        scheme(vec![0], fun(vec![var(0)], Type::Bool)),
    );
    add(
        &mut env,
        "list.get",
        scheme(
            vec![0],
            fun(vec![list(var(0)), Type::Int], result(var(0), Type::Str)),
        ),
    );
    for name in ["list.first", "list.last"] {
        add(
            &mut env,
            name,
            scheme(vec![0], fun(vec![list(var(0))], result(var(0), Type::Str))),
        );
    }
    add(
        &mut env,
        "list.rest",
        scheme(vec![0], fun(vec![list(var(0))], list(var(0)))),
    );
    add(
        &mut env,
        "list.slice",
        scheme(
            vec![0],
            fun(
                vec![list(var(0)), Type::Int, Type::Int],
                result(list(var(0)), Type::Str),
            ),
        ),
    );
    add(
        &mut env,
        "list.map",
        scheme(
            vec![0, 1],
            fun(vec![fun(vec![var(0)], var(1)), list(var(0))], list(var(1))),
        ),
    );
    add(
        &mut env,
        "list.filter",
        scheme(
            vec![0],
            fun(
                vec![fun(vec![var(0)], Type::Bool), list(var(0))],
                list(var(0)),
            ),
        ),
    );
    add(
        &mut env,
        "list.fold",
        scheme(
            vec![0, 1],
            fun(
                vec![fun(vec![var(1), var(0)], var(1)), var(1), list(var(0))],
                var(1),
            ),
        ),
    );
    for name in ["list.some", "list.every"] {
        add(
            &mut env,
            name,
            scheme(
                vec![0],
                fun(
                    vec![fun(vec![var(0)], Type::Bool), list(var(0))],
                    Type::Bool,
                ),
            ),
        );
    }
    add(
        &mut env,
        "list.has",
        scheme(vec![0], fun(vec![list(var(0)), var(0)], Type::Bool)),
    );
    add(
        &mut env,
        "list.cat",
        scheme(vec![0], variadic_fun(vec![], list(var(0)), list(var(0)))),
    );
    add(
        &mut env,
        "list.prepend",
        scheme(vec![0], fun(vec![var(0), list(var(0))], list(var(0)))),
    );
    add(
        &mut env,
        "list.append",
        scheme(vec![0], fun(vec![list(var(0)), var(0)], list(var(0)))),
    );

    add(
        &mut env,
        "obj.is",
        scheme(vec![0], fun(vec![var(0)], Type::Bool)),
    );
    add(
        &mut env,
        "obj.len",
        scheme(vec![0], fun(vec![object_row(0)], Type::Int)),
    );
    add(
        &mut env,
        "obj.has",
        scheme(vec![0], fun(vec![object_row(0), Type::Str], Type::Bool)),
    );
    add(
        &mut env,
        "obj.get",
        scheme(
            vec![0, 1],
            fun(vec![object_row(1), Type::Str], result(var(0), Type::Str)),
        ),
    );
    add(
        &mut env,
        "obj.set",
        scheme(
            vec![0, 1],
            fun(vec![object_row(0), Type::Str, var(1)], object_row(0)),
        ),
    );
    add(
        &mut env,
        "obj.del",
        scheme(vec![0], fun(vec![object_row(0), Type::Str], object_row(0))),
    );
    add(
        &mut env,
        "obj.keys",
        scheme(vec![0], fun(vec![object_row(0)], list(Type::Str))),
    );
    add(
        &mut env,
        "obj.values",
        scheme(vec![0, 1], fun(vec![object_row(1)], list(var(0)))),
    );
    add(
        &mut env,
        "obj.to-map",
        scheme(vec![0, 1], fun(vec![object_row(1)], map_type(var(0)))),
    );
    add(
        &mut env,
        "obj.cat",
        scheme(vec![0], variadic_fun(vec![], object_row(0), object_row(0))),
    );
    add(
        &mut env,
        "map.len",
        scheme(vec![0], fun(vec![map_type(var(0))], Type::Int)),
    );
    add(
        &mut env,
        "map.has",
        scheme(vec![0], fun(vec![map_type(var(0)), Type::Str], Type::Bool)),
    );
    add(
        &mut env,
        "map.get",
        scheme(
            vec![0],
            fun(vec![map_type(var(0)), Type::Str], result(var(0), Type::Str)),
        ),
    );
    add(
        &mut env,
        "map.set",
        scheme(
            vec![0],
            fun(vec![map_type(var(0)), Type::Str, var(0)], map_type(var(0))),
        ),
    );
    add(
        &mut env,
        "map.del",
        scheme(
            vec![0],
            fun(vec![map_type(var(0)), Type::Str], map_type(var(0))),
        ),
    );
    add(
        &mut env,
        "map.keys",
        scheme(vec![0], fun(vec![map_type(var(0))], list(Type::Str))),
    );
    add(
        &mut env,
        "map.values",
        scheme(vec![0], fun(vec![map_type(var(0))], list(var(0)))),
    );
    add(
        &mut env,
        "map.cat",
        scheme(
            vec![0],
            variadic_fun(vec![], map_type(var(0)), map_type(var(0))),
        ),
    );
    add(
        &mut env,
        "ui.html",
        scheme(vec![0], fun(vec![var(0)], Type::Str)),
    );
    add(
        &mut env,
        "ui.node",
        scheme(vec![0], fun(vec![var(0)], ui_node())),
    );
    add(
        &mut env,
        "ui.result",
        scheme(
            vec![0, 1, 2],
            fun(
                vec![var(0), list(var(1)), list(var(2))],
                ui_update_result(var(0)),
            ),
        ),
    );

    add(
        &mut env,
        "result.try",
        scheme(
            vec![0, 1, 2],
            fun(
                vec![
                    result(var(0), var(2)),
                    fun(vec![var(0)], result(var(1), var(2))),
                ],
                result(var(1), var(2)),
            ),
        ),
    );
    add(
        &mut env,
        "result.map",
        scheme(
            vec![0, 1, 2],
            fun(
                vec![result(var(0), var(2)), fun(vec![var(0)], var(1))],
                result(var(1), var(2)),
            ),
        ),
    );
    add(
        &mut env,
        "result.map-err",
        scheme(
            vec![0, 1, 2],
            fun(
                vec![result(var(0), var(1)), fun(vec![var(1)], var(2))],
                result(var(0), var(2)),
            ),
        ),
    );
    add(
        &mut env,
        "result.recover",
        scheme(
            vec![0, 1, 2],
            fun(
                vec![
                    result(var(0), var(1)),
                    fun(vec![var(1)], result(var(0), var(2))),
                ],
                result(var(0), var(2)),
            ),
        ),
    );

    add(
        &mut env,
        "io.println",
        scheme(vec![0], fun(vec![var(0)], Type::Null)),
    );

    env
}

pub(crate) fn overloads() -> BTreeMap<String, Vec<Scheme>> {
    let mut env = BTreeMap::new();

    for name in ["+", "-", "*", "/", "//", "%"] {
        add_overloads(
            &mut env,
            name,
            vec![
                mono(fun(vec![Type::Int, Type::Int], Type::Int)),
                mono(fun(vec![Type::BigInt, Type::BigInt], Type::BigInt)),
                mono(fun(vec![Type::Float, Type::Float], Type::Float)),
            ],
        );
    }

    for name in ["<", ">", "<=", ">="] {
        add_overloads(
            &mut env,
            name,
            vec![
                mono(fun(vec![Type::Int, Type::Int], Type::Bool)),
                mono(fun(vec![Type::BigInt, Type::BigInt], Type::Bool)),
                mono(fun(vec![Type::Float, Type::Float], Type::Bool)),
                mono(fun(vec![Type::Str, Type::Str], Type::Bool)),
            ],
        );
    }

    add_overloads(
        &mut env,
        "math.abs",
        vec![
            mono(fun(vec![Type::Int], Type::Int)),
            mono(fun(vec![Type::BigInt], Type::BigInt)),
            mono(fun(vec![Type::Float], Type::Float)),
        ],
    );
    for name in ["math.min", "math.max"] {
        add_overloads(
            &mut env,
            name,
            vec![
                mono(fun(vec![Type::Int, Type::Int], Type::Int)),
                mono(fun(vec![Type::BigInt, Type::BigInt], Type::BigInt)),
                mono(fun(vec![Type::Float, Type::Float], Type::Float)),
            ],
        );
    }
    add_overloads(
        &mut env,
        "math.pow",
        vec![
            mono(fun(vec![Type::Int, Type::Int], Type::Int)),
            mono(fun(vec![Type::Float, Type::Float], Type::Float)),
        ],
    );

    env
}

pub fn variants() -> BTreeMap<String, BTreeSet<String>> {
    BTreeMap::from([
        ("option".to_owned(), tags(["none", "some"])),
        ("result".to_owned(), tags(["err", "ok"])),
    ])
}

fn add(env: &mut BTreeMap<String, Scheme>, name: &str, scheme: Scheme) {
    env.insert(name.to_owned(), scheme);
}

fn add_overloads(env: &mut BTreeMap<String, Vec<Scheme>>, name: &str, schemes: Vec<Scheme>) {
    env.insert(name.to_owned(), schemes);
}

fn mono(body: Type) -> Scheme {
    Scheme::mono(body)
}

fn scheme(vars: Vec<u32>, body: Type) -> Scheme {
    Scheme {
        variables: vars.into_iter().map(TypeVar).collect(),
        body,
    }
}

fn var(id: u32) -> Type {
    Type::Var(TypeVar(id))
}

fn list(item: Type) -> Type {
    Type::List(Box::new(item))
}

fn map_type(value: Type) -> Type {
    Type::Map(Box::new(value))
}

fn object_row(rest: u32) -> Type {
    Type::Object(ObjectRow {
        fields: BTreeMap::new(),
        rest: Some(TypeVar(rest)),
    })
}

fn fun(parameters: Vec<Type>, result: Type) -> Type {
    Type::Function {
        parameters,
        rest: None,
        result: Box::new(result),
    }
}

fn variadic_fun(parameters: Vec<Type>, rest: Type, result: Type) -> Type {
    Type::Function {
        parameters,
        rest: Some(Box::new(rest)),
        result: Box::new(result),
    }
}

fn result(ok: Type, err: Type) -> Type {
    Type::Named {
        name: "result".to_owned(),
        arguments: vec![ok, err],
    }
}

fn option(item: Type) -> Type {
    Type::Named {
        name: "option".to_owned(),
        arguments: vec![item],
    }
}

fn ui_node() -> Type {
    Type::Named {
        name: "ui.node".to_owned(),
        arguments: vec![],
    }
}

fn ui_update_result(state: Type) -> Type {
    Type::Named {
        name: "ui.update-result".to_owned(),
        arguments: vec![state],
    }
}

fn tags<const N: usize>(names: [&str; N]) -> BTreeSet<String> {
    names.into_iter().map(str::to_owned).collect()
}

# Native Rust subset

`jisp_macros::lisp_file!` compiles a checked Jisp module through the normal
parser, expander, lowerer, type checker, and `jisp-codegen-rust`. It is a
concrete-layout subset, not a second interpreter and not a `Value` ABI.

`docs/native-support.json` is the source of truth. The macro integration test
checks every row against its fixture, owning test, and the IDs in this table.

| id | area | source fixture | interpreter result | native status | native expectation |
| --- | --- | --- | --- | --- | --- |
| scalars | values | differential.lisp | scalar exports | supported | concrete scalar layouts |
| strings-lists | collections | differential.lisp | string/list exports | supported | `String`, `Vec<T>` |
| list-callbacks | collections | differential.lisp | map/filter/fold/some/every exports | supported | typed callbacks |
| closed-objects | collections | differential.lisp | static field/get exports | supported | generated closed-row structs |
| collection-snapshots | collections | differential.lisp | immutable update exports | supported | immutable snapshots |
| homogeneous-dynamic-objects | collections | differential.lisp | dynamic homogeneous reads/updates | supported | key dispatch over one field type |
| maps | collections | differential.lisp | map helper exports | supported | `IndexMap<String, A>` |
| functions-closures | functions | differential.lisp | local/returned closures | supported | typed concrete calls/closures |
| variadics | functions | differential.lisp | rest-argument exports | supported | concrete `Vec<T>` rest values |
| imports | modules | imports/main.lisp | imported-entry | supported | resolved, Cargo-tracked imports |
| macros | modules | macro-normalizer/main.lisp | `42` | supported | expansion before codegen |
| result-helpers | control-flow | differential.lisp | result helper exports | supported | concrete result layouts |
| option-case | control-flow | differential.lisp | option case export | supported | concrete option enum |
| enum-case | control-flow | differential.lisp | declared enum case export | supported | generated Rust enum |
| patterns | control-flow | differential.lisp | nested list/object alternatives | supported | structural native matches |
| bigint | values | bigint.lisp | bigint entry | supported | `num_bigint::BigInt` |
| ui-values | native-boundary | unsupported_ui_html.lisp | UI host required | rejected | Jisp diagnostic at macro expansion |
| heterogeneous-dynamic-objects | native-boundary | collection-toolbox/unsupported.lisp | type error | rejected | Jisp diagnostic at macro expansion |
| open-row-polymorphism | native-boundary | collection-toolbox/open-row.lisp | generic row function | rejected | concrete-layout diagnostic at macro expansion |

## Rejections and diagnostics

Native compilation deliberately rejects host/UI values, dynamic access to a
heterogeneous object, and polymorphic row definitions. A downstream crate must
receive a Jisp diagnostic during macro expansion; it must not receive opaque
generated-Rust errors. Unresolved types and unrepresentable function values are
likewise outside the native ABI. There is no source form that asks codegen to
fall back to `jisp_eval::Value`.

## ABI rules

- Scalars use native Rust scalars; strings are owned `String`; lists are
  `Vec<T>`; maps are `indexmap::IndexMap<String, T>`; bigints are
  `num_bigint::BigInt`.
- Closed `obj` rows become generated structs. Native operations clone/update
  snapshots instead of exposing shared mutable runtime values.
- Functions and closures carry their inferred concrete types. Variadic direct
  calls pass a concrete `Vec<T>` rest value; codegen does not expose a dynamic
  callable ABI.
- Enums, `option`, and `result` use generated concrete Rust enum layouts.

## Parity policy

Every supported row has a differential test: evaluate the same source with the
interpreter, invoke its native export, and compare structural output. Every
rejected source row has a downstream proc-macro compile-fail test. New native
claims require an inventory row, a fixture, and one of those tests before this
table changes.

Portable language fixtures are the long-term semantic source of those rows.
The inventory will link each portable fixture/test identity to exactly one
native obligation: supported parity, intentional rejection, or
interpreter-only. It must not infer native eligibility from source syntax.
Native-only fixtures remain valid for generated-Rust ABI, proc-macro, and
diagnostic-remapping integration coverage.

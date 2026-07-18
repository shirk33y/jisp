# Native Rust subset

`jisp_macros::lisp_file!` compiles a checked Jisp module through the normal
parser, expander, lowerer, type checker, and `jisp-codegen-rust`. It is a
concrete-layout subset, not a second interpreter and not a `Value` ABI.

`docs/native-support.json` is the source of truth. The macro integration test
checks every row against its fixture, owning test, portable-test ID, and the
backend obligation in this table.

| id | source fixture | portable test id | backend obligation | native expectation |
| --- | --- | --- | --- | --- |
| scalars | differential.lisp | core-control: sequential `let` | supported | concrete scalar layouts |
| strings-lists | differential.lisp | lists: concat/prepend/append | supported | `String`, `Vec<T>` |
| list-callbacks | differential.lisp | list-pipeline: map/filter/fold | supported | typed callbacks |
| closed-objects | differential.lisp | objects-ui: static field | supported | generated closed-row structs |
| collection-snapshots | differential.lisp | objects-ui: immutable set/delete | supported | immutable snapshots |
| homogeneous-dynamic-objects | differential.lisp | objects-ui: homogeneous dynamic map | supported | key dispatch over one field type |
| maps | differential.lisp | objects-ui: stable keys/values | supported | `IndexMap<String, A>` |
| list-get-boundaries | differential.lisp | — native-only | supported | concrete `ok`/`err` result branches |
| list-slice-boundaries | differential.lisp | — native-only | supported | valid and out-of-bounds ranges |
| empty-list-callbacks | differential.lisp | — native-only | supported | typed empty list after filter/map |
| object-view-helpers | differential.lisp | — native-only | supported | `len`, `has`, keys, values, `to-map` |
| map-view-helpers | differential.lisp | — native-only | supported | `cat`, `len`, `has`, keys, values |
| pattern-fallback | differential.lisp | — native-only | supported | failed nested alternative reaches `_` |
| functions-closures | differential.lisp | functions-scope: closures | supported | typed concrete calls/closures |
| variadics | differential.lisp | functions-scope: rest list | supported | concrete `Vec<T>` rest values |
| imports | imports/main.lisp | — native-only | supported | resolved, Cargo-tracked imports |
| macros | macro-normalizer/main.lisp | macros-quote: expansion | supported | expansion before codegen |
| result-helpers | differential.lisp | results-options: result map | supported | concrete result layouts |
| option-case | differential.lisp | results-options: option case | supported | concrete option enum |
| enum-case | differential.lisp | case-patterns: variant case | supported | generated Rust enum |
| patterns | differential.lisp | case-patterns: nested list | supported | structural native matches |
| bigint | differential.lisp | bigint: arithmetic | supported | `num_bigint::BigInt` |
| ui-values | unsupported_ui_html.lisp | ui: raw command object rejection | intentionally-rejected | Jisp diagnostic at macro expansion |
| heterogeneous-dynamic-objects | collection-toolbox/unsupported.lisp | — native-only | intentionally-rejected | Jisp diagnostic at macro expansion |
| open-row-polymorphism | collection-toolbox/open-row.lisp | — native-only | intentionally-rejected | concrete-layout diagnostic at macro expansion |

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

Every `supported` row has a differential test: evaluate its fixture with the
interpreter, invoke its native export, and compare structural output. Every
`intentionally-rejected` row has a downstream proc-macro compile-fail test.
The inventory verifies the fixture, named test, stable portable ID (when one
exists), one-to-one portable mapping, and a concrete runner before the suite
runs. The runner executes every declared native obligation. A portable ID
anchors the semantic contract; a native integration fixture may remain distinct
when it proves proc-macro, concrete ABI, or diagnostic-remapping behavior. New
native claims require an inventory row and a matching test before this table
changes.

Portable language fixtures are the long-term semantic source of those rows.
The inventory links each portable fixture/test identity to at most one native
obligation: `supported`, `intentionally-rejected`, or `interpreter-only`.
It must not infer native eligibility from source syntax.
Native-only fixtures remain valid for generated-Rust ABI, proc-macro, and
diagnostic-remapping integration coverage.

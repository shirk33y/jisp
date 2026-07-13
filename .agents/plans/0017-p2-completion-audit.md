# P2 completion audit

This is an evidence-based audit of the P2 section in `TODO.md`. It does not
mark P2 complete: every unchecked row below remains work, not a suggestion to
silently narrow the milestone.

## Verified P2 work

| Contract | Evidence |
| --- | --- |
| Bigints and concrete native helpers | Portable language fixtures, native differential tests, and `num_bigint::BigInt` emission. |
| Callback-last `use`, UI proof, and portable runner | `tests/language/`, `examples/ui_button.lisp`, and evaluator/Cargo integration tests. |
| Typed native functions, closures, variadics, results/options, homogeneous dynamic object reads, and homogeneous maps | `crates/jisp-macros/tests/native_differential.rs`, `crates/jisp-types/src/infer_test.rs`, and `crates/jisp/tests/codegen_rust.rs`. |
| Template macros with local binding hygiene and origin diagnostics | `jisp-expand` tests, facade expansion tests, and `.agents/plans/0010-user-macros.md`. |
| Case aliases, guards, alternatives, and nested list/object alternatives | parser/lowering/type/evaluator tests plus native differential coverage; nested native alternative emission is in `e4e46d6`, and finite list/object coverage is in `817dd59`. |
| Native diagnostics at generated item granularity | `RustSourceMap`, CLI Cargo JSON remapping, and `jisp native-check` tests. |
| Resolved, monomorphic export schema | `crates/jisp/tests/export_schema.rs` and `jisp export-schema`. |
| Formatter, stateful REPL, package initialization/entry execution, and basic LSP | CLI tests and README contract. LSP includes diagnostics, completion, hover, and top-level local/imported definition lookup. |
| Native immutable value semantics | `d3d7e88` emits clones for local Jisp values, with interpreter/native differential tests for collection updates and reusable non-mutating values. |

## Remaining public contracts

| Requirement in `TODO.md` | Why existing code is insufficient | Completion evidence required |
| --- | --- | --- |
| Heterogeneous dynamic reads and open object rows | Homogeneous runtime dictionaries are now explicit `map<str, A>` values. A row tail still has no value-type ABI, and heterogeneous dynamic lookup still has no Jisp result type. The native backend correctly rejects it rather than using `Value`. | A source-visible finite selection/dynamic JSON value proposal, plus open-row monomorphisation if native generic field access is required. See [0016](0016-native-open-object-abi.md). |
| Cross-module, general compile-time macros | Current template macros are ordered and local by design. Local template-introduced bindings are hygienic, but imports are not macro exports and the macro body is not a compile-time evaluator. | Macro import/export rules, evaluator capability/sandboxing rules, dependency/cycle diagnostics, and all three syntax tests. |
| Full list/object exhaustiveness | Finite nested alternatives are handled, but arbitrary list/object domains and multi-field relational coverage have no proof engine. | A documented coverage algorithm with redundancy/exhaustiveness regression suite; it must preserve guard conservatism. |
| Generic export-schema instantiations | `export_schema` rejects schemes with variables because CLI has no type-instantiation input language. | Public instantiation syntax/API, type parser/resolver, recursive named-type handling, CLI/docs/tests. |
| Package registry tooling | Local path dependencies are supported, but there is no registry/lock design. | Manifest schema, lock/registry decision, and offline tests. |

## Completion gate

P2 can be marked complete only after each remaining row has a public contract,
implementation across the frontend/runtime/native seams that it affects, tests
at the narrowest and end-to-end layers, current docs, and a clean full local
validation run. A design note alone is not completion evidence.

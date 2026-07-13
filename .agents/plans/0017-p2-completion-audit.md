# P2 completion audit

This is an evidence-based audit of the P2 section in `TODO.md`. It does not
mark P2 complete: every unchecked row below remains work, not a suggestion to
silently narrow the milestone.

## Verified P2 work

| Contract | Evidence |
| --- | --- |
| Bigints and concrete native helpers | Portable language fixtures, native differential tests, and `num_bigint::BigInt` emission. |
| Callback-last `use`, UI proof, and portable runner | `tests/language/`, `examples/ui_button.lisp`, and evaluator/Cargo integration tests. |
| Typed native functions, closures, variadics, results/options, and homogeneous dynamic object reads | `crates/jisp-macros/tests/native_differential.rs` and `crates/jisp/tests/codegen_rust.rs`. |
| Template macros with origin diagnostics | `jisp-expand` tests and `.agents/plans/0010-user-macros.md`. |
| Case aliases, guards, alternatives, and nested list/object alternatives | parser/lowering/type/evaluator tests plus native differential coverage; nested native alternative emission is in `e4e46d6`, and finite list/object coverage is in `817dd59`. |
| Native diagnostics at generated item granularity | `RustSourceMap`, CLI Cargo JSON remapping, and `jisp native-check` tests. |
| Resolved, monomorphic export schema | `crates/jisp/tests/export_schema.rs` and `jisp export-schema`. |
| Formatter, stateful REPL, package initialization/entry execution, and basic LSP | CLI tests and README contract. LSP includes diagnostics, completion, hover, and top-level local/imported definition lookup. |
| Native immutable value semantics | `d3d7e88` emits clones for local Jisp values, with interpreter/native differential tests for collection updates and reusable non-mutating values. |

## Remaining public contracts

| Requirement in `TODO.md` | Why existing code is insufficient | Completion evidence required |
| --- | --- | --- |
| Dynamic deletion, heterogeneous reads, and open object rows | A row tail has no value-type ABI; heterogeneous dynamic lookup has no Jisp result type. The native backend correctly rejects it rather than using `Value`. | A specified source-visible map/selection type, parser/type/runtime/schema/native implementation, and differential tests. See [0016](0016-native-open-object-abi.md). |
| Hygienic, cross-module, general compile-time macros | Current template macros are ordered and local by design; template identifiers can capture and imports are not macro exports. | Hygiene model, macro import/export rules, evaluator capability/sandboxing rules, origin diagnostics, and all three syntax tests. |
| Full list/object exhaustiveness | Finite nested alternatives are handled, but arbitrary list/object domains and multi-field relational coverage have no proof engine. | A documented coverage algorithm with redundancy/exhaustiveness regression suite; it must preserve guard conservatism. |
| Structured secondary native diagnostics and macro-origin remapping | Primary Cargo errors now select stable expression/item ranges. | Preserve and render rustc secondary labels, then attach macro-origin chains in diagnostic fixtures. |
| Generic export-schema instantiations | `export_schema` rejects schemes with variables because CLI has no type-instantiation input language. | Public instantiation syntax/API, type parser/resolver, recursive named-type handling, CLI/docs/tests. |
| Local-binding definition and package dependency/registry tooling | IR retains no name spans for lambda/let bindings; package manifest only has package metadata and entry. | Binding-span representation through lowering, LSP locations/ranges tests, manifest schema, local dependency resolver, lock/registry decision, and offline tests. |

## Completion gate

P2 can be marked complete only after each remaining row has a public contract,
implementation across the frontend/runtime/native seams that it affects, tests
at the narrowest and end-to-end layers, current docs, and a clean full local
validation run. A design note alone is not completion evidence.

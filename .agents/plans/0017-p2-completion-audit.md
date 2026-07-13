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
| Case aliases, guards, alternatives, and nested list/object alternatives | parser/lowering/type/evaluator tests plus native differential coverage; nested native alternative emission is in `e4e46d6`, finite list/object coverage is in `817dd59`, and guarded branches after unguarded full coverage are rejected as unreachable in `e594315`. |
| Native diagnostics at generated item granularity | `RustSourceMap`, CLI Cargo JSON remapping, and `jisp native-check` tests. |
| Resolved export schema with explicit generic and recursive instantiations | `crates/jisp/tests/export_schema.rs` and `jisp export-schema --type`. |
| Formatter, stateful REPL, package initialization/entry execution, local dependency lockfiles, and basic LSP | CLI tests and README contract. LSP includes diagnostics, completion, hover, and top-level local/imported definition lookup. |
| Native immutable value semantics | `d3d7e88` emits clones for local Jisp values, with interpreter/native differential tests for collection updates and reusable non-mutating values. |

## Remaining public contracts

| Requirement in `TODO.md` | Why existing code is insufficient | Completion evidence required |
| --- | --- | --- |
| Heterogeneous dynamic reads and open object rows | Homogeneous runtime dictionaries are now explicit `map<str, A>` values, and homogeneous closed objects can be converted with `obj.to-map` before runtime-sized map updates such as dynamic deletion. A row tail still has no value-type ABI, and heterogeneous dynamic lookup still has no Jisp result type. The native backend correctly rejects it rather than using `Value`. | A source-visible finite selection/dynamic JSON value proposal, plus open-row monomorphisation if native generic field access is required. See [0016](0016-native-open-object-abi.md). |
| Cross-module, general compile-time macros | Current template macros are ordered and local by design. Local template-introduced bindings are hygienic, exporting macros is an expansion error across Lisp, JSON, and YAML, and `macro-import` is reserved with a dedicated not-implemented diagnostic. Runtime imports are still not macro exports and the macro body is not a compile-time evaluator. | Implement `macro-import` dependency resolution, evaluator capability/sandboxing rules if adopted, dependency/cycle diagnostics, and cross-module macro tests. |
| Full list/object exhaustiveness | The shipped contract now proves finite enum/bool/null coverage, finite list lengths, exact finite-list products, nested finite object refinements, and finite object-field products up to the documented 256-combination cap. Missing diagnostics name finite list/object combinations when they are known, redundant branches after full finite enum/bool/null, list, or object-product coverage are rejected, and guard branches after such unguarded full coverage are rejected as unreachable. It deliberately does not attempt arbitrary list/object domains or guard-sensitive relational proofs. | Future expansion needs a general pattern-matrix design for arbitrary structural domains, richer open-domain missing-pattern diagnostics, and guard-sensitive reachability without weakening the current conservative guard rule. |
| Package registry tooling | Local path dependencies and lockfiles are supported. Registry dependency specs now have a documented manifest shape, source/index decision, checksum policy, an offline cache-hit path that verifies locked version/SHA-256 source checksums, `jisp lock` preservation for used registry cache entries, and local file-index cache population into `.jisp/cache`. Remote registry lookup and archive downloads do not exist yet. | Remote registry index/download policy if that remains in scope, plus end-to-end remote registry tests. |

## Completion gate

P2 can be marked complete only after each remaining row has a public contract,
implementation across the frontend/runtime/native seams that it affects, tests
at the narrowest and end-to-end layers, current docs, and a clean full local
validation run. A design note alone is not completion evidence.

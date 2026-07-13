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
| Template macros with local binding hygiene, origin diagnostics, and aliased file-module imports | `jisp-expand` tests, facade expansion tests, and `.agents/plans/0010-user-macros.md`; path-aware facade coverage now expands imported macros as `alias.name`, while portable module-level error fixture coverage asserts that unresolved raw `macro-import` cannot reach lowering. |
| Case aliases, guards, alternatives, and nested list/object alternatives | parser/lowering/type/evaluator tests plus native differential coverage; nested native alternative emission is in `e4e46d6`, finite list/object coverage is in `817dd59`, guarded branches after unguarded full coverage are rejected as unreachable in `e594315`, and portable positive/negative `.lisp` fixtures now cover guard dispatch, redundant guarded branches, and non-exhaustive finite lists. |
| Native diagnostics at generated item granularity | `RustSourceMap`, CLI Cargo JSON remapping, and `jisp native-check` tests. |
| Resolved export schema with explicit generic and recursive instantiations | `crates/jisp/tests/export_schema.rs` and `jisp export-schema --type`. |
| Formatter, stateful REPL, package initialization/entry execution, local dependency lockfiles, and basic LSP | CLI tests and README contract. LSP includes diagnostics, completion, hover, and top-level local/imported definition lookup. |
| Native immutable value semantics | `d3d7e88` emits clones for local Jisp values, with interpreter/native differential tests for collection updates and reusable non-mutating values. |

## Remaining public contracts

| Requirement in `TODO.md` | Why existing code is insufficient | Completion evidence required |
| --- | --- | --- |
| Heterogeneous dynamic reads and open object rows | Homogeneous runtime dictionaries are now explicit `map<str, A>` values, and homogeneous closed objects can be converted with `obj.to-map` before runtime-sized map updates such as dynamic deletion. Heterogeneous dynamic lookup has no implicit Jisp result type and is rejected by the type checker unless the key is static; the native backend also rejects unsupported heterogeneous dynamic field and `obj.get` access instead of using `Value`, with direct regression tests. A row tail still has no value-type ABI. | A source-visible finite selection/dynamic JSON value proposal, plus open-row monomorphisation if native generic field access is required. See [0016](0016-native-open-object-abi.md). |
| Cross-module, general compile-time macros | Current template macros are ordered and bounded to quote/quasiquote templates. Local template-introduced bindings are hygienic, exporting macros is an expansion error across Lisp, JSON, and YAML, path-aware `macro-import` can import file-module template macros under an alias, transitive macro-import source files are included in facade/native/proc-macro dependency tracking, and transitive macro-import cycles are rejected with import-cycle diagnostics. Runtime imports are still not macro exports and the macro body is not a general compile-time evaluator. | Add evaluator capability/sandboxing rules only if a future general compile-time evaluator is adopted. |
| Full list/object exhaustiveness | The shipped contract now proves finite enum/bool/null coverage, finite list lengths, exact finite-list products, nested finite object refinements, and finite object-field products up to the documented 256-combination cap. Missing diagnostics name finite list/object combinations when they are known, redundant branches after full finite enum/bool/null, list, or object-product coverage are rejected, and guard branches after such unguarded full coverage are rejected as unreachable. It deliberately does not attempt arbitrary list/object domains or guard-sensitive relational proofs. | Future expansion needs a general pattern-matrix design for arbitrary structural domains, richer open-domain missing-pattern diagnostics, and guard-sensitive reachability without weakening the current conservative guard rule. |
| Package registry tooling | Local path dependencies and lockfiles are supported. Registry dependency specs now have a documented manifest shape, source/index decision, checksum policy, an offline cache-hit path that verifies locked version/SHA-256 source checksums, `jisp lock` preservation for used registry cache entries, local file-index cache population into `.jisp/cache`, and explicit `jisp lock` rejection for `http://`/`https://` remote registry URLs. Remote registry lookup and archive downloads do not exist yet. | Remote registry index/download policy if that remains in scope, plus end-to-end remote registry tests. |

## Completion gate

P2 can be marked complete only after each remaining row has a public contract,
implementation across the frontend/runtime/native seams that it affects, tests
at the narrowest and end-to-end layers, current docs, and a clean full local
validation run. A design note alone is not completion evidence.

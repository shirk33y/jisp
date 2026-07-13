# P2 completion audit

This is an evidence-based audit of the P2 section in `TODO.md`. It marks P2
complete for the current language contract and concrete native ABI. Items that
would require a broader contract are recorded as deferred work, not silently
implemented through a universal dynamic runtime value.

## Verified P2 work

| Contract | Evidence |
| --- | --- |
| Bigints and concrete native helpers | Portable language fixtures, native differential tests, and `num_bigint::BigInt` emission. |
| Callback-last `use`, UI proof, and portable runner | `tests/language/`, `examples/ui_button.lisp`, and evaluator/Cargo integration tests. |
| Typed native functions, closures, variadics, results/options, homogeneous dynamic object reads, and homogeneous maps | `crates/jisp-macros/tests/native_differential.rs`, `crates/jisp-types/src/infer_test.rs`, and `crates/jisp/tests/codegen_rust.rs`. |
| Template macros with local binding hygiene, origin diagnostics, and aliased file-module imports | `jisp-expand` tests, facade expansion tests, and `.agents/plans/0010-user-macros.md`; path-aware facade coverage expands imported macros as `alias.name`, while portable module-level error fixture coverage asserts that unresolved raw `macro-import` cannot reach lowering. |
| Macro import dependency tracking and cycle rejection | Facade/native/proc-macro dependency tests include direct and transitive `macro-import` files; transitive cycles fail with import-cycle diagnostics. |
| Case aliases, guards, alternatives, and nested list/object alternatives | Parser/lowering/type/evaluator tests plus native differential coverage; nested native alternative emission is in `e4e46d6`, finite list/object coverage is in `817dd59`, guarded branches after unguarded full coverage are rejected as unreachable in `e594315`, and portable positive/negative `.lisp` fixtures cover guard dispatch, redundant guarded branches, and non-exhaustive finite lists. |
| Native diagnostics at generated item and expression granularity | `RustSourceMap`, CLI Cargo JSON remapping, and `jisp native-check` tests. |
| Resolved export schema with explicit generic and recursive instantiations | `crates/jisp/tests/export_schema.rs` and `jisp export-schema --type`. |
| Formatter, stateful REPL, package initialization/entry execution, local dependency lockfiles, offline registry cache entries, and basic LSP | CLI tests and README contract. LSP includes diagnostics, completion, hover, and top-level local/imported definition lookup. |
| Native immutable value semantics | `d3d7e88` emits clones for local Jisp values, with interpreter/native differential tests for collection updates and reusable non-mutating values. |
| Unsupported heterogeneous dynamic reads are rejected consistently | Type inference rejects dynamic reads on heterogeneous closed rows unless the key is static, and native emission tests assert the unsupported path fails without a `Value` fallback. |
| Unsupported remote registry fetches are rejected explicitly | `jisp lock` rejects `http://` and `https://` registry URLs with an unsupported-remote-registry error; local file registries and offline lock/cache entries remain the supported P2 contract. |

## Deferred beyond P2

| Deferred area | Reason |
| --- | --- |
| Native open-row function monomorphisation | Open-row functions need concrete call-site specialization keys, stable generated names, import-aware instantiation, and source-map provenance. This is orthogonal to the current closed-object and explicit-map ABI. |
| Heterogeneous dynamic selection / JSON value | A dynamic lookup on `{count: int, enabled: bool}` has no single implicit result type. If needed, it must be a source-visible finite sum or JSON-like value with parser, type, evaluator, exhaustiveness, and codegen support. |
| General compile-time macro evaluator | The shipped macro system is intentionally bounded to quote/quasiquote templates. A general evaluator would need capability, sandboxing, dependency, determinism, and IO rules before implementation. |
| Full pattern-matrix exhaustiveness | The current checker deliberately proves finite enum/bool/null, finite list, and finite object-product domains. A uniform matrix algorithm for arbitrary structural domains is a future compatibility target. |
| Remote registry lookup and downloads | The P2 package contract is local/offline. Network fetches require an end-to-end trust, checksum, cache, and lockfile policy. |

## Completion gate

P2 is complete when the public contract, implementation, tests, and docs agree
and the full local validation run is clean. This audit records that boundary:
the concrete native ABI remains `TypedModule` plus generated Rust types, and
unsupported dynamic shapes fail at type/codegen boundaries instead of using
`jisp_eval::Value`, `serde_json::Value`, or `Box<dyn Any>`.

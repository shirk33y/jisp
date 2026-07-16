# Stability and compatibility

Jisp is pre-1.0. A change is compatible only when its documented contract,
conformance tests, and diagnostics remain compatible together.

| Surface | Label | Contract |
| --- | --- | --- |
| Core source semantics and typed IR | experimental | migration note and conformance update for a semantic change |
| Lisp/JSON/YAML/ws normalization | experimental | equivalent portable fixtures retain AST/IR meaning |
| Interpreter | reference | defines observable Jisp semantics |
| `jisp_macros::{lisp_file!, lisp_expr!}` | supported embedding surface | source diagnostics and import tracking are maintained |
| Native Rust layouts | experimental concrete ABI | only [native inventory rows](NATIVE.md) are supported; no `Value` fallback |
| Generated Rust dependencies | explicit | callers declare `indexmap` and `num-bigint` when generated output needs them |
| CLI, LSP, and package metadata | experimental | may evolve separately from source semantics |

## Versioning

- Before 1.0, breaking source, macro, package, or generated-code changes require
  a migration note in the release notes and an updated compatibility test.
- A deprecated supported macro/source form remains accepted for at least one
  published minor release unless a correctness or security issue requires an
  earlier removal.
- New native support is additive only after its inventory row, differential
  test, compile-fail boundary, and `NATIVE.md` entry agree.

## Embedding compatibility

`lisp_file!` and `lisp_expr!` accept path literals relative to the downstream
crate's `CARGO_MANIFEST_DIR`. They track the source and resolved Jisp imports
with `include_str!`, so editing either recompiles the Rust crate. The macro,
the Jisp source, and the generated dependency set are one compatibility unit.

The generated output is not a stable standalone Rust API: generated item names,
types, and dependencies follow the checked Jisp module and the experimental
concrete native ABI. A downstream crate must keep its Jisp source, macro crate,
and explicit generated dependencies compatible when upgrading.

## Lockfiles and release notes

Local/package resolution remains lockfile-and-cache only; no command silently
fetches from a remote registry. Changes to lockfile meaning, macro dependency
tracking, source syntax normalization, native layouts, or diagnostics require a
release note. Release notes name affected source forms, macro paths, dependency
versions, and migration steps.

For supported layouts, compatibility and rejection policy live in
[NATIVE.md](NATIVE.md). This document owns lifecycle policy, not helper
signatures.

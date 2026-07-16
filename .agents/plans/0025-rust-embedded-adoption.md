# Plan: stable Rust-embedded Jisp DSL

**Status:** Proposed. This plan does not authorize a new Jisp language feature,
dynamic ABI, FFI, remote fetching, or changes to native codegen semantics.

## Product thesis

Jisp is a typed Lisp DSL embedded in Rust. The interpreter remains the semantic
reference; `jisp_macros` exposes a deliberately smaller, concrete-layout native
subset for Rust applications.

The native conformance inventory and example suite from plan 0024 are the
baseline. Do not broaden support merely to make an embedding example compile.

## Goal

Make the supported Rust embedding path understandable, stable enough to adopt,
and diagnosable in a downstream crate.

## Deliverables

1. `docs/STABILITY.md` defines the compatibility contract.
2. One documented public Rust embedding path, including generated dependencies.
3. Downstream integration coverage for supported code and mapped failures.
4. Focused diagnostics/LSP improvements on the embedding workflow.

## 1. Define stability before promising it

Write `docs/STABILITY.md` with concise labels:

| Surface | Initial label | Contract |
| --- | --- | --- |
| Core source semantics and typed IR | experimental | changes require migration note and conformance update |
| Lisp/JSON/YAML/ws normalization | experimental | equivalent portable fixtures retain AST/IR meaning |
| Interpreter | reference | defines observable Jisp semantics |
| `jisp_macros::{lisp_file!, lisp_expr!}` | supported embedding surface | source diagnostics and import tracking are maintained |
| Native Rust layouts | experimental concrete ABI | no `Value` fallback; support comes only from inventory rows |
| Generated Rust dependency set | explicit | document `indexmap` and `num-bigint` requirements; do not hide them |
| CLI/LSP/package metadata | experimental | versioned independently from source semantics |

Define: versioning rule, deprecation window, macro/source compatibility,
lockfile compatibility, generated-code compatibility, and what requires a
release note. Link `docs/NATIVE.md`; do not duplicate its matrix.

## 2. Make one public embedding path excellent

Choose and document `jisp_macros::lisp_file!` as the canonical item-emitting
path and `lisp_expr!` for an exported zero-argument `main` expression.

Create `docs/RUST_EMBEDDING.md` with only:

1. minimal downstream `Cargo.toml`;
2. one item macro example;
3. one expression macro example;
4. imports and Cargo rebuild tracking;
5. generated dependency requirements and bigint example;
6. expected Jisp diagnostic when native support rejects source;
7. link to `docs/NATIVE.md` for support, not copied helper signatures.

Add a downstream fixture that builds exactly the documented crate. Keep it
offline and path-based. It must prove both a successful compiled call and a
source-ranged macro failure. If the current macro API needs a user-hostile
workaround, document it first; change public API only with a separate proposal.

## 3. Close the highest-value diagnostic gaps

Audit the embedding flow from source edit to downstream compiler output:

- missing/import-cycle module errors;
- macro expansion origins and imported macro locations;
- type mismatch ranges;
- native layout/rejection diagnostics;
- generated Rust errors that can be mapped back to Jisp.

For each gap, add a focused fixture and regression test before changing output.
Prefer Jisp file/range/excerpt diagnostics. Keep unmappable Rust-only failures
labelled as such; do not invent a fake Jisp span.

Then add or improve LSP coverage for the same source locations: hover/type,
definition through imports, diagnostics, and macro-origin navigation where the
current protocol can express it. Measure fixture-level correctness, not editor
UI screenshots.

## 4. Acceptance

- `docs/STABILITY.md` and `docs/RUST_EMBEDDING.md` own their contracts and link
  to the existing native inventory.
- A copied documented downstream crate builds offline and executes a native
  export.
- Supported and rejected downstream fixtures report Jisp diagnostics with a
  path; mapped cases include the relevant source range/excerpt.
- LSP regression tests cover every diagnostic/navigation improvement made here.
- `cargo fmt --all -- --check`,
  `cargo test --workspace --exclude jisp-macros --quiet`, and
  `cargo test -p jisp-macros --quiet` pass.

## Explicitly later

Only after this plan is accepted, choose one product expansion with its own
design and conformance gate:

1. versioned JSON/YAML data profile for configuration/data work;
2. renderer capability/profile for UI adoption; or
3. `jisp-wire/1` runner for multi-host integration.

Do not start that work as part of this plan. Native open-row monomorphisation,
heterogeneous dynamic selection, FFI, a dynamic `Value` ABI, general
compile-time evaluation, remote registry fetching, and new language syntax are
out of scope.

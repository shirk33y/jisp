# Plan: native conformance and flagship example

**Status:** Proposed execution plan. No language, stdlib, ABI, or source-syntax
extension is in scope.

**Goal:** make the current native subset easy to trust, discover, and evaluate
through one machine-checked support matrix and one realistic Rust embedding.

## Decisions

- The interpreter remains the semantic reference.
- Native codegen supports an explicit subset; rejection is a valid result.
- A supported feature needs a differential test. An unsupported shape needs a
  stable Jisp diagnostic test.
- Documentation is generated or checked against the same support inventory.
- Do not add a helper or syntax merely to make the example prettier.

## Deliverables

1. A native support inventory owned by tests or structured test data.
2. Differential coverage for every inventory item.
3. Compile-fail coverage for every intentional native boundary.
4. `docs/NATIVE.md`, derived from or checked against the inventory.
5. One `examples/` Rust-embedded application with executable tests.

## 1. Define the support inventory

Create one table-like source of truth. Each row has:

```text
id
area                 # value, control flow, helper, import, macro, diagnostic
source fixture
interpreter result   # value or diagnostic class
native status        # supported | intentionally-rejected
native expectation   # value or diagnostic substring/code
notes                # ABI constraint or source-range requirement
```

Start with current native areas:

- scalars, strings, bigint, lists, closed objects, and homogeneous maps;
- functions, typed function values, closures, variadics, imports, and macros;
- enums, `case`, finite list/object patterns, `option`, and `result` helpers;
- supported string/list/math/object/map helpers;
- native rejection boundaries: UI values, open rows, heterogeneous dynamic
  selection, dynamic ABI values, unsupported patterns, and unsupported helpers.

Keep one row per behavioural boundary, not one row per implementation function.

## 2. Test layers

### Differential tests

For every `supported` row:

1. evaluate the fixture through the normal interpreter path;
2. compile the same module through `jisp_macros::lisp_file!` or the stable
   native facade seam;
3. execute the native export; and
4. compare observable values structurally.

Keep fixtures small and named by inventory id. Include imports, closures, and
callbacks as separate rows because their ABI paths differ from simple values.

### Compile-fail tests

For every `intentionally-rejected` row:

1. compile a downstream temporary crate through the proc macro;
2. assert a Jisp diagnostic, not a generated-Rust error;
3. assert the relevant source range or source excerpt when the error is mapped;
4. record the rejection reason in the inventory.

### Cross-syntax guard

For portable fixtures, keep Lisp, JSON, YAML, and `ws` normalization checks.
Add a row only after the canonical fixture and generated forms agree on AST/IR
and interpreter result. Native tests may use the canonical Lisp fixture.

## 3. Documentation contract

Add `docs/NATIVE.md` with five short sections:

1. What native codegen promises.
2. Supported matrix by value, control flow, helper, and module feature.
3. Intentional rejections and their diagnostic meaning.
4. ABI rules: concrete layouts, ownership snapshots, closures, variadics,
   maps, bigint dependencies, and no dynamic `Value` ABI.
5. Parity policy: every supported item has a differential test; new support
   changes the inventory, test, and document together.

The document must link to the inventory/test location. Do not duplicate long
function signatures; `docs/STDLIB.md` owns those.

## 4. Flagship: Rust-embedded task pipeline

Build one medium-sized example under `examples/` plus a Rust integration test.
It should model a small typed task/config pipeline, not a toy arithmetic demo.

Required Jisp behavior:

- imported module and typed exported entry point;
- an enum or `result` error path;
- closed object plus homogeneous map/list transformation;
- callback helper and a capturing closure or variadic function;
- one macro only if it clarifies real repeated structure;
- deterministic output that the Rust test asserts.

Required Rust behavior:

- consume it through the public proc-macro or facade API, not private crates;
- compile and run in a downstream-style test crate;
- demonstrate one source-mapped compile failure in a companion fixture.

Keep UI, network, filesystem, FFI, remote registry, and dynamic JSON out of
this example. They are separate product decisions.

## 5. Delivery order

1. Inventory the existing tests. Mark gaps; do not add features.
2. Add differential and compile-fail rows until every current boundary is
   represented.
3. Add `docs/NATIVE.md`; make its matrix checked or generated from the
   inventory.
4. Build the flagship using only inventory-supported rows.
5. Add the example to CI and runnable documentation where useful.
6. Review gaps exposed by the example. Propose the next feature separately.

## Acceptance criteria

- Every native support claim maps to a named inventory row and passing test.
- Every deliberate rejection maps to a downstream compile-fail fixture with a
  Jisp diagnostic.
- The flagship passes through interpreter and native execution with equal
  observable output.
- `docs/NATIVE.md`, `docs/STDLIB.md`, and test inventory do not contradict.
- No new language feature, stdlib item, or universal dynamic ABI is introduced.

## Follow-up decision

Only after acceptance, choose one evidence-backed direction:

- data ergonomics: accept or reject a versioned JSON/YAML data profile;
- UI adoption: define one renderer capability/profile and conformance gate;
- host integration: design `jisp-wire/1` runner before a native FFI.

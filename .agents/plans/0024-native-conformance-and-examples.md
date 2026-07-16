# Plan: native conformance and examples

**Status:** Proposed execution plan. No language, stdlib, ABI, or source-syntax
extension is in scope.

**Goal:** make the current native subset easy to trust, discover, and evaluate
through one machine-checked support matrix and one realistic Rust embedding.

## Decisions

- The interpreter remains the semantic reference.
- Native codegen supports an explicit subset; rejection is a valid result.
- A supported feature needs a differential test. An unsupported shape needs a
  stable Jisp diagnostic test.
- Portable language fixtures are the semantic corpus. Native support is a
  declared target of an individual fixture, never inferred from its syntax.
- Documentation is generated or checked against the same support inventory.
- Do not add a helper or syntax merely to make the example prettier.

## Deliverables

1. A native support inventory owned by tests or structured test data.
2. Differential coverage for every inventory item.
3. Compile-fail coverage for every intentional native boundary.
4. `docs/NATIVE.md`, derived from or checked against the inventory.
5. At least five executable examples; at least three are nontrivial.

## 1. Define the support inventory

Create one table-like source of truth. Each row has:

```text
id
area                 # value, control flow, helper, import, macro, diagnostic
source fixture
portable test id     # stable fixture/test identity when the behaviour is portable
interpreter result   # value or diagnostic class
backend obligation   # interpreter-only | native-supported | native-rejected
native status        # supported | intentionally-rejected, when native applies
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

### Portable corpus and target selection

The canonical portable corpus currently lives in `tests/language/*.lisp` and
has generated JSON, YAML-like, and `ws` equivalents. File extension is an
implementation detail: its named `test` or `test-error` form is the stable
semantic identity.

Give each portable test a deterministic ID derived from its fixture and test
name. The inventory must reference that ID for a behaviour that can run through
both interpreter and native paths. A row may still reference a native-only
fixture when it proves proc-macro integration, generated Rust ABI, or compiler
diagnostic remapping rather than language semantics.

Do not make every portable test native. UI values, host effects, open rows, and
other intentional native rejections remain portable interpreter tests plus a
separate native diagnostic expectation. The inventory is the only authority for
which backend obligations a test has.

## 2. Test layers

### Differential tests

For every `supported` row:

1. evaluate the fixture through the normal interpreter path;
2. compile the same module through `jisp_macros::lisp_file!` or the stable
   native facade seam;
3. execute the native export; and
4. compare observable values structurally.

Reuse the portable fixture for every semantic row once it has a stable ID.
Keep native-only fixtures small and named by inventory id. Include imports,
closures, and callbacks as separate rows because their ABI paths differ from
simple values.

### Compile-fail tests

For every `intentionally-rejected` row:

1. compile a downstream temporary crate through the proc macro;
2. assert a Jisp diagnostic, not a generated-Rust error;
3. assert the relevant source range or source excerpt when the error is mapped;
4. record the rejection reason in the inventory.

### Cross-syntax guard

For portable fixtures, keep Lisp, JSON, YAML, and `ws` normalization checks.
Add a row only after the canonical fixture and generated forms agree on AST/IR
and interpreter result. The native runner executes the canonical source for a
row marked `supported`; it does not need to compile every syntax spelling.

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

## 4. Example suite

Add at least five examples under `examples/`. Each has source, a short README,
expected output, and an executable test. Examples must use only inventory rows
marked `supported`.

| Example | Scope | Level | Required proof |
| --- | --- | --- | --- |
| `task-pipeline` | imported modules, enum/result, lists, maps, closures | nontrivial | interpreter/native equal output |
| `pricing-rules` | typed rules, callbacks, variadics, error recovery | nontrivial | interpreter/native equal output |
| `rust-embedded-report` | public proc macro or facade in a downstream-style Rust crate | nontrivial | compiled Rust test plus mapped failure fixture |
| `macro-normalizer` | hygienic template macro and normal data transform | compact | expansion and runtime output |
| `collection-toolbox` | immutable list/object/map updates and helper boundary | compact | output plus unsupported-native diagnostic |

The three nontrivial examples must be multi-file or multi-module, have named
domain data types, include an error path, and prove both execution paths. The
compact examples isolate one feature family and make its native boundary easy to
find.

No example adds UI, network, filesystem, FFI, remote registry, or dynamic JSON.
Those are separate product decisions.

## 5. Delivery order

1. Assign stable IDs to existing portable `test` and `test-error` forms.
2. Inventory the existing tests, linking semantic rows to those IDs. Mark
   gaps; do not add features.
3. Add differential and compile-fail rows until every current boundary is
   represented.
4. Add `docs/NATIVE.md`; make its matrix checked or generated from the
   inventory.
5. Build the compact examples, then the three nontrivial examples, using only
   inventory-supported rows.
6. Add every example to CI and runnable documentation where useful.
7. Review gaps exposed by the example. Propose the next feature separately.

## Acceptance criteria

- Every native support claim maps to a named inventory row and passing test.
- Every portable test in scope of a native boundary declares its backend
  obligation through the inventory: supported parity, intentional rejection,
  or interpreter-only.
- Every deliberate rejection maps to a downstream compile-fail fixture with a
  Jisp diagnostic.
- At least five examples exist; three meet the nontrivial definition above.
- Every example passes through interpreter and native execution with equal
  observable output, or is an explicit native-rejection example.
- `docs/NATIVE.md`, `docs/STDLIB.md`, and test inventory do not contradict.
- No new language feature, stdlib item, or universal dynamic ABI is introduced.

## Follow-up decision

Only after acceptance, choose one evidence-backed direction:

- data ergonomics: accept or reject a versioned JSON/YAML data profile;
- UI adoption: define one renderer capability/profile and conformance gate;
- host integration: design `jisp-wire/1` runner before a native FFI.

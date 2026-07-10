# MVP Snapshot Diff Review

Source archive: `jisp-mvp-working-snapshot-2026-07-10.zip`, created from
snapshot HEAD `bbfe090b0c131afad3cb277ff51a7ed981ac1932`.

## Method

- Extracted the archive into a temporary `.agents/artifacts/` directory.
- Indexed the extracted tree with CMM as
  `home-shirk3y-work-jisp-.agents-artifacts-0003-jisp-mvp-working-snapshot-2026-07-10`.
- Compared file inventory, workspace membership, TODO/handoff state, and the
  largest or new implementation files.
- Removed the extracted tree after review. Do not commit the full snapshot.

## Inventory Diff

Files only in the snapshot:

- `crates/jisp-expand/Cargo.toml`
- `crates/jisp-expand/src/lib.rs`
- `crates/jisp-project/Cargo.toml`
- `crates/jisp-project/src/lib.rs`
- `crates/jisp-project/src/schema.rs`
- `crates/jisp-core/src/canonical.rs`
- `docs/WORKLOG_CURRENT.md`
- `SNAPSHOT_GIT_STATUS.txt`
- `SNAPSHOT_UNCOMMITTED.patch`

Files only in current repo include the newer project guidance and tests:

- `AGENTS.md`
- `GLEAM.md`
- `.agents/plans/0001-cargo-native-portable-lisp-tests.md`
- `.agents/plans/0002-ui-language-validation.md`
- `crates/jisp-eval/build.rs`
- `crates/jisp-eval/tests/portable_lisp.rs`
- `crates/jisp-eval/tests/portable_lisp_support.rs`
- `crates/jisp-types/src/prelude.rs`
- `crates/jisp-types/src/top_level.rs`
- `crates/jisp-types/src/infer_test.rs`
- `crates/jisp-types/src/unify_test.rs`
- `crates/jisp/tests/module_resolution.rs`
- `tests/language/list-pipeline.lisp`
- `tests/language/result-case-pipeline.lisp`

Large snapshot-only or snapshot-divergent implementations:

- `crates/jisp-codegen-rust/src/lib.rs`: 2401 lines in snapshot, 17 lines
  currently.
- `crates/jisp-expand/src/lib.rs`: 1012 lines, new crate.
- `crates/jisp-project/src/lib.rs`: 914 lines, new crate.
- `crates/jisp-types/src/infer.rs`: 1644 lines in snapshot, 1078 currently.
- `crates/jisp-types/src/unify.rs`: 300 lines in snapshot, 274 currently.

## What Is Useful

### `jisp-expand`

Useful for P0. It has a concrete expander shape:

- public `expand_module` / `expand_module_with` entry points;
- a `MacroInterface` for imported public macros;
- `ExpansionMap` origin tracking with bounded origin chasing;
- quote/quasiquote/unquote/unquote-splicing handling;
- hygienic rewriting helpers for binders and spans.

Do not copy it directly. It is a 1012-line single file with little direct test
coverage and it predates our current module/test conventions. Best port path:
create a small `jisp-expand` crate, add focused tests beside the module, then
port the public API and quasiquote/origin slices incrementally.

### Static object helper refinements

Snapshot has `Inferencer::infer_object_builtin`, including static typing for:

- `obj.set`: inserts a literal key into the returned object row;
- `obj.del`: removes a literal key from the returned object row;
- `obj.values`: derives the list item type from known object fields;
- `obj.cat`: merges known object rows.

This is directly relevant to the next P0 type-system slice. Adjust before
porting: the snapshot rejects non-literal keys for `obj.set` and `obj.del`,
whereas current repo already has broad prelude schemes for runtime object
helpers. Current behavior should keep dynamic-key fallback instead of making
dynamic keys a type error.

### `jisp-project`

Useful as an architectural reference, not as a replacement. It extracts
directory-as-module loading, import aliasing, imported macro propagation,
cycle detection, project checking, linking, and source-file dependency listing
into a dedicated crate.

Current repo already implements much of module resolution in the `jisp` facade,
with tests for file imports, directory modules, mixed syntax, extensionless
imports, cycles, exported-only visibility, and `check --deps`. A later cleanup
can move that logic into `jisp-project`, but doing so now would be a broader
refactor than the P0 gap requires.

### `jisp-codegen-rust`

Useful for P1 and proc-macro follow-up. It includes:

- typed IR to Rust token generation;
- object shape and enum generation;
- builtin lowering;
- `main` generation checks;
- project-aware generation via `CheckedProject`;
- proc-macro integration support through generated dependency `include_str!`.

Do not adopt wholesale before macro expansion and current type inference settle.
The file is too large for current style limits and depends on the snapshot's
`jisp-project` structure. Treat it as prior art for small native-codegen slices.

### `jisp-core/src/canonical.rs`

Small and low-risk. It serializes normalized AST nodes to canonical JSON and
pretty JSON. This could help syntax-equivalence tests and portable fixture
debugging. It is not a P0 blocker.

## What Is Older Or Incompatible

- Snapshot `TODO.md` still lists type inference, module loading, diagnostics,
  and numeric semantics work that current repo has already partially completed.
- Snapshot handoff lacks current Gleam notes, portable Lisp test fixtures,
  imported dependency listing, object-helper schemes, variadic functions, and
  multiline diagnostic rendering.
- Snapshot has no `AGENTS.md` and uses older inline test placement in several
  modules, while current repo now prefers `<module>_test.rs` beside the module.
- Snapshot codegen and proc macro path assumes `jisp-project` is already the
  central resolver. Current repo's active resolver seam is still in the `jisp`
  facade.
- Snapshot adds substantial code with sparse tests around the newest P0/P1
  pieces. Port with regression tests first.

## Recommendation

Adopt in this order:

1. Port static object helper refinements from snapshot ideas into current
   `jisp-types`, preserving dynamic-key fallback through existing prelude
   schemes.
2. Add `jisp-expand` as the next P0 crate, starting with quote/quasiquote and
   `ExpansionMap` origin tracking tests.
3. Add `canonical.rs` only when it helps a concrete syntax-equivalence or
   portable-test workflow.
4. Defer `jisp-project` extraction until after macro expansion, unless proc
   macro dependency tracking becomes blocked by facade-local resolver code.
5. Defer `jisp-codegen-rust` wholesale adoption to P1. Mine it for targeted
   codegen design and tests only.

Do not merge the snapshot tree directly. It is valuable as prior art and has
some P0 code we should port, but current repo is newer in tests, docs,
diagnostics, module dependency listing, Gleam rationale, and type-system shape.

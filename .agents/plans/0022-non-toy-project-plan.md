# Plan: make Jisp feel like a serious project instead of a toy

## Context

Jisp already has a substantial compiler foundation: multiple source syntaxes feed
one source-aware frontend; the project has type inference, imports, macros, an
interpreter, a bounded native Rust backend, proc-macro integration, package
workflow pieces, a formatter, an LSP, a REPL, lockfiles, and UI experiments.

What keeps the project feeling toy-like is not a lack of compiler work. The gap
is the surrounding product and reliability surface: conformance depth, stability
promises, packaging trust, diagnostics/editor polish, real examples, release
engineering, performance baselines, and one compelling production slice.

This plan intentionally avoids proposing random new syntax. The goal is to make
the existing foundation boringly reliable and clearly useful.

## Product thesis

Before adding much more language surface, choose one primary thesis for the next
milestone:

1. **Typed JSON-native configuration and schema language**
   - Competes with CUE, Dhall, and Jsonnet.
   - Emphasize schemas, validation, imports, lockfiles, reproducibility, and Rust
     embedding.
2. **Rust-embedded statically typed Lisp DSL**
   - Competes with heavy Rust proc-macro DSLs.
   - Emphasize `jisp_macros`, native codegen, diagnostics, and typed generated
     Rust.
3. **Portable deterministic UI/state language**
   - Competes with Elm-style update models and framework-specific state DSLs.
   - Emphasize renderer-neutral UI trees, reducers, deterministic host
     simulation, and compiled UI IR.
4. **Small statically typed Lisp for JSON-shaped programs**
   - The broadest thesis, but hardest to sell.
   - Requires unusually polished language docs, examples, package workflow, and
     tooling.

Recommended next thesis: pick one of the first three. Each gives users a concrete
reason to adopt Jisp before it has a full general-purpose language ecosystem.

## Phase 1: trust the existing surface

### 1. Deepen conformance testing

The current roadmap already points at hardening the P2 surface. Make that the
first non-toy milestone.

Work items:

- Grow interpreter-versus-native differential tests around every supported value
  shape and helper.
- Add systematic compile-fail fixtures for unsupported native shapes.
- Add golden diagnostic snapshots for parser, lowering, type, import, macro, and
  native-codegen errors.
- Add property tests proving equivalent source syntaxes normalize to equivalent
  AST/IR shapes.
- Add regression tests for every fixed bug.
- Keep runnable documentation examples in the normal test suite and expand them
  whenever prose introduces a non-obvious semantic guarantee.
- Version conformance fixtures so future language releases can preserve or
  intentionally migrate behavior.

Why this matters:

- Toy languages usually have examples; serious languages have conformance.
- Jisp has multiple syntaxes and two execution paths, so drift prevention is a
  core product requirement.

### 2. Publish a native backend support matrix

The native backend is deliberately bounded and rejects unsupported programs
instead of falling back to a universal dynamic value. That is a strength, but it
must be legible.

Create `docs/NATIVE.md` with:

- Supported value/type matrix.
- Supported prelude/helper matrix.
- Interpreter-only features.
- Native-only constraints.
- Unsupported constructs and example diagnostics.
- Generated dependency requirements such as `num-bigint` and `indexmap`.
- ABI notes for function values, closures, variadics, maps, objects, result, and
  bigint support.
- Performance expectations and current unknowns.
- Interpreter/native parity policy.

Why this matters:

- Users can tolerate a subset if the boundary is explicit.
- Native rejection diagnostics become part of the language contract instead of a
  surprise.

### 3. Add a stability and compatibility policy

Create `docs/STABILITY.md` covering:

- CLI and crate SemVer policy.
- Language-version policy.
- Prelude compatibility.
- Syntax compatibility.
- Lockfile compatibility.
- Native-codegen compatibility.
- Macro compatibility.
- Deprecation and migration process.
- What is experimental, what is stable, and what is intentionally not planned.

Why this matters:

- A serious project tells users what may break.
- The current TODO distinguishes completed, deferred, and not-planned work, but a
  user-facing stability contract should make those boundaries operational.

## Phase 2: make it usable

### 4. Improve diagnostics and editor experience

Work items:

- Improve LSP ranges, completions, hover, go-to-definition, and live diagnostics.
- Improve source-ranged type errors with expected/actual types.
- Improve import-cycle diagnostics with full module paths.
- Improve macro-origin diagnostic traces.
- Improve native-codegen rejection diagnostics so errors point at Jisp source,
  not generated Rust.
- Snapshot diagnostics to keep message quality stable.

Why this matters:

- Languages feel mature when wrong programs fail clearly.
- Jisp already preserves source files, expansion origins, dependencies, tokens,
  and generated Rust source mappings; that seam should become a visible user
  advantage.

### 5. Document for users, not only implementers

Keep the current spec, architecture, testing, package, roadmap, and TODO docs.
Add or expand user-facing docs:

- `docs/GETTING_STARTED.md`
- `docs/LANGUAGE_TOUR.md`
- `docs/NATIVE.md`
- `docs/MACROS.md`
- `docs/PACKAGES_TUTORIAL.md`
- `docs/ERRORS.md`
- `docs/STABILITY.md`
- `docs/COOKBOOK.md`
- `docs/EMBEDDING_IN_RUST.md`
- `docs/UI.md` if UI remains a central direction

Why this matters:

- Existing documentation is strong as a handoff/spec/architecture package.
- A non-toy project also needs docs for users who do not already understand the
  compiler architecture.

### 6. Build realistic examples

Add examples that prove Jisp can be used outside tiny demos:

- A medium-sized CLI config processor.
- A schema/export pipeline.
- A small package with dependencies and a lockfile.
- A Rust crate using Jisp proc macros in tests and production code.
- A realistic UI app with update/reducer tests.
- A data transformation pipeline.
- A generated Rust module benchmark.
- A comparison showing how a real task differs from JSON, YAML, TypeScript, CUE,
  Dhall, Jsonnet, or Gleam.

Why this matters:

- Serious languages have examples that look like actual programs.
- A flagship example can validate the product thesis.

## Phase 3: make it adoptable

### 7. Harden package and dependency workflow

Current packages are deliberately minimal and registry resolution is offline or
local-file based. That is safe, but production users eventually need a complete
trustworthy package workflow.

Work items:

- Finish the remote registry index and archive-download design.
- Preserve source-first, checksum-driven resolution.
- Add lockfile upgrade commands.
- Add dependency graph inspection.
- Decide whether Jisp needs version solving or a deliberately simpler policy.
- Add package publishing workflow.
- Add reproducible offline build documentation.
- Add security/audit documentation.
- Reject network fallback during ordinary build/check/run/proc-macro expansion
  unless a command explicitly opts into fetching.

Why this matters:

- Packages and lockfiles are adoption infrastructure.
- The current checksum/cache model is a good base; the remote registry layer
  should complete it without weakening reproducibility.

### 8. Add release engineering

Work items:

- Tagged releases.
- Changelog.
- Binary artifacts for the CLI.
- Crate publishing policy.
- MSRV policy.
- Toolchain pinning documentation.
- Release checklist.
- Breaking-change process.
- Nightly/canary examples.
- Reproducible release builds.
- Installation docs.

Why this matters:

- Even experimental languages feel more serious when releases are repeatable.
- Users need to know how to install, upgrade, and report issues against a known
  version.

### 9. Add performance and resource baselines

Work items:

- Parser benchmarks.
- Type-checker benchmarks.
- Interpreter benchmarks.
- Native-codegen benchmarks.
- Macro-expansion stress tests.
- Package resolver benchmarks.
- Memory usage measurements.
- Large-module tests.
- Incremental/editor-latency tests.

Why this matters:

- A serious compiler can answer basic performance questions.
- Jisp's concrete native ABI should be measured and documented, not only
  described.

## Phase 4: expand carefully

### 10. Keep deferred-feature boundaries explicit

Do not add these opportunistically:

- Raw `{}` metadata.
- FFI/native bindings.
- Runtime `eval`.
- Classes/methods/Rust-surface idioms.
- GC or dynamic `any` as a core language direction.
- Native open-row function monomorphisation.
- Source-visible heterogeneous dynamic selection/JSON values.
- General compile-time macro evaluator.
- Full arbitrary structural pattern-matrix checker.
- Remote registry network lookup/downloads without trust/checksum/lockfile
  design.

Work items:

- For each deferred feature, classify it as one of:
  - not planned;
  - planned after design;
  - experimental;
  - interpreter-only;
  - native-subset candidate;
  - blocked on stability or security policy.
- Add a user-facing support matrix so contributors and users do not infer hidden
  promises from implementation details.

Why this matters:

- Serious projects say no clearly.
- Jisp's concrete ABI and source-visible design discipline are advantages only if
  the project keeps respecting them.

### 11. Add security and capability designs before risky features

Before implementing FFI, remote registries, general compile-time macro
evaluation, host effects, or richer UI capabilities, write design documents for:

- Package threat model.
- Registry trust/signature/checksum policy.
- Macro-expansion threat model.
- General compile-time evaluator sandbox and capability model.
- Host capability and UI effect model.
- FFI ABI, ownership, errors, dynamic libraries, headers, and binding generation.
- Reproducible-build expectations.

Why this matters:

- The current bounded macro system and offline registry behavior are conservative
  by design.
- The project should not lose trust by adding powerful host capabilities without
  an explicit security model.

## Milestone checklist

A reasonable definition of "no longer toy-like" for Jisp:

- [ ] One primary product thesis is chosen and documented.
- [ ] `docs/STABILITY.md` exists.
- [ ] `docs/NATIVE.md` exists.
- [ ] A user-facing language tour exists.
- [ ] A Rust embedding guide exists.
- [ ] Native support matrix is documented.
- [ ] Interpreter/native differential coverage exists for every supported native
      value shape and helper.
- [ ] Systematic native compile-fail fixtures exist.
- [ ] Diagnostic snapshots exist for core failure modes.
- [ ] Source-syntax equivalence property tests exist.
- [ ] At least one realistic flagship example exists.
- [ ] Release process and changelog exist.
- [ ] CLI install instructions exist.
- [ ] Benchmark baselines exist.
- [ ] Package trust and remote registry design is written before remote fetching
      is implemented.
- [ ] Security/capability designs exist before FFI, general compile-time eval, or
      richer host effects are implemented.

## Bottom line

Jisp does not need flashy syntax to stop being perceived as a toy. It needs a
sharp product thesis, a stable supported subset, heavy conformance testing,
excellent diagnostics, serious package and release workflow, user-facing docs,
real examples, performance baselines, and written security/trust designs for
powerful deferred features.

The existing architecture is already pointed in the right direction: one shared
Core IR, parser/semantics separation, shared interpreter/native frontend, a
concrete native ABI, and explicit deferred-feature boundaries. The next step is
to turn that foundation into a reliable product surface.

# Plan: make Jisp a reliable project

**Status:** Direction only. No new syntax or runtime feature is authorized by
this plan.

## Goal

Jisp already has compiler seams, UI experiments, native codegen, tooling, and
package basics. The next milestone is reliability and a clear adoption story.

## Pick one product thesis

Choose one before expanding the language:

1. Typed JSON-native configuration and schema language.
2. Rust-embedded, statically typed Lisp DSL.
3. Portable deterministic UI/state language.
4. Broad typed Lisp for JSON-shaped programs.

Prefer 1, 2, or 3. Each gives users a concrete reason to adopt Jisp. Thesis 4
needs the most documentation, examples, and ecosystem work.

## Phase 1: trust the current surface

1. **Conformance.** Cover every supported native value shape and helper with
   interpreter/native differential tests, compile-fail fixtures, diagnostic
   snapshots, source-syntax equivalence tests, and executable documentation.
2. **Native boundary.** Add `docs/NATIVE.md`: supported matrix, interpreter-only
   features, intentional rejections, ABI constraints, generated dependencies,
   and parity policy.
3. **Compatibility.** Add `docs/STABILITY.md`: experimental/stable labels,
   SemVer, language and syntax versions, lockfile/macro/codegen compatibility,
   and deprecation rules.

## Phase 2: make it usable

1. Improve LSP ranges, completion, hover, definitions, type errors, import
   cycles, macro origins, and native-source diagnostics.
2. Add user docs: getting started, language tour, macros, packages, errors,
   cookbook, Rust embedding, and UI guide when UI is a product thesis.
3. Add an executable example suite for the chosen thesis: at least five
   examples, three nontrivial, with one serving as the flagship. Include tests,
   dependencies or embedding where relevant, and real workflows.

## Phase 3: make it adoptable

1. Design remote registry, publishing, version policy, audit flow, and explicit
   fetch. Builds/checks/runs/proc macros must stay lockfile+cache only.
2. Add releases: tags, changelog, CLI artifacts, crate policy, MSRV, install,
   and reproducible release checklist.
3. Add baselines: parser/type/eval/codegen/package performance, memory,
   large-module, macro-stress, and editor-latency tests.

## Guardrails

- Do not add raw `{}`, FFI, host capabilities, general compile-time evaluation,
  remote fetches, or richer UI effects without a written security/design
  contract.
- Do not hide native limitations behind `Value`, `serde_json::Value`, or
  `Box<dyn Any>`.
- Keep one source-aware AST and one Core IR.
- Treat conformance and diagnostics as product work, not cleanup.

## Exit criteria

- One thesis and an executable example suite: at least five examples, three
  nontrivial, and one flagship.
- Native support and stability docs.
- Differential and compile-fail coverage for the native subset.
- Diagnostic snapshots and source-syntax equivalence tests.
- Repeatable release/install path and benchmark baseline.
- Remote registry security/trust design before remote fetching.

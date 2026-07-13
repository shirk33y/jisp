# Jisp documentation

This directory holds durable language, architecture, and research documents.
Repository-root files remain deliberately small and operational:

- [`README.md`](../README.md) is the project entry point.
- [`ROADMAP.md`](../ROADMAP.md) is the product-level direction.
- [`TODO.md`](../TODO.md) is the authoritative implementation queue.
- [`AGENTS.md`](../AGENTS.md) is the contributor workflow.

## Language and implementation

- [Language specification](SPEC.md): source formats, core forms, data, and
  semantic rules.
- [Standard library](STDLIB.md): every public prelude function, its signature,
  behaviour, and example.
- [Declarative UI](UI.md): default component/host-element syntax, lowering
  contract, static HTML behavior, and deferred interactive-runtime work.
- [UI playground](../playground/index.html): a GitHub Pages preview with
  editable examples that runs the real interpreter in WebAssembly; its host is
  intentionally limited to static HTML rendering.
- [Architecture](ARCHITECTURE.md): crate boundaries and non-negotiable design
  invariants.
- [Diagnostics](DIAGNOSTICS.md): source ranges, expansion origins, and
  generated-code diagnostic remapping.
- [Testing](TESTING.md): test layers, native conformance, and executable docs.
- [Packages](PACKAGES.md): package manifest, local dependencies, offline
  registry/cache behavior, and deferred remote registry policy.
- [FFI future](FFI_FUTURE.md): constraints that must be designed before an FFI
  is implemented.

## Research and external references

- [Gleam reference mapping](research/GLEAM.md): the pinned compiler-design
  reference and the Jisp-specific adoption decisions.
- [MAL and multi-host execution](research/MAL.md): analysis of MAL/miniMAL,
  JSON as canonical source, native extensions, process runners, and portable
  host support.

## Document ownership

Keep a rule in the narrowest document that owns it. Do not duplicate a roadmap
item in an architecture document or repeat an API signature outside `STDLIB.md`.
Link to the owner instead. Runnable source examples use the convention described
in [Testing](TESTING.md#documentation-examples).

# Jisp documentation

This directory holds durable language, architecture, and research documents.
Repository-root files remain deliberately small and operational:

- [`README.md`](../README.md) is the project entry point.
- [`ROADMAP.md`](../ROADMAP.md) is the product-level direction.
- [`TODO.md`](../TODO.md) is the authoritative implementation queue.
- [`AGENTS.md`](../AGENTS.md) is the contributor workflow.

The specification and implementation documents describe the current contract.
Research reports and `.agents/plans/` explain proposals or historical design
rationale; they do not enable a language feature by themselves.

## Language and implementation

- [Language specification](SPEC.md): source formats, core forms, data, and
  semantic rules.
- [Standard library](STDLIB.md): every public prelude function, its signature,
  behaviour, and example.
- [Declarative UI](UI.md): default component/host-element syntax, lowering
  contract, static HTML behavior, and the update-driven interactive host.
- [UI commands and ownership](UI_EFFECTS.md): portable command, subscription,
  cancellation, browser-capability, and WIT contract.
- [UI playground](../playground/index.html): a GitHub Pages preview with
  editable examples that runs the real interpreter in WebAssembly; its host is
  intentionally limited to structural JUIR patch application and browser event
  forwarding.
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
- [Interop architecture](research/INTEROP.md): the staged runner, C ABI,
  generated-adapter, WIT, and direct-codegen decision guide.
- [IO, storage, and host capabilities](research/IO_STORAGE.md): proposed
  `memfs` baseline, optional `redb` image backend, capability-boundary rules,
  and cross-host storage/code-generation trade-offs.
- [Memory safety](research/MEMORY_SAFETY.md): current Rust safety boundary and
  the conditions for expanding host-facing features.
- [JSON/YAML data dialects](research/JSON_DATA_DIALECTS.md): research on `$`
  symbols, list/form classification, raw object literals, YAML restrictions,
  and cross-host safety.

## Plans and historical rationale

- [Compiled portable UI runtime](../.agents/plans/0019-compiled-portable-ui-runtime.md):
  completed JUIR runtime milestones and remaining host boundaries.
- [Indentation reader evaluation](../.agents/plans/0023-indentation-reader-syntax-evaluation.md):
  rationale for the implemented `ws` reader.
- [Non-toy project plan](../.agents/plans/0022-non-toy-project-plan.md):
  conformance, support-boundary, release, and adoption work that follows P2.
- [Native conformance and examples](../.agents/plans/0024-native-conformance-and-examples.md):
  concrete execution plan for the current native subset before new features.

## Document ownership

Keep a rule in the narrowest document that owns it. Do not duplicate a roadmap
item in an architecture document or repeat an API signature outside `STDLIB.md`.
Link to the owner instead. Runnable source examples use the convention described
in [Testing](TESTING.md#documentation-examples).

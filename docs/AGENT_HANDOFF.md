# Agent handoff

This is an operational orientation, not a second specification or roadmap.
Read the documents that own the relevant contract instead of copying status
here.

## Read first

1. [README](../README.md) for entry points and current capability summary.
2. [Specification](SPEC.md) for language semantics.
3. [Architecture](ARCHITECTURE.md) for crate boundaries and invariants.
4. [Roadmap](../ROADMAP.md) and [TODO](../TODO.md) for priority and unfinished
   work.
5. [Testing](TESTING.md) before adding a behavior contract or a native feature.

Read [Gleam research](research/GLEAM.md) when adapting compiler design ideas,
and [MAL research](research/MAL.md) when changing JSON source, host execution,
or FFI direction.

## Stable seams

- Parsers implement `jisp_core::SyntaxParser` and normalize syntax only.
- Special forms belong in the registry and lowerer; regenerate schema snapshots
  when their shape changes.
- The `jisp` facade owns parsing, expansion, import resolution, checking,
  evaluation, detailed diagnostics, and native-emission entry points.
- `jisp-types::TypedModule` is the contract consumed by
  `jisp-codegen-rust::generate`.
- `jisp-macros` tracks dependencies and invokes facade/codegen seams; it does
  not own a second parser, resolver, or type checker.
- A standard-library addition needs one reusable runtime operation, evaluator
  wrapper, prelude scheme, documentation entry, and tests.

## Guardrails

- Preserve the one-Core-IR rule across Lisp, `ws`, JSON, and YAML-like source.
- Never introduce a universal evaluator `Value` as the ABI for ordinary native
  program values; emit concrete layouts or reject unsupported shapes.
- Do not implement raw `{}` metadata, FFI, or host globals opportunistically.
  Write the relevant design first.
- Preserve source ranges and macro-origin diagnostics through every frontend
  transformation.
- Keep runnable documentation examples tagged as described in
  [Testing](TESTING.md#documentation-examples).

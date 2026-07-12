# Jisp remaining work

This is the single authoritative list of intentionally unfinished work.

## P0 — complete

- MVP frontend/type contract is complete enough to hand to native-codegen work:
  Core IR expression inference is exhaustive over current `ExprKind`, module
  inference returns top-level schemes, structural object helpers have static-key
  refinements plus dynamic-key fallback, refined `case` exhaustiveness covers
  finite `bool`/`null`/enum domains plus exact-list and nested object-field
  refinements, and `jisp-types` exposes `TypedModule` as the backend input.

## P1 — complete

- `jisp-codegen-rust` emits native Rust for the P1 `TypedModule` subset:
  monomorphic scalar/function definitions, list literals, closed structural
  objects, field access, string templates, simple literal/bind/wildcard `case`,
  concrete native enum constructors, variant `case`, list/object `case`
  patterns, imports, binary intrinsics, typed string/list/math helpers, and
  static closed-row object helpers such as `obj.set`, `obj.del`, `obj.values`,
  and `obj.cat`.
- Generated Rust follows `.agents/plans/0004-p1-runtime-abi-validation.md`: it
  uses concrete typed layouts or fails codegen explicitly, never a universal
  dynamic `Value` for ordinary program values.
- `jisp-macros` uses the facade native file/item path and tracks imported source
  files with `include_str!`.
- Jisp has a P1 UI-language proof-of-shape in `examples/ui_button.lisp`:
  React-like nodes are plain structural data and Tailwind-like utility classes
  are first-class object keys with boolean activation, not `class` or
  `className` strings.

## P2 — language completeness

- P2 milestone queue:
  1. Done: add explicit `bigint` values to the language, interpreter, type
     prelude, docs, and portable tests.
  2. Done: add native backend support for the remaining typed prelude helpers
     such as `str.slice`, `list.get`, `list.slice`, and the first concrete
     `result<T,E>` / `option<T>` helpers that can compile without a dynamic
     `Value` fallback.
  3. Done: add `use` desugaring for callback-last flows, including
     multi-binding callbacks and portable `.lisp` coverage.
  4. Done: build a minimal UI proof prototype with Jisp structural UI data
     rendered to escaped HTML strings through `ui.html`.
  5. Done: improve portable `.lisp` test runner UX with Cargo-visible listing,
     filtering, and fixture/test-aware failure reporting.
  6. Done: emit non-capturing top-level function values as typed native function
     pointers, including `list.map`, `list.filter`, `list.fold`, `list.some`,
     and `list.every` callbacks.
  7. Done: emit known static closed-row `obj.get` fields as concrete
     `result<T, str>` values.
- Add native backend support for nested/capturing function values, variadic user
  functions, `bigint` emission, dynamic object helpers/open rows, and dynamic
  field access.
- Add compile-time evaluation for user macros.
- Add case guards, alternative patterns, aliases, and robust exhaustiveness.
- Expand `jisp-macros` beyond item-position native file emission.
- Broaden the current item-level generated-to-source mapping for `emit-rust`
  output into the granularity needed for diagnostics. The facade already maps
  generated Rust functions, structs, and enums back to Jisp definition/type
  spans.
- Wrap Cargo/rustc JSON diagnostics and remap them to Jisp source ranges.
- Finalise immutable/COW semantics for `list` and `obj` updates.
- Add project-aware JSON Schema generated from resolved modules.
- Add formatter, REPL persistence, LSP, and package tooling.

## Deferred by design

- The meaning of raw `{}` metadata is undecided; parsers reject it.
- FFI/native bindings are deferred. Before coding them, write a design covering
  C ABI, ownership, Result/error representation, `.so/.dll/.dylib`, `.h`, and
  optional binding generators.
- Runtime `eval`, classes, methods, Rust surface idioms, GC, and dynamic `any`
  are not planned for the core language.

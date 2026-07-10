# Jisp remaining work

This is the single authoritative list of intentionally unfinished work.

## P0 — complete

- MVP frontend/type contract is complete enough to hand to native-codegen work:
  Core IR expression inference is exhaustive over current `ExprKind`, module
  inference returns top-level schemes, structural object helpers have static-key
  refinements plus dynamic-key fallback, refined `case` exhaustiveness covers
  finite `bool`/`null`/enum domains plus exact-list and nested object-field
  refinements, and `jisp-types` exposes `TypedModule` as the backend input.

## P1 — native compiler and product validation

- Expand `jisp-codegen-rust` from the current monomorphic scalar/function plus
  list literal, closed structural object, field access, string template, and
  simple literal/bind/wildcard `case`, concrete native enum constructors,
  variant `case`, list/object `case` patterns, binary intrinsic subset, and
  typed string/list/math helper subset to the rest of `jisp_types::TypedModule`.
- Follow `.agents/plans/0004-p1-runtime-abi-validation.md`: generated Rust must
  use concrete typed layouts or fail codegen explicitly, never a universal
  dynamic `Value` for ordinary program values.
- Expand `jisp-macros` beyond item-position native file emission once codegen
  supports more expression/data shapes.
- Validate Jisp as a universal UI description language: React-like components,
  renderer targets, state/event bindings, and Tailwind-like first-class utility
  class sets where class names are data keys or symbols with boolean activation,
  not hidden `class`/`className` strings.

## P2 — language completeness

- Add `use` desugaring.
- Add compile-time evaluation for user macros.
- Add case guards, alternative patterns, aliases, and robust exhaustiveness.
- Add arbitrary-precision `bigint` values, with an explicit constructor form
  such as `[bigint, "32849384983498230592309502398509388908203986232306"]`
  before deciding whether plain integer literals may exceed `i64`.
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

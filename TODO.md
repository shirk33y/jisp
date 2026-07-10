# Jisp remaining work

This is the single authoritative list of intentionally unfinished work.

## P0 — make the MVP production-usable

- Implement full type inference over `jisp-ir`, including structural object
  refinements and nested/refined `case` exhaustiveness. Basic structural
  objects, object-row stdlib schemes for runtime object helpers, variadic
  function types, variadic `str.cat`/`list.cat` schemes, and conservative
  list/object exhaustiveness are implemented. Static-key `obj.get`, `obj.set`,
  `obj.del`, homogeneous `obj.values`, and closed-row `obj.cat` refinements are
  implemented with dynamic-key fallback. Refined exact-list and object-field
  `case` coverage now handles finite domains such as `bool`, `null`, and enum
  tags, including nested object fields and redundant refinement detection.
  Remaining P0 work is a final audit of deeper structural object narrowing and
  any gaps that block typed IR as the native-codegen input.
## P1 — native compiler and product validation

- Implement `jisp-codegen-rust` from typed IR.
- Replace the compile-error scaffold in `jisp-macros` with validation and native
  token emission.
- Add optional `emit-rust` output and generated-to-source mapping.
- Wrap Cargo/rustc JSON diagnostics and remap them to Jisp source ranges.
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

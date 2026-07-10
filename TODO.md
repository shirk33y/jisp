# Jisp remaining work

This is the single authoritative list of intentionally unfinished work.

## P0 — make the MVP production-usable

- Implement full type inference over `jisp-ir`, including structural objects,
  richer exhaustive `case`, and stdlib schemes.
- Implement a hygienic macro expander for quote/quasiquote/unquote/splicing.
- Implement directory-as-module loading, qualified imports, aliases, exports,
  cycle detection, and mixed `.json`/`.yaml`/`.lisp` modules.
- Improve diagnostics rendering and preserve macro-origin chains.
- Decide and specify exact numeric semantics: integer overflow, division, mixed
  integer/float operations, and NaN equality.

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

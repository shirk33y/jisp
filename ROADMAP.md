# Jisp roadmap

Jisp is a compiler foundation for statically checked, JSON-shaped programs.
The interpreter is the reference execution path; native Rust emission is a
deliberately smaller, concrete-type subset. This roadmap describes direction,
not dates. The detailed engineering queue remains [TODO.md](TODO.md).

## Current baseline

- One source-aware frontend accepts Lisp, canonical JSON, and restricted
  YAML-like syntax and lowers them to the same typed Core IR.
- The interpreter supports immutable data, algebraic data types, imports,
  pattern matching, results/options, bigint values, and the complete prelude.
- Native Rust generation supports a growing monomorphic subset: typed functions
  and function values, captured closures, user-defined variadics, lists, closed
  objects, selected `case` patterns, imports, and selected list/result/object
  helpers. It rejects unsupported programs instead of using a universal dynamic
  runtime value.

## Next: make native execution useful for more real programs

1. **Bigints and dynamic objects.** Emit native `bigint` values, then support
   open object rows and dynamic field access without weakening the concrete
   native ABI.
2. **Generated-code diagnostics.** Carry source mapping from emitted Rust
   through Cargo/rustc JSON diagnostics so native errors point back to Jisp
   code.
3. **Conformance depth.** Grow interpreter-versus-native differential and
   compile-fail tests whenever a new value shape or builtin becomes supported.

## Then: complete the language seams

1. **User macros.** Module-local quote/template macros now preserve expansion
   origins. Design hygiene, cross-module visibility, and any general
   compile-time evaluator before extending that deliberately bounded model.
2. **Pattern matching.** Add guards, alternatives, aliases, and stronger
   exhaustiveness/redundancy analysis only with matching parser, type, runtime,
   diagnostics, and native tests.
3. **Value semantics.** Finish the documented copy-on-write/immutable update
   contract for lists and objects.

## Tooling and project workflow

- Generate JSON Schema from resolved module information, not just core syntax.
- Add a formatter, persistent REPL, LSP support, and package/project tooling
  after their shared module and diagnostic contracts are stable.
- Keep runnable documentation examples in the normal test suite and preserve
  equivalence across all three source syntaxes.
- Treat cross-host execution as a protocol and conformance problem before
  adding bindings; see [the MAL research report](docs/research/MAL.md).

## Intentionally deferred

- Raw `{}` metadata has no meaning yet and remains rejected.
- FFI and native bindings require a written ABI, ownership, error, and binding
  generation design first; see [docs/FFI_FUTURE.md](docs/FFI_FUTURE.md).
- Runtime `eval`, classes, methods, a general dynamic `any`, and garbage
  collection are not planned for the core language.

## How priorities are chosen

Prefer work that strengthens a shared seam: Core IR, type inference, runtime
semantics, source diagnostics, or the concrete native ABI. A feature is done
only when its contract, tests, and relevant documentation agree. Consult
[TODO.md](TODO.md) for the ordered, implementation-level backlog.

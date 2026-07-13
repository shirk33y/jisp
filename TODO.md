# Jisp remaining work

This is the single authoritative list of intentionally unfinished work. For
product-level direction and ordering rationale, see [ROADMAP.md](ROADMAP.md).

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
- Jisp has a P1 UI-language proof-of-shape. Declarative source uses explicit
  components, host elements, and directives; lowering retains renderer-neutral
  structural nodes. `examples/ui_button.lisp` remains native-codegen coverage
  for the underlying data shape, while `examples/ui_components.lisp` is the
  default source syntax.

## P2 — complete

- P2 milestone queue:
  1. Done: add explicit `bigint` values to the language, interpreter, type
     prelude, docs, and portable tests.
  2. Done: add native backend support for the remaining typed prelude helpers
     such as `str.slice`, `list.get`, `list.slice`, and the first concrete
     `result<T,E>` / `option<T>` helpers that can compile without a dynamic
     `Value` fallback.
  3. Done: add `use` desugaring for callback-last flows, including
     multi-binding callbacks and portable language coverage.
  4. Done: build a minimal UI proof prototype with Jisp structural UI data
     rendered to escaped HTML strings through `ui.html`.
  5. Done: improve portable language test runner UX with Cargo-visible listing,
     filtering, and fixture/test-aware failure reporting.
  6. Done: emit non-capturing top-level function values as typed native function
     pointers, including `list.map`, `list.filter`, `list.fold`, `list.some`,
     and `list.every` callbacks.
  7. Done: emit known static closed-row `obj.get` fields as concrete
     `result<T, str>` values.
  8. Done: retain resolved expression types for native layout registration and
     emit concrete `result.try`, `result.map`, `result.map-err`, and
     `result.recover` callbacks.
  9. Done: emit typed function expressions, local closures, and closures that
     snapshot captured native values for direct calls and callback helpers.
  10. Done: emit monomorphic native variadic definitions, local/returned
      closures, and typed calls using a final `Vec<T>` rest ABI.
  11. Done: emit concrete `num_bigint::BigInt` values and supported bigint
      arithmetic, comparisons, and helpers in native Rust without a dynamic
      runtime fallback.
  12. Done: emit dynamic `.`/`obj.get`/`obj.has` reads for closed homogeneous
      objects using concrete string-key dispatch, while rejecting heterogenous
      and open rows without a dynamic value fallback.
  13. Done: expand module-local ordered user macros defined as `(~ (fn ...))`
      with quote/quasiquote templates, raw syntax parameters, rest splicing,
      nested expansion, expansion-step protection, and origin diagnostics.
  14. Done: add transparent `(as pattern name)` case aliases through lowering,
      type inference, interpreter execution, native enum emission, and
      exhaustiveness/redundancy analysis.
  15. Done: add `(when pattern guard)` case guards through lowering, typed
      boolean guards, interpreter execution, conservative exhaustiveness, and
      native Rust case emission. Guarded branches do not add exhaustiveness
      coverage, but branches after earlier unguarded full coverage are rejected
      as unreachable.
- Done: choose the native object ABI boundary. Homogeneous runtime-sized
  dictionaries are explicit `map<str, A>` values backed by
  `IndexMap<String, A>`, dynamic `obj.set` on closed homogeneous rows is
  supported without a dynamic runtime representation, and homogeneous closed
  objects can be converted with `obj.to-map` before using runtime-sized helpers
  such as dynamic `map.del`. Dynamic reads on heterogeneous closed rows are
  type errors unless the key is statically known; open object rows and finite
  heterogeneous selection require separate source-visible designs and are
  deferred in
  [`.agents/plans/0016-native-open-object-abi.md`](.agents/plans/0016-native-open-object-abi.md).
- Done: keep the macro system intentionally bounded. Local template bindings
  introduced by macros are hygienic, while unquoted caller syntax keeps its own
  spelling and scope. Macro exports are rejected during expansion in all source
  syntaxes. Path-aware facade loading resolves `macro-import` before lowering
  and imports file-module template macros as `alias.name`; these macro source
  files are included in facade/native/proc-macro dependency lists, including
  transitive macro-imports; transitive macro-import cycles are rejected with
  import-cycle diagnostics; raw unresolved `macro-import` still lowers to a
  dedicated diagnostic. A general compile-time evaluator remains deferred.
- Done: strengthen list/object exhaustiveness within the bounded checker.
  Finite list patterns and products of up to 256 finite object-field
  combinations are checked; native alternatives preserve branch-local bindings
  at top level and inside list/object patterns; enum alternatives with shared
  bindings emit Rust `|` patterns; missing diagnostics name finite list/object
  combinations when known; redundant branches after full finite
  enum/bool/null, list, and object product coverage are rejected; and guarded
  branches after unguarded full coverage are unreachable. A full pattern-matrix
  checker remains a future compatibility target.
- Done: expand `jisp-macros` beyond item-position emission. `lisp_expr!`
  compiles an exported zero-argument `main` as a typed Rust expression.
- Done: generated Rust has stable byte ranges for expressions as well as
  functions, structs, and enums; Cargo JSON primary and secondary spans resolve
  to the narrowest containing Jisp spans, including macro expansion origins.
- Done: finalize immutable/COW semantics for `list`, `obj`, and `map` updates.
  Native `list.prepend`/`list.append`/`list.cat`, `obj.set`/`obj.del`/`obj.cat`,
  and map updates preserve reusable inputs; the native emitter gives every local
  Jisp value an owned snapshot before it participates in a generated expression.
- Done: project-aware export schemas cover recursive named variants, concrete
  generic instantiations through `jisp export-schema --type <type>`, imported
  recursive generic type declarations, and dependency graph output.
- Done: add package and project tooling. `jisp init` creates a minimal manifest
  and `jisp run` reads its entry point; `jisp lsp` provides stdio
  initialization, core-form completion and hover, go-to-definition for
  top-level/imported names and `fn`/`let`/`case` bindings, and live frontend
  diagnostics; local path dependencies from `[dependencies]` resolve during
  imports; `jisp repl --state <file>` persists accepted definitions across
  process restarts; `jisp fmt` provides idempotent Lisp, canonical JSON, and
  flow-style YAML formatting with print/check/write modes; `jisp lock` writes a
  deterministic lockfile for local path dependencies and preserves used
  registry cache entries or populates `.jisp/cache` from a local file registry
  index; and registry dependency specs resolve from offline lock/cache entries
  with version and SHA-256 verification. Remote registry lookup and downloads
  remain deferred.

## Deferred by design

- The meaning of raw `{}` metadata is undecided; parsers reject it.
- FFI/native bindings are deferred. Before coding them, write a design covering
  C ABI, ownership, Result/error representation, `.so/.dll/.dylib`, `.h`, and
  optional binding generators.
- Runtime `eval`, classes, methods, Rust surface idioms, GC, and dynamic `any`
  are not planned for the core language.
- Native open-row function monomorphisation and source-visible heterogeneous
  dynamic selection/JSON values remain future language proposals. The compiled
  ABI must stay concrete and must not silently use `jisp_eval::Value`,
  `serde_json::Value`, or `Box<dyn Any>`.
- A general compile-time macro evaluator is deferred until its capability,
  sandboxing, dependency, and determinism contract is written.
- A full pattern-matrix checker for arbitrary structural domains is a future
  compatibility target beyond the current bounded finite-domain checker.
- Remote registry index lookup and archive downloads are deferred until the
  network, checksum, lockfile, and trust policy is implemented end to end.

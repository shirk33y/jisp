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
      native Rust case emission.
- Add native backend support for dynamic object mutation, heterogeneous dynamic
  reads, and open rows with an explicitly designed concrete ABI. Dynamic
  `obj.set` on closed homogeneous rows is now supported without a dynamic
  runtime representation; dynamic deletion still changes the concrete shape.
  The required type/ABI split is recorded in
  [`.agents/plans/0016-native-open-object-abi.md`](.agents/plans/0016-native-open-object-abi.md).
- Extend the intentionally bounded macro system only after designing hygiene,
  cross-module visibility, and a general compile-time evaluator.
- Add stronger list/object exhaustiveness analysis. Finite list patterns and
  products of up to 256 finite object-field combinations are checked; native
  alternatives preserve branch-local bindings at top level and inside
  list/object patterns; enum alternatives with shared bindings emit Rust `|`
  patterns.
- Expand `jisp-macros` further beyond item-position emission. `lisp_expr!`
  now compiles an exported zero-argument `main` as a typed Rust expression.
- Generated Rust has stable byte ranges for expressions as well as functions,
  structs, and enums; Cargo JSON primary and secondary spans resolve to the
  narrowest containing Jisp spans, including macro expansion origins.
- Finalise immutable/COW semantics for `list` and `obj` updates. Native
  `list.prepend`/`list.append`/`list.cat` and `obj.set`/`obj.del`/`obj.cat`
  now preserve reusable inputs; the native emitter gives every local Jisp value
  an owned snapshot before it participates in a generated expression.
- Extend project-aware export schemas to explicit generic instantiations and
  richer recursive type annotations. Monomorphic JSON-native exports, including
  non-parameterized algebraic types, now resolve imports and expose their
  dependency graph through `jisp export-schema`.
- Add package registry tooling. `jisp init` creates a minimal manifest and `jisp run` reads its
  entry point; `jisp lsp` provides stdio initialization, core-form completion
  and hover, go-to-definition for top-level/imported names and `fn`/`let`/`case` bindings, and live frontend diagnostics;
  local path dependencies from `[dependencies]` resolve during imports; `jisp repl --state <file>` persists accepted
  definitions across process restarts; and `jisp fmt` provides idempotent Lisp,
  canonical JSON, and flow-style YAML formatting with print/check/write modes.

## Deferred by design

- The meaning of raw `{}` metadata is undecided; parsers reject it.
- FFI/native bindings are deferred. Before coding them, write a design covering
  C ABI, ownership, Result/error representation, `.so/.dll/.dylib`, `.h`, and
  optional binding generators.
- Runtime `eval`, classes, methods, Rust surface idioms, GC, and dynamic `any`
  are not planned for the core language.

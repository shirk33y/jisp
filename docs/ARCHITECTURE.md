# Architecture

```text
JSON / restricted YAML / Lisp
              ↓
      source-aware Node AST
              ↓
        macro expansion
              ↓
 module/import resolution
              ↓
        type inference
              ↓
     typed Core IR / TypedModule
          ↙          ↘
 tree evaluator    Rust codegen subset
```

## Crates

- `jisp-core`: source files, spans, AST, diagnostics, special-form registry,
  generated core schema.
- `jisp-syntax-*`: three readers normalising to the same AST.
- `jisp-expand`: quote/quasiquote/unquote/unquote-splicing expansion and
  origin tracking before lowering.
- `jisp-ir`: syntax-independent Core IR and lowering.
- `jisp-types`: type representation, unifier, prelude schemes, import
  environments, expression/module inference, and `TypedModule` output for
  backend consumers.
- `jisp-runtime`: reusable pure implementations of math/string/list/object ops.
- `jisp-eval`: lexical typed-IR-oriented evaluator and tests.
- `jisp-codegen-rust`: native backend over `TypedModule`; it emits the current
  monomorphic scalar/function plus list literal, closed structural object, field
  access, string template, simple literal/bind/wildcard `case`, concrete native
  enum constructors, variant `case`, list/object `case` patterns, simple binary
  intrinsic subset, concrete `num_bigint::BigInt` values and numeric helpers,
  typed function values, local closures that snapshot captured values, concrete
  final-`Vec<T>` variadic functions, calls through function expressions and
  callback list helpers, a typed string/list/math helper subset, and closed-row
  object helpers including dynamic reads on homogeneous fields, plus concrete
  `result` callback helpers. It receives resolved expression types from
  `TypedModule`, so each native `result` and inline closed-object layout is
  registered before Rust emission. It rejects unsupported shapes without
  introducing a dynamic `Value` ABI.
- `jisp-macros`: Cargo dependency-tracking proc macros that call the facade
  native-emission seam.
- `jisp`: facade API, including detailed native emission with source files,
  expansion origins, dependencies, tokens, and item-level generated Rust source
  mapping.
- `jisp-cli`: `check`, `run`, `schema`, and `emit-rust` for the supported
  native subset.

## Invariants

1. Parsers contain no language semantics beyond normalisation.
2. Features are implemented against Core IR, not separately per syntax.
3. Interpreter and codegen share frontend, IR, and runtime helpers.
4. The interpreter's internal `Value` is not the compiled language ABI.
5. Raw `{}` remains unsupported until its purpose is explicitly designed.
6. FFI is not implemented opportunistically; start with a written ABI design.

Checked facade operations retain the normalized Core IR for every resolved
import. `check`, `evaluate`, and `run_main` therefore use one imported module
graph for type inference and interpretation instead of reparsing imports during
evaluation. The native-import path reuses the same resolver cache while it
builds its prefixed typed module.

## Design reference

Gleam is the external reference for ADTs, inference, exhaustiveness,
diagnostics, and module loading. [The Gleam mapping](research/GLEAM.md) tracks
the feature mapping and rationale. [The MAL report](research/MAL.md) covers the
separate portability and host-integration strategy. Both are references for
architecture and behavior, not sources of vendored code or a reason to redesign
Jisp surface syntax.

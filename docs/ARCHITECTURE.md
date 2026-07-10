# Architecture

```text
JSON / restricted YAML / Lisp
              ↓
      source-aware Node AST
              ↓
        macro expansion
              ↓
 module/name resolution (TODO)
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
  monomorphic scalar/function plus simple binary intrinsic subset and rejects
  unsupported shapes without introducing a dynamic `Value` ABI.
- `jisp-macros`: Cargo dependency-tracking proc macros that call the facade
  native-emission seam.
- `jisp`: facade API.
- `jisp-cli`: `check`, `run`, `schema`, and `emit-rust` for the supported
  native subset.

## Invariants

1. Parsers contain no language semantics beyond normalisation.
2. Features are implemented against Core IR, not separately per syntax.
3. Interpreter and codegen share frontend, IR, and runtime helpers.
4. The interpreter's internal `Value` is not the compiled language ABI.
5. Raw `{}` remains unsupported until its purpose is explicitly designed.
6. FFI is not implemented opportunistically; start with a written ABI design.

## Design reference

Gleam is the external reference for ADTs, inference, exhaustiveness,
diagnostics, and module loading. Repository-root `GLEAM.md` tracks the feature
mapping and rationale. It is a reference for architecture and behavior, not a
source of vendored code or a reason to redesign Jisp surface syntax.

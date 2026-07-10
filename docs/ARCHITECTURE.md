# Architecture

```text
JSON / restricted YAML / Lisp
              ↓
      source-aware Node AST
              ↓
     macro expansion (TODO)
              ↓
 module/name resolution (TODO)
              ↓
   type inference (foundation)
              ↓
         typed Core IR
          ↙          ↘
 tree evaluator    Rust codegen (TODO)
```

## Crates

- `jisp-core`: source files, spans, AST, diagnostics, special-form registry,
  generated core schema.
- `jisp-syntax-*`: three readers normalising to the same AST.
- `jisp-ir`: syntax-independent Core IR and lowering.
- `jisp-types`: type representation and unifier; expression inference pending.
- `jisp-runtime`: reusable pure implementations of math/string/list/object ops.
- `jisp-eval`: lexical typed-IR-oriented evaluator and tests.
- `jisp-codegen-rust`: stable native backend seam, currently a scaffold.
- `jisp-macros`: Cargo dependency-tracking macro scaffold.
- `jisp`: facade API.
- `jisp-cli`: `check`, `run`, `schema`, `emit-rust` scaffold.

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

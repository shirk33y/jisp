# Jisp

Jisp is a small statically oriented language frontend that reads Lisp,
canonical JSON, and restricted YAML-like source into the same source-aware AST,
then expands, lowers, checks, interprets, or compiles the currently supported
native subset through a shared Rust implementation.

Rust is the implementation backend, not the surface language. The three input
syntaxes are meant to be semantically equivalent, so tooling can choose the most
useful representation without changing program behavior.

## Pipeline

```text
source file
  -> syntax reader
  -> source-aware AST
  -> macro expansion
  -> module/import resolution
  -> Core IR
  -> type inference / TypedModule
  -> interpreter or Rust codegen
```

Today the interpreter path is still the broadest path. Native Rust code
generation exists for a deliberately small typed subset and fails explicitly for
unsupported layouts or expressions instead of falling back to a dynamic runtime
`Value`. Proc-macro embedding still uses the stable facade seam only for Cargo
dependency tracking.

## Why This Exists

Jisp is exploring a compact language core with JSON-native data shapes,
multiple equivalent source syntaxes, structural objects, algebraic data types,
portable tests written as data, and a native Rust backend.

The practical goal is not to make Rust syntax nicer. The goal is to keep Jisp's
surface language small and portable while using Rust for implementation,
runtime helpers, embedding, and eventual native output.

## Current Status

Implemented or substantially wired:

- source files, spans, shared AST, and diagnostic rendering with macro-origin
  labels;
- readers for Lisp, canonical JSON, and restricted YAML-like syntax;
- quote, quasiquote, unquote, and unquote-splicing expansion before lowering;
- syntax-independent Core IR and lowering;
- lexical evaluator with closures, recursive definitions, enum constructors,
  `case`, lists, objects, string templates, imports, and builtins;
- type inference over Core IR, including module schemes and the `TypedModule`
  contract consumed by the native backend;
- limited native Rust token emission for monomorphic scalar definitions,
  zero-capture top-level functions, and simple binary arithmetic/comparison
  prelude intrinsics;
- generated core JSON Schema;
- CLI commands for checking, running, schema generation, and limited Rust
  emission;
- proc macros that track direct and transitive Jisp source imports for Cargo
  rebuilds and emit native Rust items for the supported subset;
- language, architecture, diagnostics, schema, stdlib, FFI, and handoff docs.

Still incomplete:

- compile-time evaluation for user macros;
- complete package/module loading;
- broader native Rust code generation for data structures, case, imports, and
  runtime helper calls;
- rustc diagnostic remapping through Jisp source maps;
- formatter, LSP, FFI, and binding generation.

See [`TODO.md`](TODO.md) and [`docs/AGENT_HANDOFF.md`](docs/AGENT_HANDOFF.md)
before changing language semantics.

## Workspace Crates

| Crate | Role |
| --- | --- |
| `jisp` | Public facade that connects syntax detection, parsing, expansion, lowering, checking, import discovery, evaluation, and detailed diagnostics. |
| `jisp-core` | Shared foundation for source files, spans, AST nodes, diagnostics, special forms, and generated schema data. |
| `jisp-syntax-lisp` | Lisp reader that parses human-written S-expressions into the shared source-aware AST. |
| `jisp-syntax-json` | Canonical JSON reader that normalises data-shaped modules into the same AST as the other syntaxes. |
| `jisp-syntax-yaml` | Restricted YAML-like flow reader for concise structured examples without accepting full YAML semantics. |
| `jisp-expand` | Macro-preparation layer for quote/quasiquote/unquote expansion and generated-to-origin span tracking. |
| `jisp-ir` | Core IR crate that lowers source AST forms into syntax-independent modules, definitions, expressions, and patterns. |
| `jisp-types` | Type-system crate for type representations, unification, prelude schemes, dependency grouping, import environments, inference, and typed-module output. |
| `jisp-runtime` | Pure runtime helper crate for reusable math, string, list, and object operations shared by evaluator and backends. |
| `jisp-eval` | Tree interpreter for lowered IR with lexical environments, builtins, imports, runtime errors, and portable fixture tests. |
| `jisp-codegen-rust` | Native Rust backend that accepts typed modules, emits a limited concrete Rust subset, and rejects unsupported shapes without a dynamic fallback. |
| `jisp-macros` | Proc-macro crate that tracks Jisp source dependencies through the facade resolver and emits native Rust items for the supported subset. |
| `jisp-cli` | Command-line frontend for checking, running, schema emission, and Rust token emission for the supported native subset. |

## Source Syntaxes

Lisp is the primary human-written syntax:

```lisp
(export main
  (fn ()
    (str "Hello from " ,(str "Jisp") "!")))
```

Canonical JSON is the portable tool syntax:

```json
[
  ["export", "main",
    ["fn", [],
      ["str", "Hello from ", [",", ["str", "Jisp"]], "!"]]]
]
```

Restricted YAML-like flow syntax is accepted for concise structured examples:

```yaml
[
  [export, main,
    [fn, [],
      [str, "Hello from ", [",", [str, "Jisp"]], "!"]]]
]
```

The YAML-like reader is intentionally not full YAML: maps, anchors, aliases,
tags, implicit dates, and YAML 1.1 booleans are rejected.

## Canonical Forms

```text
"name"                       symbol
["str", "name"]              string
["f", "x"]                   call
["list", 1, 2, 3]            list value
["obj", ["str", "x"], 1]     object value
["`", ...]                   quasiquote alias
[",", expression]            unquote alias
[",@", expression]           unquote-splicing alias
```

Top-level executable expressions are rejected; execution starts at exported
`main`.

## CLI

```text
cargo run -p jisp-cli -- check examples/hello.lisp
cargo run -p jisp-cli -- run examples/hello.lisp
cargo run -p jisp-cli -- schema
cargo run -p jisp-cli -- emit-rust examples/native.lisp
```

`emit-rust` prints Rust tokens for the currently supported monomorphic native
subset, including direct top-level calls and simple binary arithmetic/comparison
prelude intrinsics. Unsupported programs report a codegen error rather than
falling back to the interpreter.

## Rust Embedding

The public facade supports parsing, expansion, lowering, checking, interpreter
execution, dependency listing, and limited Rust token emission. The
`jisp-macros` crate records direct and imported source files for Cargo rebuilds,
then emits native Rust items through the same facade for the supported subset.

## Development

Focused smoke checks:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
cargo run -q -p jisp-cli -- check examples/hello.lisp
cargo run -q -p jisp-cli -- run examples/hello.lisp
cargo run -q -p jisp-cli -- emit-rust examples/native.lisp
```

Current CI validation target:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
```

## Reference

Gleam is the main compiler reference for type-system, diagnostic,
exhaustiveness, and module-loading choices. [`GLEAM.md`](GLEAM.md) tracks which
ideas are being ported, why they fit Jisp, and where the local indexed checkout
lives.

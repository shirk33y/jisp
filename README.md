# Jisp

Jisp is a small, statically oriented language frontend that normalises Lisp,
canonical JSON, and restricted YAML-like source into one source-aware AST before
lowering, checking, interpreting, or eventually compiling to Rust.

The intended production pipeline is:

```text
source
  → reader
  → macro expansion
  → module resolution
  → type inference
  → typed Core IR
  → interpreter or Rust codegen
```

Rust is an implementation backend, not part of Jisp's surface language, and the
three source syntaxes are intended to be semantically equivalent.

## Source syntaxes

- Lisp (`.lisp`) is the primary human-written syntax.
- Canonical JSON (`.json`) is a portable structured syntax for tools.
- Restricted YAML-like flow syntax (`.yaml` / `.yml`) is accepted for concise
  structured examples without full YAML semantics.

## Current status

This repository is an active Rust foundation for the compiler, evaluator, and
native backend; it is not a finished production compiler yet.

Implemented or substantially wired:

- source-aware AST and diagnostics with macro-origin labels;
- custom readers for canonical JSON, Lisp, and restricted YAML-like syntax;
- quote/quasiquote/unquote/unquote-splicing expansion before lowering;
- lowering to a shared Core IR;
- lexical evaluator with closures, recursive definitions, enum constructors, `case`,
  lists, objects, string templates, and a minimal standard environment;
- reusable type/unification data structures;
- generated core JSON Schema;
- CLI skeleton;
- file proc-macro scaffolds with Cargo dependency tracking;
- detailed language, architecture, diagnostics, schema, stdlib, and FFI notes;
- unit and integration tests.

Still incomplete:

- full Hindley–Milner inference over Core IR;
- compile-time evaluation for user macros;
- complete module graph and package loader;
- native Rust code generation;
- source-map remapping of rustc diagnostics;
- production-grade formatter/LSP;
- FFI and binding generation.

See [`TODO.md`](TODO.md) and [`docs/AGENT_HANDOFF.md`](docs/AGENT_HANDOFF.md)
before changing language semantics.

Gleam is the main compiler reference for several type-system, diagnostic, and
module-loading choices. See [`GLEAM.md`](GLEAM.md) for the tracked feature
mapping, rationale, local checkout, and CMM project.

## Workspace crates

- `jisp` is the public facade that connects syntax detection, parsing,
  expansion, lowering, type checking, import resolution, evaluation, and
  detailed diagnostic rendering.
- `jisp-core` owns source files, spans, the shared AST, diagnostics, special-form
  registry, and generated core schema.
- `jisp-syntax-lisp` parses the Lisp surface syntax into the shared source-aware
  AST.
- `jisp-syntax-json` normalises canonical JSON modules into the same shared AST
  used by the other syntaxes.
- `jisp-syntax-yaml` accepts the deliberately restricted YAML-like flow syntax
  and normalises it into the shared AST.
- `jisp-expand` performs quote, quasiquote, unquote, and unquote-splicing
  expansion while recording generated-to-origin spans.
- `jisp-ir` defines Core IR and lowers source AST forms into syntax-independent
  module, definition, expression, and pattern structures.
- `jisp-types` contains type representations, unification, prelude schemes,
  top-level dependency grouping, import type environments, and current inference
  logic.
- `jisp-runtime` provides reusable pure runtime operations for math, strings,
  lists, and objects.
- `jisp-eval` interprets lowered IR with lexical environments, imports, builtins,
  runtime errors, and portable language fixture tests.
- `jisp-codegen-rust` is the native Rust backend seam and currently remains a
  scaffold until typed IR emission is implemented.
- `jisp-macros` exposes Rust proc-macro entry points that track Cargo
  dependencies and currently fail clearly until native code generation is ready.
- `jisp-cli` is the command-line frontend for `check`, `run`, `schema`, and the
  future `emit-rust` flow.

## Quick commands

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros
cargo run -p jisp-cli -- check examples/hello.lisp
cargo run -p jisp-cli -- run examples/hello.lisp
```

## Canonical JSON

A JSON file is a list of top-level forms:

```json
[
  ["export", "greet",
    ["fn", ["name"],
      ["str", "Hello, ", [",", "name"], "!"]]],

  ["greet", ["str", "Ada"]]
]
```

Rules:

```text
"name"                       symbol
["str", "name"]              string
["f", "x"]                   call
["list", 1, 2, 3]            list value
["obj", ["str", "x"], 1]     object value
["`", ...]                   quasiquote alias (reserved for macro phase)
[",", expression]            unquote alias
[",@", expression]           unquote-splicing alias
```

## Restricted YAML-like syntax

Quoted scalars are strings; unquoted scalars are symbols:

```yaml
[
  [export, greet,
    [fn, [name],
      [str, "Hello, ", [",", name], "!"]]]
]
```

This is intentionally **not full YAML**. Maps (`{}`), anchors, aliases, tags, implicit
dates, and YAML 1.1 booleans are not accepted.

## Lisp syntax

```lisp
(export greet
  (fn (name)
    (str "Hello, " ,name "!")))
```

## Rust embedding

The public facade already supports parsing, lowering, and interpreter execution.
The file proc macros currently only track source dependencies and deliberately
fail with a clear message until native Rust code generation is implemented.

## CLI shape

```text
jisp check path
jisp run path
jisp schema [output]
jisp emit-rust path      # currently reports that native codegen is unfinished
```

# jisp

`jisp` is a small, statically-oriented Lisp frontend with three source syntaxes:

- Lisp (`.lisp`)
- canonical JSON (`.json`)
- a deliberately restricted YAML-like flow syntax (`.yaml` / `.yml`)

All three readers produce the same source-aware AST. The intended production pipeline is:

```text
source
  → reader
  → macro expansion
  → module resolution
  → type inference
  → typed Core IR
  → interpreter or Rust codegen
```

Rust is an implementation backend, not part of Jisp's surface language.

## Status of this repository

This archive is a **foundation and handoff package**, not a finished compiler.

Implemented or substantially sketched:

- source-aware AST and diagnostics;
- custom readers for canonical JSON, Lisp, and restricted YAML-like syntax;
- lowering to a shared Core IR;
- lexical evaluator with closures, recursive definitions, enum constructors, `case`,
  lists, objects, string templates, and a minimal standard environment;
- reusable type/unification data structures;
- generated core JSON Schema;
- CLI skeleton;
- file proc-macro scaffolds with Cargo dependency tracking;
- detailed language, architecture, diagnostics, schema, stdlib, and FFI notes;
- unit and integration tests.

Not complete:

- full Hindley–Milner inference over Core IR;
- hygienic macro expander;
- complete module graph and package loader;
- native Rust code generation;
- source-map remapping of rustc diagnostics;
- production-grade formatter/LSP;
- FFI and binding generation.

See [`TODO.md`](TODO.md) and [`docs/AGENT_HANDOFF.md`](docs/AGENT_HANDOFF.md) before continuing.

Gleam is the main compiler reference for several type-system, diagnostic, and
module-loading choices. See [`GLEAM.md`](GLEAM.md) for the tracked feature
mapping, rationale, local CMM project, and source attribution policy.

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

No installation or build was performed while preparing this archive.

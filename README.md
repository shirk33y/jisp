# Jisp

Jisp is an experimental, statically oriented Lisp for JSON-shaped programs.
Lisp, canonical JSON, and a restricted YAML-like syntax share one source-aware
frontend, type checker, interpreter, and deliberately bounded Rust code
generator.

Rust is the implementation backend, not the language surface. Native emission
uses concrete typed layouts and rejects unsupported programs instead of falling
back to a universal dynamic value representation.

## Quick start

From the workspace root:

```sh
cargo run -p jisp-cli -- check --types examples/hello.lisp
cargo run -p jisp-cli -- run examples/hello.lisp
cargo run -p jisp-cli -- emit-rust examples/native.lisp
```

The first command validates the frontend and types. The second evaluates the
exported zero-argument `main` through the interpreter. The third prints Rust
tokens for the supported native subset.

## A small program

```lisp test=readme.hello-lisp mode=run
(export main
  (fn ()
    (str "Hello from " ,"Jisp" "!")))
```

The same program can be written as canonical JSON:

```json test=readme.hello-json mode=run
[
  ["export", "main",
    ["fn", [],
      ["str", "Hello from ", "Jisp", "!"]]]
]
```

Or in the restricted YAML-like flow syntax:

```yaml test=readme.hello-yaml mode=run
[
  [export, main,
    [fn, [],
      [str, "Hello from ", "Jisp", "!"]]]
]
```

In JSON, ordinary strings are symbols. Use `["str", "..."]` for a string
literal. In Lisp and YAML-like source, quoted values are strings.

## What works today

- Source-aware parsing, quote/template-macro expansion, lowering, type
  inference, imports, and diagnostics across all three source formats.
- Immutable values: integers, bigints, floats, booleans, null, strings, lists,
  structural objects, closures, and algebraic-data constructors.
- Pattern matching with current exhaustiveness checks for finite enum, boolean,
  null, list, and structural-object cases.
- Interpreter execution of an exported, typed, zero-argument `main`.
- A deliberately narrower native Rust subset: monomorphic definitions, closed
  objects, lists, bigints, typed function values, capturing closures, and
  variadic user functions, supported `case` patterns, imports, and selected
  helpers including `list.map`, `list.filter`, `list.fold`, `list.some`,
  `list.every`, static and dynamic reads on closed homogeneous objects, and
  concrete `result.try`, `result.map`, `result.map-err`, and `result.recover`.
- Proc-macro integration that compiles supported Jisp files into native Rust
  items while tracking imported source dependencies.

The interpreter is the broadest execution path. Native closures snapshot their
captured values, and native emission intentionally does not yet support open
object rows, dynamic object mutation, or dynamic reads on heterogeneous object
rows. A proc-macro consumer whose generated module uses bigints must declare
`num-bigint = "0.4"` directly; generated Rust uses its concrete
`num_bigint::BigInt` type.

## CLI

```text
jisp check [--types] [--deps] <path>
jisp run <path>
jisp schema [output]
jisp export-schema <path> <export> [output]
jisp emit-rust <path>
```

| Command | Purpose |
| --- | --- |
| `check` | Parse, expand, and lower; `--types` also checks types, while `--deps` lists resolved imports. |
| `run` | Type-check and evaluate exported `main` with source-ranged errors. |
| `schema` | Print or write the generated core JSON Schema. |
| `export-schema` | Print or write a JSON Schema for one monomorphic, JSON-native public export. |
| `emit-rust` | Emit Rust tokens for the supported native subset. |

Useful examples live in [examples](examples/): a basic hello program, a native
codegen fixture, static object helpers, and structural UI data.

## Architecture

```text
source
  -> syntax reader
  -> macro expansion
  -> module/import resolution
  -> Core IR
  -> type inference
  -> interpreter or Rust codegen subset
```

Parser crates only normalize syntax. Semantics are shared by the IR, type,
runtime, evaluator, and codegen crates, so every accepted source format follows
the same frontend pipeline.

## Status and roadmap

Jisp is a compiler foundation, not a production language. The current focus is
P2 language completeness: broader native codegen, richer patterns, diagnostics
for generated Rust, formatter and tooling work, and a designed FFI boundary.

The product-level direction is in [ROADMAP.md](ROADMAP.md); the authoritative
implementation queue is [TODO.md](TODO.md). The language contract is in
[docs/SPEC.md](docs/SPEC.md).

## Development

Run the same checks as CI:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude jisp-macros --quiet
cargo test -p jisp-macros --quiet
```

The macro suite is separate because it validates proc-macro expansion and
compilation of generated Rust in downstream fixtures.

## Documentation

- [Documentation index](docs/README.md)
- [Language specification](docs/SPEC.md)
- [Standard library surface](docs/STDLIB.md)
- [Roadmap](ROADMAP.md) and [implementation queue](TODO.md)
- [Architecture and invariants](docs/ARCHITECTURE.md)
- [Research: Gleam mapping](docs/research/GLEAM.md)
- [Research: MAL and multi-host execution](docs/research/MAL.md)

## Reference

Gleam is the main external reference for ADTs, inference, exhaustiveness,
diagnostics, and module loading. Jisp does not vendor Gleam code; the
[Gleam mapping](docs/research/GLEAM.md) records the pinned reference and
rationale. The [MAL research report](docs/research/MAL.md) records the separate
decision to use JSON as canonical source while keeping one semantic reference.

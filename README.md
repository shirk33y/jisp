# Jisp

Jisp is an experimental, statically oriented Lisp for JSON-shaped programs.
Lisp, indentation-based `ws`, canonical JSON, and a restricted YAML-like syntax
share one source-aware frontend, type checker, interpreter, and deliberately
bounded Rust code generator.

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

The same program can be written in `ws`:

```ws
export main
  fn ()
    str "Hello from " ,"Jisp" "!"
```

Or as canonical JSON:

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
literal. In Lisp, `ws`, and YAML-like source, quoted values are strings.

`ws` uses indentation for nested forms and a line-leading `...` marker for flat
argument continuation:

```ws
def profile-summary
  fn (user scores)
    obj
      ... "name" (. user "name")
      ... "slug"
      str.replace
        str.lower (. user "name")
        " "
        "-"
      ... "score"
      list.fold
        fn (total value)
          + total value
        0
        scores
```

This has the same shape as:

```lisp
(def profile-summary
  (fn (user scores)
    (obj
      "name" (. user "name")
      "slug" (str.replace (str.lower (. user "name")) " " "-")
      "score" (list.fold
        (fn (total value) (+ total value))
        0
        scores))))
```

## What works today

- Source-aware parsing, quote/template-macro expansion, lowering, type
  inference, imports, and diagnostics across all supported source formats.
- Immutable values: integers, bigints, floats, booleans, null, strings, lists,
  structural objects, closures, and algebraic-data constructors.
- Pattern matching with current exhaustiveness checks for finite enum, boolean,
  null, list, and structural-object cases.
- Interpreter execution of an exported, typed, zero-argument `main`.
- Declarative, renderer-neutral UI components with explicit host elements,
  attributes, properties, utility classes, event actions, keys, and repeated
  children; `ui.html` is the escaped static-HTML host and the WebAssembly
  playground provides an experimental update-driven browser host.
- A deliberately narrower native Rust subset: monomorphic definitions, closed
  objects, lists, bigints, typed function values, capturing closures, and
  variadic user functions, supported `case` patterns, imports, and selected
  helpers including `list.map`, `list.filter`, `list.fold`, `list.some`,
  `list.every`, homogeneous `map<str, A>` dictionaries, static and dynamic reads
  on closed homogeneous objects, explicit `obj.to-map` conversion, and
  concrete `result.try`, `result.map`, `result.map-err`, and `result.recover`.
- Proc-macro integration that compiles supported Jisp files into native Rust
  items while tracking imported source dependencies.

The interpreter is the broadest execution path. Native closures snapshot their
captured values, and native emission intentionally rejects unsupported object
shapes instead of compiling them through an implicit dynamic value. Use
`map<str, A>` for runtime-sized homogeneous dictionaries. Dynamic reads on
heterogeneous objects require a statically known key, and native open rows or
heterogeneous dynamic selection remain future source-visible language designs.
A proc-macro consumer whose generated module uses bigints must declare
`num-bigint = "0.4"` directly; generated Rust uses its concrete
`num_bigint::BigInt` type. A generated module that uses native maps must also
declare `indexmap = "2"` directly.

For expression-position Rust integration, `jisp_macros::lisp_expr!("path")`
expands a Jisp file with exported zero-argument `main` into a typed Rust
expression while tracking the source file and its Jisp imports for Cargo.

## CLI

```text
jisp check [--types] [--deps] <path>
jisp run [path]
jisp schema [output]
jisp export-schema [--type <type>] <path> <export> [output]
jisp emit-rust <path>
jisp native-check <path>
jisp fmt [--check | --write] <path>
jisp repl
jisp lsp
jisp init [path]
jisp lock [path]
```

| Command | Purpose |
| --- | --- |
| `check` | Parse, expand, and lower; `--types` also checks types, while `--deps` lists resolved imports. |
| `run` | Type-check and evaluate exported `main` with source-ranged errors. Without a path, reads `entry` from local `jisp.toml`. |
| `schema` | Print or write the generated core JSON Schema. |
| `export-schema` | Print or write a JSON Schema for one JSON-native public export. Use `--type "(list int)"` or `--type "(box int)"` to instantiate polymorphic exports and generic tagged variants. |
| `emit-rust` | Emit Rust tokens for the supported native subset. |
| `native-check` | Compile generated Rust in a temporary offline Cargo crate and remap compiler errors to the narrowest generated Jisp expression or item. |
| `fmt` | Format `.lisp`/`.jisp`, canonical `.json`, or flow-style `.yaml`/`.yml`; default prints, `--check` validates, and `--write` updates the file. |
| `repl` | Start a REPL. `def`, `defn`, `component`, `type`, and `import` forms persist for later expressions; `--state <file>` also persists accepted definitions across runs. Use `:help`, `:reset`, or `:quit`. |
| `lsp` | Start a stdio Language Server Protocol endpoint with initialization, core-form completion and hover, go-to-definition for top-level/imported names plus `fn`, `let`, and `case` bindings, and live frontend diagnostics for opened or changed documents. |
| `init` | Create a new package directory with `jisp.toml` and a runnable `main.lisp`; refuses to overwrite either file. |
| `lock` | Resolve the package entry and local path dependencies, then write a deterministic `jisp.lock`. |

`jisp.toml` may declare local package dependencies. An import matching the
dependency name resolves to its path when no sibling module with that name
exists:

```toml
[dependencies]
math = { path = "../math" }
```

`jisp lock` writes the resolved entry source and transitive dependency source
files to `jisp.lock`; it is the current lockfile format for local path
dependencies. Registry-style dependency specs can resolve from existing
`jisp.lock` cache entries when their versions and SHA-256 checksums match, and
`jisp lock` preserves used registry cache entries or populates `.jisp/cache`
from a local file registry index; remote registry lookup and downloads remain
deferred. See
[Packages](docs/PACKAGES.md) for the lock/cache contract.

Useful examples live in [examples](examples/): a basic hello program, a native
codegen fixture, static object helpers, legacy structural UI data for native
codegen coverage, and declarative [UI components](examples/ui_components.lisp).
Try the browser-only [UI playground](https://shirk33y.github.io/jisp/) for a
small, explicitly documented update-driven preview subset.

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

Jisp is a compiler foundation, not a production language. The P2 language
completeness milestone is implemented for the current concrete native ABI. The
current focus is hardening that surface with more conformance tests, sharper
diagnostics, and package/editor workflow polish while keeping larger features
such as FFI, remote registries, open-row native codegen, and heterogeneous
dynamic values behind explicit designs.

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
- [Declarative UI syntax](docs/UI.md)
- [Roadmap](ROADMAP.md) and [implementation queue](TODO.md)
- [Architecture and invariants](docs/ARCHITECTURE.md)
- [Research: Gleam mapping](docs/research/GLEAM.md)
- [Research: MAL and multi-host execution](docs/research/MAL.md)
- [Research: JSON/YAML data dialects](docs/research/JSON_DATA_DIALECTS.md)

## Reference

Gleam is the main external reference for ADTs, inference, exhaustiveness,
diagnostics, and module loading. Jisp does not vendor Gleam code; the
[Gleam mapping](docs/research/GLEAM.md) records the pinned reference and
rationale. The [MAL research report](docs/research/MAL.md) records the separate
decision to use JSON as canonical source while keeping one semantic reference.

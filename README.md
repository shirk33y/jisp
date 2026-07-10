# Jisp

Jisp is an experimental, statically oriented Lisp for JSON-shaped programs. It
reads Lisp, canonical JSON, and a restricted YAML-like syntax into the same
source-aware AST, then runs that program through one Rust implementation for
expansion, lowering, type inference, interpretation, and limited native Rust
code generation.

Rust is the implementation backend, not the user-facing language. The compiled
language is designed around typed layouts and explicit backend failures rather
than a universal dynamic `Value` ABI.

## Status

Jisp is a compiler foundation, not a production language yet.

P0 and P1 are complete enough to support the current frontend, type contract,
interpreter, portable Lisp tests, and a native Rust codegen subset. P2 is now
focused on language completeness: first-class function values, nested and
variadic user functions, native `bigint` emission, dynamic object helpers, user
macro evaluation, richer `case` patterns, diagnostics/source maps, formatter,
LSP, package tooling, and FFI design.

The interpreter is the broadest execution path. Native Rust emission is
deliberately smaller: it accepts typed programs that can be emitted as concrete
Rust and rejects unsupported forms instead of falling back to a dynamic runtime.

## Design Goals

- Keep the surface language small and portable.
- Make Lisp, JSON, and YAML-like inputs semantically equivalent.
- Preserve source ranges through parsing, expansion, lowering, and diagnostics.
- Treat ordinary failures as values, especially through `result`-style flows.
- Use structural data for objects, UI nodes, and JSON-native interchange.
- Compile supported programs to typed native Rust without a catch-all dynamic
  representation.

## Quick Start

Run the existing examples from the workspace root:

```sh
cargo run -p jisp-cli -- check examples/hello.lisp
cargo run -p jisp-cli -- run examples/hello.lisp
cargo run -p jisp-cli -- emit-rust examples/native.lisp
```

The main validation path is:

```sh
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros
```

`jisp-macros` is tested separately when proc-macro behavior is in scope:

```sh
cargo test -p jisp-macros
```

## Language Snapshot

Lisp is the primary human-written syntax. It uses normal S-expressions; square
brackets belong to the JSON and YAML-like syntaxes.

```lisp
(def classify
  (fn (score)
    (case score
      (0 (str "empty"))
      (1 (str "single"))
      (_ (str "many")))))

(export main
  (fn ()
    (str.cat (classify 2) (str ": ") (str.from (+ 40 2)))))
```

The equivalent canonical JSON form is data-shaped, which makes Jisp easy to
generate from tools:

```json
[
  ["def", "answer", ["fn", [], ["+", 40, 2]]],
  ["export", "main", ["fn", [], ["answer"]]]
]
```

The restricted YAML-like reader accepts concise flow-style forms without full
YAML semantics:

```yaml
[
  [def, answer, [fn, [], [+, 40, 2]]],
  [export, main, [fn, [], [answer]]]
]
```

Core forms include `def`, `export`, `import`, `type`, `fn`, `let`, `do`, `if`,
`case`, `use`, `quote`, quasiquote, `macro`, `.`, `and`, `or`, and `not`.
Top-level executable expressions are rejected; execution starts at exported
`main`.

## Runtime Data

Jisp values are immutable in the language model. The current evaluator supports
integers, floats, booleans, null, strings, lists, objects, closures, enum
constructors, and explicit arbitrary-precision integers:

```lisp
(def huge (bigint "32849384983498230592309502398509388908203986232306"))

(export main
  (fn ()
    (+ huge (bigint "4"))))
```

Objects are structural data, not classes or method receivers. Field lookup uses
`.` as an explicit form:

```lisp
(def user
  (obj
    (str "name") (str "Ada")
    (str "active") true))

(export main
  (fn ()
    (if (. user "active")
      (. user "name")
      (str "inactive"))))
```

## UI Proof

Jisp also has a small UI-language proof-of-shape. UI nodes are ordinary
structural objects, and Tailwind-like utility classes are first-class object
keys with boolean activation rather than a space-separated `class` string.

```lisp
(def saving false)

(export main
  (fn ()
    (obj
      (str "tag") (str "button")
      (str "classes")
        (obj
          (str "px-4") true
          (str "py-2") true
          (str "opacity-50") saving
          (str "bg-emerald-600") (not saving))
      (str "children")
        (list
          (obj
            (str "tag") (str "text")
            (str "value") (str "Save"))))))
```

The prototype `ui.html` builtin renders this data shape to escaped HTML. It is
useful for validating the representation, not a full UI framework.

## Pipeline

```text
source file
  -> syntax reader
  -> source-aware AST
  -> quote/quasiquote expansion
  -> module/import resolution
  -> Core IR
  -> type inference / TypedModule
  -> interpreter or Rust codegen subset
```

Parser crates only normalize syntax. Language semantics live in shared IR,
type, runtime, evaluator, and backend crates so every syntax reaches the same
behavioral surface.

## CLI

```text
jisp check [--types] [--deps] <path>
jisp run <path>
jisp schema [output]
jisp emit-rust <path>
```

- `check` parses, expands, lowers, and optionally type-checks a module.
- `run` evaluates exported `main` through the interpreter.
- `schema` prints or writes the generated core JSON Schema.
- `emit-rust` prints native Rust tokens for the supported typed subset.

## Workspace Crates

| Crate | Role |
| --- | --- |
| `jisp` | Public facade for syntax detection, parsing, expansion, lowering, type checking, import resolution, evaluation, dependency discovery, diagnostics, and Rust emission. |
| `jisp-core` | Shared source files, spans, AST nodes, syntax detection, diagnostics, special forms, and generated schema data. |
| `jisp-syntax-lisp` | Lisp reader for human-written S-expressions. |
| `jisp-syntax-json` | Canonical JSON reader for tool-generated source and interchange. |
| `jisp-syntax-yaml` | Restricted flow-style YAML-like reader for concise structured examples. |
| `jisp-expand` | Quote, quasiquote, unquote, unquote-splicing, and macro-origin tracking before lowering. |
| `jisp-ir` | Syntax-independent Core IR and AST-to-IR lowering. |
| `jisp-types` | Type representations, unification, prelude schemes, import environments, inference, top-level dependency grouping, and `TypedModule` output. |
| `jisp-runtime` | Pure reusable math, string, list, and object helpers shared by evaluator and native backends. |
| `jisp-eval` | Tree interpreter with lexical environments, builtins, imports, runtime errors, UI rendering, and portable language fixtures. |
| `jisp-codegen-rust` | Native Rust backend for the supported concrete typed subset. |
| `jisp-macros` | Proc macros that track Jisp source dependencies and emit supported native Rust items through the facade. |
| `jisp-cli` | Command-line frontend for checking, running, schema generation, and Rust token emission. |

## Testing

Portable language tests live under `tests/language/*.lisp` and are exposed to
Cargo as individual libtest cases through `jisp-eval`.

Useful checks:

```sh
cargo test --workspace --exclude jisp-macros
cargo test -p jisp-macros
```

Documentation examples are being moved toward runnable Markdown-backed tests so
the reference docs cannot drift from the implementation. The current plan is in
`.agents/plans/0006-testable-documentation.md`.

## Documentation

- `docs/SPEC.md` defines the current language surface.
- `docs/STDLIB.md` lists the intentionally small standard library surface.
- `docs/ARCHITECTURE.md` describes crate boundaries and compiler invariants.
- `docs/DIAGNOSTICS.md` tracks diagnostic expectations.
- `docs/TESTING.md` describes test strategy and needs a refresh as runnable
  documentation lands.
- `docs/AGENT_HANDOFF.md` captures the current engineering handoff.
- `TODO.md` is the authoritative remaining-work list.
- `GLEAM.md` records the pinned Gleam compiler reference and the features Jisp
  intentionally borrows in spirit.

## Roadmap

Near-term P2 work is ordered around features that increase language usefulness
without compromising the typed native backend:

1. Broaden native Rust emission for first-class functions, nested functions,
   variadic user functions, `bigint`, and dynamic object helpers.
2. Add compile-time evaluation for user macros.
3. Extend `case` with guards, alternatives, aliases, and stronger
   exhaustiveness diagnostics.
4. Finalize immutable list/object update semantics and object-row behavior.
5. Turn documentation examples into runnable tests.
6. Design FFI before implementing native bindings.

## Reference

Gleam is the main external compiler reference for ADTs, inference,
exhaustiveness, diagnostics, module loading, and friendly static-language
design. Jisp does not vendor Gleam code; `GLEAM.md` documents the pinned
checkout, feature mapping, and rationale.

# Jisp

Jisp is an experimental, statically oriented Lisp for JSON-shaped programs. It
reads Lisp, canonical JSON, and a restricted YAML-like syntax into the same
source-aware AST, then runs that program through one Rust implementation for
expansion, lowering, type inference, interpretation, and limited native Rust
code generation.

Rust is the implementation backend, not the user-facing language. The compiled
language is designed around typed layouts and explicit backend failures rather
than a universal dynamic `Value` ABI.

## Examples

### Language Tour

```lisp
; Algebraic data types define constructors.
(type response
  (cached int)
  (miss str))

; Functions are ordinary values.
(def load
  (fn (response)
    (case response
      ((cached value)
        (ok value))
      ((miss key)
        (if (str.has key "user")
          (ok 40)
          (err (str "missing:" ,key)))))))

; Lists, higher-order functions, and explicit integer division compose.
(def visible-score
  (fn (scores)
    (list.fold
      (fn (total value) (+ total value))
      0
      (list.filter
        (fn (value) (> value 1))
        (list.map (fn (value) (+ value 1)) scores)))))

; Objects are structural data. Field access is explicit.
(def public-user
  (fn (user)
    (obj.del
      (obj.set user "slug" (str.replace (str.lower (. user "name")) " " "-"))
      "internal-score")))

; `use` is callback-last sugar for result propagation.
(def finish
  (fn (response)
    (use value (result.try (load response))
      (ok (+ value 2)))))

(export main
  (fn ()
    (do
      ; `do` evaluates forms in order and returns the last one.
      (let (ignored (str.len "warmup")) ignored)
      (let (user (public-user
                   (obj
                     "name" "Ada Lovelace"
                     "active" true
                     "internal-score" 41))
            large (+ (bigint "9223372036854775808") (bigint "4"))
            score (visible-score (list 0 1 2 3)))
        (obj
          "loaded" (finish (cached 40))
          "fallback" (finish (miss "user:42"))
          "score" score
          "large" (str.from large)
          "label" (str "user:" ,(. user "slug") ":" ,(str.from score))
          "active" (or false (and (. user "active") (not false)))
          "name" (case (some (. user "name"))
            ((some name) name)
            ((none) "anonymous"))
          "math" (list (/ -7 3) (// -7 3) (% -7 3) (math.sqrt 81.0))
          "first" (list.get (list "a" "b" "c") 0)
          "slice" (str.slice "abcdef" 1 4))))))
```

### Structural UI Data

```lisp
; UI nodes are plain objects. Utility classes are boolean object keys,
; not one space-separated string.
(def button
  (fn (saving)
    (obj
      "tag" "button"
      "id" "save-button"
      "title" "Save <draft>"
      "classes"
        (obj
          "px-4" true
          "py-2" true
          "opacity-50" saving
          "bg-emerald-600" (not saving))
      "children"
        (list
          (obj
            "tag" "text"
            "value" "Save & close")))))

(export main
  (fn ()
    (ui.html (button false))))
```

### Equivalent Tool Syntax

Lisp is the primary human-written syntax. JSON is the canonical interchange
syntax, so string literals need the explicit `["str", "..."]` form there because
plain JSON strings are symbols.

```json
[
  ["def", "answer", ["fn", [], ["+", 40, 2]]],
  ["export", "main", ["fn", [], ["answer"]]]
]
```

Restricted YAML-like flow syntax is accepted for concise structured examples.
Quoted YAML-like scalars are strings; bare scalars are symbols.

```yaml
[
  [def, answer, [fn, [], [+, 40, 2]]],
  [export, main, [fn, [], [answer]]]
]
```

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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude jisp-macros
```

`jisp-macros` is tested separately when proc-macro behavior is in scope:

```sh
cargo test -p jisp-macros
```

## Language Snapshot

Lisp is the primary human-written syntax. It uses normal S-expressions; square
brackets belong to the JSON and YAML-like syntaxes. In Lisp and YAML-like
source, plain quoted values are normal string literals. The explicit `str` form
is mainly for string templates:

```lisp
(str "hello " ,name " and " ,@(list "Lin" "Grace"))
```

Core forms include `def`, `export`, `import`, `type`, `fn`, `let`, `do`, `if`,
`case`, `use`, `quote`, quasiquote, `macro`, `.`, `and`, `or`, and `not`.
Top-level executable expressions are rejected; execution starts at exported
`main`.

Jisp values are immutable in the language model. The current evaluator supports
integers, floats, booleans, null, strings, lists, objects, closures, enum
constructors, and explicit arbitrary-precision integers.

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

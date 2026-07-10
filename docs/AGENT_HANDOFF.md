# Agent handoff

Start by reading `SPEC.md`, `ARCHITECTURE.md`, and repository-root `TODO.md`.
Do not redesign syntax before completing the existing seams.

## Repository context

This workspace is a Rust rewrite of an earlier TypeScript implementation plus
syntax experiments and related prototypes. Treat the previous code as reference
material only. It is available in git history and on the `old` branch when
recovering intent, APIs, tests, or syntax examples.

Gleam is the primary external compiler reference for ADTs, inference,
exhaustiveness, diagnostics, and module loading. Read repository-root
`GLEAM.md` before porting or adapting behavior from it.
The current local Gleam checkout is indexed as CMM project
`home-shirk3y-stuff-gleam`; `GLEAM.md` records the commit and feature mapping.

## Current P0 focus

Type inference now covers core expressions, let-generalisation, top-level
recursive SCC grouping, enum constructors, `case` pattern typing, minimal
variant exhaustiveness for user-defined ADTs and prelude `result`/`option`,
finite `bool`/`null` literal cases, redundant finite-domain `case` patterns, and
conservative list/object `case` exhaustiveness for irrefutable patterns.
Variadic function types are represented in `jisp-types`; lambda rest parameters
are typed as lists of extra arguments, and runtime-variadic `str.cat` and
`list.cat` have matching prelude schemes. Runtime object helpers including
`obj.get`, `obj.set`, `obj.del`, `obj.values`, and variadic `obj.cat` have broad
object-row prelude schemes, plus static-key refinements for `obj.get`,
`obj.set`, and `obj.del`, homogeneous closed-row `obj.values`, and closed-row
`obj.cat` with dynamic-key fallback. The prelude also has fixed-arity stdlib
functions plus simple runtime helpers such as predicates, `result.recover`,
numeric overloads, `io.println`, and basic object introspection. Remaining
type-system work includes nested/refined exhaustiveness for lists and objects
plus deeper structural object narrowing.

`jisp-types` now exposes `Inferencer::infer_module_with_imports` and
`ImportTypeEnvironments`. It resolves each `import` by path and installs
exported schemes as `alias.name` bindings. The `jisp` facade now resolves
imports from files and directories for both `jisp::check` and runtime
`evaluate`/`run_main`: it supports extensionless
`.lisp`/`.jisp`/`.json`/`.yaml`/`.yml` imports, mixed syntax directory modules,
exported-only visibility, cycle detection, and imported source dependency
listing through `jisp::import_dependencies` and `jisp check --deps`. Mixed
`.lisp`/`.json`/`.yaml` directory modules, exported-only visibility, and
extensionless/directory/transitive dependency lists are covered by regression
tests. The next module-system step is proc-macro/native dependency tracking
around the same resolver seam.

Portable Lisp fixture tests now live under `tests/language/` and are registered
as Cargo-visible tests by `crates/jisp-eval/build.rs`. The generated tests call
`crates/jisp-eval/tests/portable_lisp_support.rs`, which strips the selected
top-level `(test "name" (assert.equal expected actual))` form into synthetic
exports, type-checks the generated module with the prelude, evaluates it
normally, and compares the exported values structurally. This is intentionally a
test fixture format, not core language semantics yet.

Numeric semantics are now specified in `SPEC.md`: integers are checked `i64`,
float arithmetic is `f64`, numeric builtins do not coerce int/float operands,
division by zero is an error, and NaN is not equal to itself.

`jisp-expand` runs after parsing and before lowering through the `jisp` facade.
It expands `quote`, `quasiquote`/`` ` ``, `unquote`/`,`, and
`unquote-splicing`/`,@`, and records an `ExpansionMap` on `ParsedModule`.
Compile-time user macro evaluation remains P1. Diagnostic rendering in
`jisp-core` supports source snippets, notes, cross-file secondary labels, and
multi-line spans. Rendering macro-origin chains through the expansion map is
still pending.

## Useful existing seams

- New syntax: implement `jisp_core::SyntaxParser` only.
- New special form: update the special-form registry and lowerer; regenerate
  schema snapshots.
- New stdlib function: add one reusable runtime operation, evaluator wrapper,
  and type scheme.
- Native compilation: implement only `jisp-codegen-rust::generate`; do not move
  parsing or type logic into proc macros.

## Acceptance criteria for the MVP

- Equivalent programs in all three syntaxes produce equivalent IR and values.
- `type` constructors and `case` are statically checked and exhaustive.
- Errors are source-ranged in original files.
- Imports resolve directory modules independent of file order.
- Proc macro emits native Rust tokens and tracks all imported source files.
- No ordinary program value is represented as a catch-all dynamic enum in
  compiled output.

## Do not implement yet

See `TODO.md`, especially `{}` metadata and FFI.

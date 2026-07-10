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

## Current focus

Type inference now covers core expressions, let-generalisation, top-level
recursive SCC grouping, enum constructors, `case` pattern typing, minimal
variant exhaustiveness for user-defined ADTs and prelude `result`/`option`,
finite `bool`/`null` literal cases, redundant finite-domain `case` patterns, and
conservative list/object `case` exhaustiveness for irrefutable patterns. It also
tracks refined exact-list coverage over finite item domains and refined
object-field coverage over finite fields, including nested object paths such as
`user.active`, duplicate refinement rejection, and conservative enum-tag
coverage for whole variant payloads.
Variadic function types are represented in `jisp-types`; lambda rest parameters
are typed as lists of extra arguments, and runtime-variadic `str.cat` and
`list.cat` have matching prelude schemes. Runtime object helpers including
`obj.get`, `obj.set`, `obj.del`, `obj.values`, and variadic `obj.cat` have broad
object-row prelude schemes, plus static-key refinements for `obj.get`,
`obj.set`, and `obj.del`, homogeneous closed-row `obj.values`, and closed-row
`obj.cat` with dynamic-key fallback. Native codegen emits static closed-row
`obj.len`, `obj.has`, `obj.keys`, `obj.set`, `obj.del`, `obj.values`, and
`obj.cat`; `obj.get` still depends on the P2 generic `result<T,E>` native
layout. The prelude also has fixed-arity stdlib functions plus simple runtime
helpers such as predicates, `result.recover`, numeric overloads including
explicit `(bigint "...")` values, `io.println`, and basic object introspection.
P0 is complete: `jisp-types` exposes
`TypedModule`, and `jisp-codegen-rust::generate` accepts it as the native
backend contract. Native token emission P1 covers monomorphic scalar/function
definitions, list literals, closed structural objects, field access, string
templates, simple literal/bind/wildcard `case`, concrete enum constructors,
variant `case`, list/object `case` patterns, imports, and the current binary
intrinsic plus typed string/list/math/object helper subset without a dynamic
`Value` ABI fallback.

`jisp-types` now exposes `Inferencer::infer_module_with_imports`,
`Inferencer::infer_typed_module_with_imports`, `ImportTypeEnvironments`, and
`TypedModule`. It resolves each `import` by path and installs exported schemes
as `alias.name` bindings. The `jisp` facade now resolves
imports from files and directories for both `jisp::check` and runtime
`evaluate`/`run_main`: it supports extensionless
`.lisp`/`.jisp`/`.json`/`.yaml`/`.yml` imports, mixed syntax directory modules,
exported-only visibility, cycle detection, and imported source dependency
listing through `jisp::import_dependencies` and `jisp check --deps`. Mixed
`.lisp`/`.json`/`.yaml` directory modules, exported-only visibility, and
extensionless/directory/transitive dependency lists are covered by regression
tests. `jisp-macros` consumes the same `jisp::import_dependencies` seam during
macro expansion and emits generated `include_str!` entries for transitive
imports before its current native-codegen scaffold `compile_error!`.

Portable Lisp fixture tests now live under `tests/language/` and are registered
as Cargo-visible tests by `crates/jisp-eval/build.rs`. The generated tests call
`crates/jisp-eval/tests/portable_lisp_support.rs`, which strips the selected
top-level `(test "name" (assert.equal expected actual))` form into synthetic
exports, type-checks the generated module with the prelude, evaluates it
normally, and compares the exported values structurally. This is intentionally a
test fixture format, not core language semantics yet. The current fixture set
covers broad P0/P1 behavior plus bug-boundary regressions for short-circuit
evaluation, skipped Result callbacks, Unicode character slicing, negative list
and string indices, empty rest bindings, and stable object update order.

Numeric semantics are now specified in `SPEC.md`: integers are checked `i64`,
bigints are explicit arbitrary-precision values, float arithmetic is `f64`,
numeric builtins do not coerce numeric types, division by zero is an error, and
NaN is not equal to itself.

`jisp-expand` runs after parsing and before lowering through the `jisp` facade.
It expands `quote`, `quasiquote`/`` ` ``, `unquote`/`,`, and
`unquote-splicing`/`,@`, and records an `ExpansionMap` on `ParsedModule`.
Detailed facade errors retain the `SourceMap` and `ExpansionMap`, and
`ModuleError::render_diagnostics` renders macro-origin chains as secondary
labels. `jisp check` uses the detailed path for parse/check failures.
Compile-time user macro evaluation remains P2. Diagnostic rendering in
`jisp-core` supports source snippets, notes, cross-file secondary labels, and
multi-line spans.

## Useful existing seams

- New syntax: implement `jisp_core::SyntaxParser` only.
- New special form: update the special-form registry and lowerer; regenerate
  schema snapshots.
- New stdlib function: add one reusable runtime operation, evaluator wrapper,
  and type scheme.
- Native compilation: implement `jisp-codegen-rust::generate` from
  `jisp_types::TypedModule`; proc macros may call the facade for dependency
  tracking but must not own parsing or type logic.

## P1 native compiler acceptance criteria

- Equivalent programs in all three syntaxes produce equivalent IR and values.
- `type` constructors and `case` are statically checked and exhaustive.
- Frontend errors are source-ranged in original files. Fine-grained generated
  Rust sourcemaps and rustc/Cargo diagnostic remapping are P2; P1 should favor
  native feature coverage.
- Imports resolve directory modules independent of file order.
- Proc macro emits native Rust tokens through the existing file/item path and
  tracks all imported source files. Broader macro embedding is P2 unless a small
  integration change is required by a P1 backend feature.
- No ordinary program value is represented as a catch-all dynamic enum in
  compiled output.
- Native helper calls either emit concrete Rust over typed values or fail
  codegen; the backend still lacks source-ranged remapping for runtime failures
  inside generated Rust.
- P1 UI-language work is proof-of-shape/spec and feature pressure for native
  Jisp. A real renderer/prototype, formatter, richer portable test UX, native
  bigint emission, project-aware schema, and fine-grained generated diagnostics
  are P2.
- P1 is complete as of the native imports, UI data-shape, and static object
  helper milestones. Remaining native gaps are P2 unless they are regressions
  inside the documented P1 subset.

## Do not implement yet

See `TODO.md`, especially `{}` metadata and FFI.

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

Type inference now covers core expressions, let-generalisation, enum
constructors, `case` pattern typing, minimal variant exhaustiveness for
user-defined ADTs and prelude `result`/`option`, and a conservative prelude for
fixed-arity stdlib functions. Remaining type-system work includes richer
exhaustiveness for literals, lists, objects, redundant patterns, and stdlib
schemes for variadic, overloaded, and object/row-polymorphic builtins.

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

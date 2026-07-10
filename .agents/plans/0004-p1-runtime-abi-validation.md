# P1 runtime ABI validation

## Verdict

P1 can start without making `jisp_eval::Value` part of the compiled ABI, but
the backend must make layout classification an explicit first step. Generated
Rust should either pick a concrete Rust representation for every exported and
internal definition it emits, or fail code generation with a source-ranged
diagnostic. It must not fall back to a universal dynamic enum for ordinary
program values.

## Evidence from current code

- `jisp_eval::Value` is interpreter-only. It stores `Builtin`, `Closure`,
  `Constructor`, and `Uninitialized` variants alongside data values, so it is
  useful for tree evaluation but too broad for compiled output.
- `jisp_types::TypedModule` contains the lowered `Module` plus inferred
  top-level `Scheme`s. That is the correct P1 backend entry point.
- `jisp_types::Type` already distinguishes `null`, `bool`, `int`, `float`,
  `str`, `list`, structural `object`, function, and named ADT types.
- `jisp_runtime` helpers are mostly statically typed Rust functions:
  `list` helpers are generic over `T`, `string` helpers work on strings, and
  `object` helpers operate on `IndexMap<String, T>` rather than on evaluator
  `Value`.
- Current object inference includes static-key refinements plus dynamic-key
  fallback. This is enough for a backend contract, but not enough to blindly map
  every object expression to one universal Rust layout.

## Required compiled representations

Initial P1 codegen should classify every inferred type into one of these
layouts before emitting Rust:

- `null`: `()`, or a zero-sized internal marker if pattern matching needs it.
- `bool`: `bool`.
- `int`: `i64`.
- `float`: `f64`.
- `str`: `String` for owned values; later optimization may use `&str` where
  lifetimes are local and obvious.
- `list<T>`: `Vec<T>` after recursively classifying `T`.
- closed structural object rows: generated Rust structs with named fields.
- homogeneous dynamic objects: `IndexMap<String, T>` only when the backend can
  prove a single concrete value type `T`.
- ADTs: generated Rust enums, with variant payload fields recursively
  classified.
- functions: generated Rust functions or closures with typed parameters and
  result. Capturing closures may need generated environment structs.

Anything outside those layouts is not a reason to emit `Value`; it is a reason
to reject native codegen for that program until the layout is designed.

## Red lines

- Do not depend on `jisp-eval` from `jisp-codegen-rust`.
- Do not generate a catch-all `enum Value` for ordinary values.
- Do not encode ADT values as `{ tag: String, fields: Vec<_> }` in compiled
  output.
- Do not encode closed structural objects as `IndexMap<String, _>` when their
  fields have known names and potentially different types.
- Do not erase all function calls through a dynamic callable interface.

## P1 implementation order

1. Add a private layout classifier in `jisp-codegen-rust` that consumes
   `TypedModule` schemes and returns backend layout descriptors.
2. Support only scalar literals, simple functions, lists, closed objects, and
   non-recursive ADTs at first.
3. Add explicit native-codegen errors for unsupported layouts, especially open
   object rows, polymorphic exported definitions, capturing closures, and any
   dynamic object whose value type cannot be proven homogeneous.
4. Emit Rust only after all referenced definitions have layouts.
5. Add generated-code tests that assert no output mentions `jisp_eval::Value`
   or a backend catch-all `Value`.

## Progress

- Implemented the initial `jisp-codegen-rust` layout classifier. It accepts
  scalar types, lists, functions, named ADTs, and closed structural object rows,
  and rejects `never`, unresolved type variables, polymorphic top-level
  definitions, and open object rows.
- `jisp-codegen-rust::generate` now runs layout classification before returning
  the current native-codegen scaffold error.

## Open design points

- Object row variables currently record openness but not a full native map
  layout. P1 should start by accepting closed rows and rejecting ambiguous open
  rows.
- Polymorphic functions need either monomorphisation or a restricted generic
  Rust emission strategy. Do not erase them dynamically.
- Recursive ADTs and recursive functions should be introduced after the scalar,
  list, closed-object, and simple-ADT path is working.

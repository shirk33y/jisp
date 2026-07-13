# Native ABI for open and dynamically updated objects

## Problem and existing guarantee

Jisp objects are immutable maps with string keys. The interpreter accepts any
runtime key and its internal `Value::Object` is intentionally flexible. Native
Rust must not adopt that representation as its ordinary ABI: P1/P2 native
values remain concrete Rust types.

Today a closed row such as `{primary: int, secondary: int}` emits a Rust
struct. Dynamic `.` and `obj.get` are compiled only when all fields have one
concrete type; a string-key dispatch then returns that type. `obj.set` uses the
same restriction and returns a copied struct. This is sound and should remain
the fast path.

The remaining cases are fundamentally different:

- `obj.del object dynamic-key` can remove any field, so its result has no one
  fixed struct layout;
- an object literal with a computed key has an open row, but `ObjectRow.rest`
  records only an unknown *row*, not the type of values in that row;
- a dynamic lookup on `{count: int, enabled: bool}` has no single Jisp result
  type. Returning an internal Rust enum would still require a corresponding
  Jisp type and operations for consuming it.

The last point is a type-system constraint, not a missing Rust match arm. The
current fallback inference deliberately leaves the lookup result to its usage
context for open/heterogeneous rows; it cannot prove one concrete native return
layout. Silently compiling it through `jisp_eval::Value`, `serde_json::Value`,
or `Box<dyn Any>` would violate the native ABI invariant and make native and
typed execution disagree.

## Decision

Do not introduce a universal dynamic value to make this feature appear
complete. Keep the existing closed, homogeneous dispatch ABI and make new
runtime-shaped object values explicit in both types and syntax before native
emission supports them.

The selected P2 path is the homogeneous dynamic map:

- **Homogeneous dynamic map.** `map<str, A>` is a source-visible type written as
  `(map str A)` in declared type positions and built with `(map "key" value
  ...)`. Its native ABI is `IndexMap<String, A>`, never `Value`. It supports
  dynamic get/set/delete, a runtime-sized key set, and immutable value
  semantics. Conversion between a closed homogeneous object and this type must
  remain explicit, so a static field access never unexpectedly changes layout.

The deferred alternative is a separate language proposal:

- **Finite heterogeneous selection / dynamic JSON value.** If Jisp needs a
  value selected from a fixed heterogeneous record, or a recursive JSON-like
  value, add a source-visible finite sum type with exhaustively matchable
  variants. Codegen may then emit a generated enum for that documented type. It
  must not manufacture an unnameable enum behind ordinary `.` or `obj.get`, and
  must not use `jisp_eval::Value`, `serde_json::Value`, or `Box<dyn Any>` as
  the ordinary native ABI.

Open rows in function signatures are a separate compilation concern. They are
currently polymorphic and `jisp-codegen-rust` rejects all polymorphic
definitions. Supporting them without a generic dynamic record requires call
site monomorphisation (including imported definitions), concrete instantiation
keys, and source-map provenance for every specialization. That work can make
static field access on open-row functions native, but does not solve a
heterogeneous dynamic result by itself.

## Required sequence

1. Done: specify `map<str, A>` as `(map str A)` in declared type positions and
   document `map`/`map.*` in `docs/SPEC.md` and `docs/STDLIB.md`.
2. Done: extend `Type`, unification, expression inference, interpreter
   builtins, JSON schema, and native emission for homogeneous maps. The
   existing row-tail variable remains a row variable, not a map value type.
3. Done: add native differential tests for dynamic get/set/delete, unknown
   keys, map concatenation, values, and input immutability.
4. Done: add explicit `obj.to-map` conversion from homogeneous closed objects
   to `map<str, A>`, so dynamic deletion/update can use the map ABI without
   changing closed object layout.
5. Design and implement monomorphisation before accepting native open-row
   function definitions. Generated specializations must have stable names and
   map diagnostics back to the original generic definition and call site.
6. Treat the heterogeneous finite-selection type as a separate language
   proposal. Its parser, exhaustiveness, schema, evaluator, and codegen must
   land together.

## Non-goals

- Do not change ordinary closed objects into hash maps.
- Do not compile a dynamically selected heterogeneous field as an arbitrary
  Rust trait object or interpreter value.
- Do not claim that dynamic deletion is a closed-row update; it changes the
  observable shape.
- Do not add raw `{}` metadata or use FFI as an escape hatch.

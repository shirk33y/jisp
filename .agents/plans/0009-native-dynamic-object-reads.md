# Native dynamic object reads

## Goal

Extend native codegen with dynamic-key field reads, `obj.get`, and `obj.has`
without introducing a universal evaluator `Value` or claiming that an open,
heterogeneous Jisp object has a concrete Rust layout.

## Current type boundary

`ObjectRow.rest` is a row-tail variable, not a value-type parameter. It records
that keys beyond the statically known fields may exist, but does not prove that
all those values share one Rust type. An `IndexMap<String, T>` would therefore
silently narrow Jisp semantics, while `IndexMap<String, DynamicValue>` would
reintroduce the erased value ABI the native backend intentionally rejects.

## First native slice

Support a dynamic key only when the object expression has a closed row and all
of its fields have the same resolved Jisp type:

- `(. object key)` emits a string-key dispatch and returns the concrete field
  type; a missing key panics, matching direct field lookup's error boundary.
- `obj.get object key` emits the same dispatch and returns a concrete
  `result<T, str>`, with the interpreter-compatible missing-key message.
- `obj.has object key` emits a string-key membership check.

The call-site expected type must agree with every field type for `.` and the
`ok` type of `obj.get`. This makes the generated Rust exhaustive and concrete.

## Explicitly deferred

- Dynamic `obj.set`, `obj.del`, and `obj.cat` results with an open row.
- Dynamic object literals and parameters whose row tail has unknown or
  heterogeneous values.
- A map/dictionary type or a generated dynamic union. Either requires a
  language-level object-row and ABI design, not an emitter fallback.

## Acceptance

- Differential tests cover dynamic `.` access, present/missing `obj.get`, and
  dynamic `obj.has` on a closed homogeneous object.
- Heterogeneous and open rows remain explicit codegen errors, never a `Value`
  fallback.
- Generated code keeps values concrete and passes the full local suite.

## Status

Implemented on `master`: inference refines dynamic reads from a closed
homogeneous row to its shared field type. Native emission dispatches over the
known keys for `.` and `obj.get`, preserves the interpreter's missing-key result
message for `obj.get`, and emits boolean membership for `obj.has`. Differential
tests cover present and missing reads; a codegen regression confirms that a
heterogeneous row is rejected rather than erased.

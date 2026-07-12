# Native dynamic updates for homogeneous closed objects

Dynamic `obj.set` now has a concrete native ABI for a closed object whose
fields all share one type. The type checker preserves the closed row only when
the replacement value unifies with that field type. Rust emission evaluates the
object, key, and value once, mutates a local concrete struct copy selected by
string dispatch, and returns it. An unknown key panics, matching the
interpreter's `obj.set` behavior.

This is immutable at the language boundary: the input value is consumed or
cloned according to normal expression semantics and the resulting struct is a
new Jisp value. It does not introduce `Value`, `IndexMap`, or an open-row ABI.
Dynamic `obj.del`, heterogeneous fields, and open rows remain separate ABI
work because deleting a dynamically selected field changes a concrete struct's
type.

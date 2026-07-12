# Case alias patterns

## Decision

Add `(as pattern name)` as a transparent pattern wrapper. It first matches
`pattern`, then binds `name` to the entire matched value. For example:

```lisp
(case response
  ((as (ok value) whole) (obj "value" value "whole" whole))
  ((err message) (obj "error" message)))
```

`as` does not alter which values a pattern accepts. Exhaustiveness and
redundancy therefore analyse its inner pattern. The new binding participates in
the normal duplicate-binding check.

## Scope

Implement the wrapper through Core IR, lowering, type inference, interpreter,
and native Rust emission. Native enum cases use Rust's `name @ Variant(...)`
form. An alias around a bare binding is valid in the interpreter but remains an
explicit native-codegen rejection because Rust cannot bind the same pattern
value to two independent identifiers without changing the emitted ABI.

Guards and alternatives remain separate increments: both affect reachability
and therefore need a stronger coverage model than this transparent wrapper.

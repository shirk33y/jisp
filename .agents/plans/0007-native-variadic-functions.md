# Native variadic functions

## Goal

Compile monomorphic Jisp functions with a rest parameter to concrete Rust while
preserving ordinary Jisp call syntax. The feature must work for top-level
definitions, local lambdas, captured closures, direct calls, and typed function
values. It must not introduce a dynamic callable or `jisp_eval::Value` into the
native ABI.

## Current boundary

`jisp-types::Type::Function` already records fixed `parameters`, an optional
`rest` item type, and a `result`. The interpreter binds the rest parameter to a
list. Native codegen currently rejects `rest` in top-level definitions, lambda
emission, function values, and callback types.

## Native ABI

For a source type:

```text
(fn (A B ...R) Z)
```

emit the Rust callable ABI:

```rust
fn(A, B, Vec<R>) -> Z
```

For function values and closures, use the existing owned callable representation:

```rust
Rc<dyn Fn(A, B, Vec<R>) -> Z>
```

At every Jisp call site, emit the fixed arguments normally and pack all remaining
arguments into `vec![...]`. A Jisp rest binding therefore remains a `list<R>`
semantically and a `Vec<R>` natively.

## In scope

- Top-level monomorphic variadic definitions and direct calls.
- Local variadic lambdas, including closures that capture values.
- Calling a variadic function through a typed local, returned closure, or typed
  function expression.
- Empty and non-empty rest arguments.
- Existing variadic intrinsics such as `str.cat` and `list.cat` retain their
  intrinsic emission; they do not need to be represented as first-class native
  variadic values in this milestone.
- Interpreter-vs-native conformance coverage and an updated downstream
  compile-fail fixture for the next remaining unsupported native boundary.

## Out of scope

- Polymorphic native definitions and monomorphisation.
- A dynamic `apply` ABI or erased callable interface.
- Dynamic/open object rows, bigint emission, and dynamic field access.
- Changing source syntax or evaluator semantics for rest parameters.

## Implementation steps

1. Teach native type emission and top-level definition emission to lower a rest
   item type as a final `Vec<R>` parameter.
2. Teach local lambda emission to bind its rest name as `Vec<R>` while retaining
   closure capture snapshot semantics.
3. Centralise typed call argument emission: validate the fixed arity, emit fixed
   arguments with their concrete types, and pack the remaining arguments into a
   typed vector.
4. Reuse that call lowering for top-level names, local `Rc<dyn Fn>` values, and
   arbitrary typed function expressions. Keep fixed-arity callback helpers
   strict unless their own contracts accept a rest argument.
5. Add native conformance fixtures for top-level, local/capturing, returned, and
   empty-rest calls. Update README, architecture, testing docs, and TODO.

## Acceptance

- A variadic native function observes `Vec::new()` for no rest arguments and all
  trailing Jisp arguments in order otherwise.
- The same behavior holds through a captured local closure and a returned
  function value.
- Generated Rust has only concrete Rust types and contains no evaluator `Value`
  fallback.
- `cargo fmt --all -- --check`, workspace Clippy with denied warnings, workspace
  tests, and `jisp-macros` tests pass locally.

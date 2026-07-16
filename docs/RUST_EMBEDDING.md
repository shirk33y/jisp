# Rust embedding

Use `jisp_macros::lisp_file!` to emit public Rust items from a checked Jisp
module. Use `lisp_expr!` only when the Jisp file exports zero-argument `main`.
Both paths are relative to the downstream crate's `CARGO_MANIFEST_DIR` and
track resolved Jisp imports for Cargo rebuilds.

The complete, executable downstream fixture is
[`crates/jisp-macros/tests/fixtures/downstream-embedding`](../crates/jisp-macros/tests/fixtures/downstream-embedding).

The executable fixture uses this exact manifest dependency (the relative path
is correct from its checked-in directory):

```toml
[dependencies]
jisp-macros = { path = "../../.." }
indexmap = "2"
num-bigint = "0.4"
```

`indexmap` and `num-bigint` are explicit generated-code dependencies: include
them when the selected Jisp source uses native maps or bigints. Do not rely on
them being re-exported by the proc macro. In another crate, replace the local
`jisp-macros` path with the matching location or published crate version.

## Item macro

```rust
jisp_macros::lisp_file!("src/report.lisp");

fn main() {
    assert_eq!(report().to_string(), "9223372036854775810");
}
```

`report.lisp` can import `values.lisp`; editing either file causes Cargo to
re-expand the macro. The emitted items are concrete Rust layouts, not
`jisp_eval::Value` values.

## Expression macro

```rust
let answer: i64 = jisp_macros::lisp_expr!("src/expression.lisp");
assert_eq!(answer, 42);
```

The expression file must export `main` as a zero-argument function. For a
named/public export, use `lisp_file!` instead.

## Diagnostics

An unsupported or ill-typed source fails during macro expansion with a Jisp
diagnostic. For example, a dynamic key on a heterogeneous closed object reports
the Jisp file and source range; it does not degrade to a generated-Rust type
error. Native layout rejections without a mappable source span still name the
Jisp source file and rejection reason.

Check [NATIVE.md](NATIVE.md) before selecting a native feature. It owns the
machine-checked support matrix and intentional rejections.

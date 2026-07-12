# Testing strategy

Existing tests cover sources, parsers, lowering/evaluation, cross-syntax
normalisation, type instantiation, unification, module resolution, portable
language fixtures, and the native Rust subset. Proc-macro integration fixtures
compile and execute generated scalar, structural, and imported Rust programs.

Add broader snapshot tests for diagnostics and schema. Add property tests
ensuring parse/normalise equivalence across syntax fixtures. Native codegen
still needs systematic compile-fail fixtures and interpreter-vs-native
differential tests.

CI pins the Rust toolchain, checks formatting and Clippy with warnings denied,
runs the workspace suite, and runs `jisp-macros` separately so generated Rust
is compiled by the test harness.

## Native conformance

`crates/jisp-macros/tests/native_differential.rs` compiles one representative
Jisp module through `jisp_macros::lisp_file!` and compares its native exports
with the interpreter. The fixture covers scalars, strings, lists,
closed-object field access, enum `case` expressions, local and returned
capturing closures, callbacks in `list.map`, `list.filter`, `list.fold`,
`list.some`, and `list.every`, plus calls through a conditional typed function
expression and variadic functions with empty/non-empty rest arguments. It also
covers `result` patterns for statically typed `obj.get`,
including an inline closed object, `option` cases, and `result.try`,
`result.map`, `result.map-err`, and `result.recover` callbacks that change the
concrete success or error layout.
Add an export and a matching structural comparison here whenever native codegen
gains a supported value shape or intrinsic.

Unsupported native shapes remain covered by explicit `CodegenError` regression
tests and downstream compile-fail fixtures. The latter build a temporary crate
that invokes the proc macro and assert its Jisp diagnostic, so unsupported
shapes never degrade into opaque generated-Rust failures.

## Documentation examples

Runnable examples in `README.md`, `docs/SPEC.md`, and `docs/STDLIB.md` use a
fenced Jisp source block with a stable `test=<name>` attribute and optional
`mode=check` or `mode=run`. The `jisp` crate build script extracts these blocks
and generates one Rust test per example, so they run in the normal workspace
suite and can be filtered by their name.

Use `mode=run` for an exported, zero-argument `main`; use `mode=check` when an
example demonstrates a valid module without an entry point. Untagged blocks
remain ordinary documentation. API signatures and tables in `STDLIB.md` are
checked against the prelude source during review; add a tagged example whenever
prose introduces a non-obvious semantic guarantee.

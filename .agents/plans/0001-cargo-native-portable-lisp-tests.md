# Cargo-native portable Lisp tests

## Goal

Make portable `(test "name" (assert.equal expected actual))` forms in
`tests/language/*.lisp` visible to Cargo as individual tests, so developers can
list and filter them with normal libtest commands:

```text
cargo test -p jisp-eval --test portable_lisp -- --list
cargo test -p jisp-eval map_filter
cargo test -p jisp-eval result_case
```

The fixture format should remain portable across future backends. Cargo-native
registration is only the Rust harness layer, not core Jisp language semantics.

## Current limitation

`crates/jisp-eval/tests/portable_lisp.rs` currently contains one Rust
`#[test]`, which loops over all Lisp fixtures at runtime. This is simple and
works, but libtest only sees the Rust test function name, so Cargo cannot list
or filter individual `.lisp` tests.

Rust's built-in test harness does not support runtime registration of test
functions. Individual Cargo-visible tests must exist as static Rust `#[test]`
items at compile time.

## Proposed design

Add a test-build generation step:

1. Add `crates/jisp-eval/build.rs`.
2. During test builds, scan `tests/language/*.lisp`.
3. Extract top-level `(test "name" ...)` forms.
4. Generate Rust modules/functions into `OUT_DIR/portable_lisp_tests.rs`.
5. `crates/jisp-eval/tests/portable_lisp.rs` includes the generated file with
   `include!(concat!(env!("OUT_DIR"), "/portable_lisp_tests.rs"));`.
6. Move the current runtime fixture logic into a support module, for example
   `crates/jisp-eval/tests/portable_lisp_support.rs`.

Generated test shape:

```rust
mod list_pipeline {
    const FILE: &str = "tests/language/list-pipeline.lisp";
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/language/list-pipeline.lisp"
    ));

    #[test]
    fn map_filter_and_fold_compose() {
        portable_lisp_support::run_lisp_test(FILE, SOURCE, 0);
    }
}
```

The final generated names should be deterministic and filter-friendly:

- file stem becomes module name, for example `list-pipeline.lisp` ->
  `list_pipeline`
- test string becomes function name, for example
  `"map, filter, and fold compose"` -> `map_filter_and_fold_compose`
- duplicate names get a stable suffix, for example `_2`

## Parser choice

Prefer using `jisp_syntax_lisp::LispParser` from `build.rs` if build dependency
wiring stays small. This avoids a second ad hoc Lisp scanner. If that creates
dependency friction, use a deliberately tiny scanner that only recognizes
top-level `(test "name" ...)` forms and fails clearly on malformed fixture test
headers.

The runtime support should remain the source of truth for lowering assertions.
The generator only discovers test names and indexes.

## Build script details

`build.rs` should emit:

```text
cargo:rerun-if-changed=../../tests/language
cargo:rerun-if-changed=../../tests/language/<fixture>.lisp
```

If no fixtures are found, generation should still produce a compilable file
with one failing test or a clear compile error. During active P0 work, a failing
test is friendlier because it appears in normal test output.

## Implementation steps

1. Extract current helper logic from `portable_lisp.rs` into
   `portable_lisp_support.rs`.
2. Add `run_lisp_test(file, source, test_index)` so generated tests can run one
   logical Lisp test at a time.
3. Add `build.rs` discovery and code generation.
4. Replace the current loop test with generated `include!`.
5. Smoke check:

```text
cargo test -p jisp-eval --test portable_lisp -- --list
cargo test -p jisp-eval --test portable_lisp map_filter
cargo test -p jisp-eval --test portable_lisp result_case
```

## Open follow-ups

- Decide whether portable fixture tests should type-check before evaluation.
- Decide whether the same generated test registry should later feed `jisp test`.
- Add backend selection later, for example eval first, then Rust codegen, then
  WASM or other targets.

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

## Current behavior

`crates/jisp-eval/build.rs` scans `tests/language/*.lisp` and generates static
Rust `#[test]` functions into `OUT_DIR/portable_lisp_tests.rs`. Cargo can list
and filter individual portable `.lisp` tests with normal libtest commands:

```text
cargo test -p jisp-eval --test portable_lisp -- --list
cargo test -p jisp-eval --test portable_lisp result_case
cargo test -p jisp-eval --test portable_lisp ui_html
```

The generated harness passes fixture path, source, index, and test name to
`crates/jisp-eval/tests/portable_lisp_support.rs`. Failures include both the
fixture and logical test name for parse, lowering, type checking, evaluation,
assertion-shape, and assertion-value failures.

## Implemented design

The test-build generation step:

1. During test builds, scan `tests/language/*.lisp`.
2. Extract top-level `(test "name" ...)` forms.
3. Generate Rust modules/functions into `OUT_DIR/portable_lisp_tests.rs`.
4. `crates/jisp-eval/tests/portable_lisp.rs` includes the generated file with
   `include!(concat!(env!("OUT_DIR"), "/portable_lisp_tests.rs"));`.
5. Runtime assertion lowering stays in
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
        portable_lisp_support::run_lisp_test(
            FILE,
            SOURCE,
            0,
            "map, filter, and fold compose",
        );
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

`build.rs` uses `jisp_syntax_lisp::LispParser`, avoiding a second ad hoc Lisp
scanner. The runtime support remains the source of truth for lowering
assertions. The generator only discovers test names and indexes.

## Build script details

`build.rs` should emit:

```text
cargo:rerun-if-changed=../../tests/language
cargo:rerun-if-changed=../../tests/language/<fixture>.lisp
```

If no fixtures are found, generation should still produce a compilable file
with one failing test or a clear compile error. During active P0 work, a failing
test is friendlier because it appears in normal test output.

## Smoke check

```text
cargo test -p jisp-eval --test portable_lisp -- --list
cargo test -p jisp-eval --test portable_lisp map_filter
cargo test -p jisp-eval --test portable_lisp result_case
```

## Open follow-ups

- Decide whether the same generated test registry should later feed `jisp test`.
- Add backend selection later, for example eval first, then Rust codegen, then
  WASM or other targets.

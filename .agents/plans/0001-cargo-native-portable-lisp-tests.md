# Cargo-native portable language tests

## Goal

Make portable `test "name"` forms visible to Cargo as individual tests, so
developers can list and filter them with normal libtest commands:

```text
cargo test -p jisp-eval --test portable_language -- --list
cargo test -p jisp-eval map_filter
cargo test -p jisp-eval result_case
```

Canonical fixtures are authored as `.lisp` files under `tests/language/`.
Generated `.json`, `.yaml`, and `.ws` fixtures live under
`tests/generated-language/` and must stay byte-for-byte in sync with the
canonical Lisp source. Cargo-native registration is only the Rust harness layer,
not core Jisp language semantics.

## Current behavior

`crates/jisp-eval/build.rs` scans canonical Lisp fixtures and generated JSON,
YAML-like, and `ws` fixtures, then generates static Rust `#[test]` functions
into `OUT_DIR/portable_language_tests.rs`. Cargo can list and filter individual
portable tests with normal libtest commands:

```text
cargo test -p jisp-eval --test portable_language -- --list
cargo test -p jisp-eval --test portable_language result_case
cargo test -p jisp-eval --test portable_language ui_html
```

The generated harness passes fixture path, source, index, and test name to
`crates/jisp-eval/tests/portable_support.rs`. Failures include both the
fixture and logical test name for parse, lowering, type checking, evaluation,
assertion-shape, and assertion-value failures.

## Implemented design

The test-build generation step:

1. During test builds, scan `tests/language/*.lisp` and
   `tests/generated-language/{json,yaml,ws}/`.
2. Extract top-level `test "name"` forms.
3. Generate Rust modules/functions into `OUT_DIR/portable_language_tests.rs`.
4. `crates/jisp-eval/tests/portable_language.rs` includes the generated file
   with `include!(concat!(env!("OUT_DIR"), "/portable_language_tests.rs"));`.
5. Runtime assertion lowering stays in
   `crates/jisp-eval/tests/portable_support.rs`.

Generated test shape:

```rust
mod list_pipeline {
    const FILE: &str = "tests/generated-language/ws/list-pipeline.ws";
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/generated-language/ws/list-pipeline.ws"
    ));

    #[test]
    fn map_filter_and_fold_compose() {
        portable_support::run_portable_test(
            FILE,
            SOURCE,
            0,
            "map, filter, and fold compose",
        );
    }
}
```

The final generated names should be deterministic and filter-friendly:

- fixture path becomes module name, for example
  `tests/generated-language/ws/list-pipeline.ws` ->
  `tests_generated_language_ws_list_pipeline_ws`
- test string becomes function name, for example
  `"map, filter, and fold compose"` -> `map_filter_and_fold_compose`
- duplicate names get a stable suffix, for example `_2`

## Parser choice

`build.rs` uses `detect_syntax` plus the syntax-specific parser for each
fixture, avoiding a second ad hoc fixture scanner. The runtime support remains
the source of truth for lowering assertions. The generator only discovers test
names and indexes.

## Build script details

`build.rs` should emit:

```text
cargo:rerun-if-changed=../../tests/language
cargo:rerun-if-changed=../../tests/generated-language
cargo:rerun-if-changed=../../tests/language/<fixture>.lisp
cargo:rerun-if-changed=../../tests/generated-language/<syntax>/<fixture>
```

If no fixtures are found, generation should still produce a compilable file
with one failing test or a clear compile error. During active P0 work, a failing
test is friendlier because it appears in normal test output.

## Smoke check

```text
cargo test -p jisp-eval --test portable_language -- --list
cargo test -p jisp-eval --test portable_language map_filter
cargo test -p jisp-eval --test portable_language result_case
```

## Open follow-ups

- Decide whether the same generated test registry should later feed `jisp test`.
- Implement the target-selection contract from
  `0024-native-conformance-and-examples.md`: give each portable test a stable
  fixture/test ID, then let `docs/native-support.json` declare whether that ID
  requires interpreter/native parity, an intentional native rejection, or
  interpreter-only execution. Do not infer target eligibility from the source.
- Add other backends only behind the same explicit target contract, for example
  Wasm or a future independent host runtime.

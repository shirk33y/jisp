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

## Portable language fixtures

`tests/language/*.lisp` generate one interpreter test per `(test "...")` or
`(test-error "...")` form. Positive tests use a normal boolean assertion:

```lisp
(test "helper keeps order"
  (assert (= (list 1 2 3) (sort-values input))))
```

The runner parses, expands quote/user macros, lowers, type-checks, evaluates,
and requires the assertion condition to be exactly `true`. This makes the
fixture language useful beyond equality as more boolean predicates arrive.
`assert.equal` remains temporarily accepted for old fixtures, but new tests
must use `assert` and ordinary Jisp predicates. Negative tests use
`(test-error "name" "expected message substring" expr)`; the runner inserts
`expr` into a temporary export and expects lowering or type-checking to fail
with a diagnostic containing that substring. To test top-level rejections,
wrap temporary module items in the fixture-only pseudo form `(module ...)`,
for example `(test-error "name" "message" (module (macro-import macros
"macros.lisp")))`.

Add a fixture here for language semantics that should be independent of native
Rust codegen, especially parser/lowering edge cases, macro expansion, pattern
matching, stdlib behaviour, immutable value semantics, and portable frontend
rejections such as redundant or non-exhaustive `case` patterns.

## Portable UI scenarios

`tests/ui/*.lisp` are the canonical UI scenarios and generate matching JSON,
YAML, and WS fixtures under `tests/generated-ui/`. A `ui.test` is fixture-only:
the app source remains ordinary Jisp and declares one `ui.app`. Its scenario
steps are deliberately data and renderer neutral:

```lisp
(ui.test "counter updates"
  (assert (= "<button>0</button>" (ui.test.html)))
  (dispatch Increment)
  (assert (= 1 (ui.test.state)))
  (assert (= "<button>1</button>" (ui.test.html))))
```

`dispatch` passes a portable action value to the declared reducer. Assertions
can observe `(ui.test.state)`, the escaped static `(ui.test.html)`, or the raw
renderer-neutral `(ui.test.tree)`. Each assertion also compares the reference
component value with the compiled JUIR execution, so a passing test covers both
the reducer and renderer contract without a browser. Keep pure helper tests in
ordinary `(test ...)` fixtures; UI scenarios should only exercise the
`ui.app` boundary.

The playground recognises the same forms. Its **Run tests** button executes
them in Wasm and shows pass/fail results, while the preview compiles the source
with fixture-only `ui.test` forms removed. A future browser E2E adapter can
replay the same `dispatch` trace without changing application test code.

## Native conformance

`crates/jisp-macros/tests/native_differential.rs` compiles one representative
Jisp module through `jisp_macros::lisp_file!` and compares its native exports
with the interpreter. The fixture covers scalars, strings, lists,
closed-object field access, enum `case` expressions, local and returned
capturing closures, callbacks in `list.map`, `list.filter`, `list.fold`,
`list.some`, and `list.every`, plus calls through a conditional typed function
expression, bigint construction/arithmetic, and variadic functions with
empty/non-empty rest arguments. It also covers `result` patterns for statically typed `obj.get`,
including an inline closed object and dynamic reads on homogeneous closed
objects, `option` cases, and `result.try`,
`result.map`, `result.map-err`, and `result.recover` callbacks that change the
concrete success or error layout.
Add an export and a matching structural comparison here whenever native codegen
gains a supported value shape or intrinsic.

Unsupported native shapes remain covered by explicit `CodegenError` regression
tests and downstream compile-fail fixtures. The latter build a temporary crate
that invokes the proc macro and assert its Jisp diagnostic, so unsupported
shapes never degrade into opaque generated-Rust failures.
The macro suite also builds and runs a downstream bigint fixture with an
explicit `num-bigint` dependency, proving the concrete generated ABI instead of
only inspecting tokens.

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

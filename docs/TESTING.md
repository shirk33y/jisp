# Testing strategy

Existing tests cover sources, parsers, lowering/evaluation, cross-syntax
normalisation, type instantiation, unification, module resolution, portable
language fixtures, and the native Rust subset. Proc-macro integration fixtures
compile and execute generated scalar, structural, and imported Rust programs.

Add broader snapshot tests for diagnostics and schema. Native codegen still
needs systematic compile-fail fixtures and interpreter-vs-native differential
tests.

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
Use `assert` and ordinary Jisp predicates; there is no special equality
assertion. Negative tests use
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

### Backend conformance contract

Portable fixtures are the canonical semantic corpus, but they do not all have
the same backend obligations. The support inventory selects one of three
outcomes for each stable fixture/test identity in scope of native conformance:

- `supported`: map the portable semantic contract to an interpreter/native
  differential test that compares observable output structurally;
- `intentionally-rejected`: map the portable semantic contract to a downstream
  native-compilation test that asserts the documented Jisp diagnostic;
- `interpreter-only`: run only the portable frontend/interpreter contract.

The harness registers individual portable tests with Cargo and checks
cross-syntax equivalence. `native_contract.rs` verifies that every linked ID
exists and is linked at most once. Native differential and compile-fail tests
may retain dedicated integration fixtures for proc-macro, ABI, or diagnostic
coverage; the inventory makes that relationship explicit. Target selection is
never guessed from source forms.

### Cross-syntax semantic parity

The build script treats each canonical `tests/language/*.lisp` fixture and its
JSON, YAML, and `ws` peers as one registry. A missing peer, orphaned generated
fixture, duplicate ID, changed test name, changed `test`/`test-error` kind, or
changed order fails the build. The generated parity suite also compares the
source-aware AST after quote-alias normalization and macro expansion, then
executes every logical test through all four spellings and compares a normalized outcome:
`assert-passed`, `JISP-LOWER`, or `JISP-TYPE` at the same frontend stage. It
deliberately does not compare source byte spans or rendered message prose.

`tests/language/parity-boundaries.lisp` and its generated peers exercise nested
data, Unicode/escaping, quote/unquote macro expansion, and a portable type
error. UI fixtures remain on their renderer-aware runner and are outside this
language-only parity gate.

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
can observe `(ui.test.state)`, the escaped static `(ui.test.html)`, the raw
renderer-neutral `(ui.test.tree)`, reducer-declared `(ui.test.commands)`, or
reducer-declared `(ui.test.subscriptions)`. The resource accessors report the
declarations from the most recent `dispatch`; they never invoke a host
capability. To test a reducer completion, declare the deterministic fixture
host's capabilities before other steps, then deliver a portable result or
stable host error:

```lisp
(ui.test "save completes"
  (supports "storage.write" 1)
  (dispatch Save)
  (deliver command "save:1" 42)
  (assert (= 42 (ui.test.state))))
```

`deliver` chooses the currently active generation, materializes the resource's
`on-ok` action template, and calls the normal reducer. `deliver-error` takes a
resource kind/id plus one of `unsupported-capability`, `permission-denied`,
`invalid-request`, `cancelled`, or `host-failure`, and an error message; it
does the same with `on-error`. This is deterministic host simulation, not I/O.
Each assertion also compares the reference
component value with the compiled JUIR execution, so a passing test covers both
the reducer and renderer contract without a browser. Keep pure helper tests in
ordinary `(test ...)` fixtures; UI scenarios should only exercise the
`ui.app` boundary.

The playground recognises the same forms. Its **Run tests** button executes
them in Wasm and shows pass/fail results, while the preview compiles the source
with fixture-only `ui.test` forms removed.

## Browser-host regression

`scripts/test-playground.sh` drives a real locally built Wasm playground with
agent-browser. It verifies that a controlled input retains its DOM identity,
focus, value, and selection after an incremental reducer update and SSR
hydration; that a focused keyed row survives an in-place reorder; and that the
preview keeps a stable scrollbar gutter and width. The test communicates with
the opaque sandboxed iframe only through its existing host message boundary and
a read-only diagnostic probe; it does not evaluate Jisp or access preview DOM
state from the outer page.

Build the package first, then run the regression:

```sh
wasm-pack build crates/jisp-wasm --target web --out-dir ../../playground/pkg --out-name jisp_wasm
scripts/test-playground.sh
```

CI installs the pinned `agent-browser` release and runs this command on every
push and pull request.

## Native conformance

`docs/native-support.json` is the machine-checked native support inventory.
`crates/jisp-macros/tests/native_contract.rs` checks that every row names an
existing fixture, owning test, portable-test ID when applicable, backend
obligation, and row in `docs/NATIVE.md`.

The inventory is the bridge from portable language fixtures to native
conformance. A linked semantic row names a stable portable fixture/test ID and
one backend obligation plus a concrete conformance runner. The contract test
validates the link, and `native_conformance.rs` runs every declared runner; it
does not infer native eligibility from source syntax. Native-only fixtures
remain appropriate for proc-macro, ABI, and generated-diagnostic integration
checks.

## Offline package determinism

Package tests cover local registry indexing, stable sorted lockfiles, verified
cache reuse after the registry disappears, checksum and malformed-index
failures, rollback of provisional lock/cache writes, cache-name collision
avoidance, and rejection of escaped lockfile sources. Remote registry URLs are
local failures; no test or command selects a network path.

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
The same fixture separately proves `list.get` and `list.slice` success/error
boundaries, empty typed callback pipelines, closed-object views (`len`, `has`,
keys, values, `to-map`), ordered map views (`cat`, `len`, `has`, keys, values),
and a nested-pattern fallback branch. These are native-only integration rows:
they exercise generated concrete layouts rather than a portable fixture ID.
Add an export and a matching structural comparison here whenever native codegen
gains a supported value shape or intrinsic.

Unsupported native shapes remain covered by explicit `CodegenError` regression
tests and downstream compile-fail fixtures. The latter build a temporary crate
that invokes the proc macro and assert its Jisp diagnostic, so unsupported
shapes never degrade into opaque generated-Rust failures.
The macro suite also builds and runs a downstream bigint fixture with an
explicit `num-bigint` dependency, proving the concrete generated ABI instead of
only inspecting tokens. `native_examples.rs` runs the five maintained examples
under `examples/` through both paths; three use multiple modules, named domain
types, and explicit error paths. See `docs/NATIVE.md` for the current boundary
and ABI promise.

`native_embedding.rs` builds the checked, documented downstream crate in
`tests/fixtures/downstream-embedding` offline. Its paired failure fixture
asserts the Jisp type diagnostic, source file, and source range presented by a
proc-macro expansion.

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

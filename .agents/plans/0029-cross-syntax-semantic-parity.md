# Plan: cross-syntax semantic parity (3–4 hours)

**Status:** proposed. **Estimate:** 3–4 hours.  This is infrastructure work
across the fixture generator, portable test runner, and checked-in corpus. It
does not introduce a fifth syntax or change the language contract.

## Why this is a real timebox

The repository has 14 canonical Lisp language fixtures and 42 generated JSON,
YAML, and `ws` counterparts. Each currently runs independently. The missing
contract is relational: every spelling must expose the same ordered test IDs
and produce the same normalized outcome. Implementing that contract requires a
generated registry, a reusable outcome API, positive and negative parity
fixtures, and failure reports that identify the exact syntax/file/test tuple.

## Goal

Make semantic parity across Lisp, canonical JSON, restricted YAML, and `ws` a
machine-checked invariant of the portable fixture corpus. A fixture change must
fail fast when a generated counterpart is missing, structurally divergent, or
behaviourally different.

## Timeboxed work

### 0:00–0:30 — inventory and contract boundary

1. Audit `crates/jisp-eval/build.rs`, `portable_support`, the current
   single-example `syntax_equivalence` test, and all fixture shapes.
2. Define the parity unit precisely: canonical fixture path + ordered logical
   test ID + test form kind (`test` or `test-error`) + normalized outcome.
3. Record explicit exclusions. UI fixtures keep their existing UI-specific
   runner unless the same outcome interface can represent them without leaking
   renderer details.

### 0:30–1:25 — generated fixture registry

Extend the build generator to produce one registry from the canonical fixture
and its three generated peers. It must reject during build when any of these is
true:

- a canonical Lisp fixture lacks JSON, YAML, or `ws` counterpart;
- a generated fixture is orphaned;
- test names, form kinds, or their order differ from canonical;
- a fixture has duplicate stable IDs.

Generate a Rust test module with clear failure labels such as
`objects-ui::map keys::yaml`. Keep source discovery in the build script; do not
create a second ad-hoc filesystem scanner in tests.

### 1:25–2:30 — normalized runtime outcomes

Refactor the portable language runner just enough to expose a test outcome to
the generated parity test instead of only panicking/asserting internally.
Compare all four syntaxes for each logical fixture test:

- positive tests: equal successful observable result;
- negative tests: same frontend stage/category and diagnostic code, while not
  requiring equal byte spans or prose wording;
- fixture runner failures: include canonical and peer path plus test ID.

Retain the existing per-syntax tests; parity is an additional relational gate,
not a replacement.

### 2:30–3:05 — adversarial corpus and repair

Add at least four deliberately sensitive semantic cases to an existing/new
portable fixture, represented in all four syntaxes:

1. nested list/object data where form-vs-data interpretation matters;
2. string escaping and Unicode;
3. macro quote/unquote expansion that normalizes before lowering;
4. a negative type/lowering case with a stable diagnostic code.

If parity exposes a reader/lowering divergence, fix it at the shared seam and
add the smallest regression case. Do not normalize away a real semantic
difference in the test harness.

### 3:05–3:30 — documentation, gate, handoff

Document the exact parity promise, intentional diagnostic-span exception, and
fixture authoring rule in `docs/TESTING.md`. Run:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
cargo test -p jisp-macros --quiet
```

Commit one conventional patch and leave a compact evidence ledger: fixtures
covered, generated registry behaviour, negative-code policy, and any UI
exclusion.

## Done when

- Every canonical language fixture has all three generated syntax peers and an
  identical ordered logical test registry.
- The generated parity test executes every logical test through all four
  syntaxes and compares normalized outcomes.
- At least four sensitive cases prove the harness catches data, Unicode,
  macro, and negative-diagnostic boundaries.
- Existing individual portable tests remain green and diagnostic spans are not
  falsely required to match across source spellings.

## Execution ledger

| Boundary | Proof |
| --- | --- |
| Fixture topology | build-time registry requires canonical Lisp plus JSON, YAML, and `ws`; orphan peers fail generation |
| Logical registry | generated peers must have the same ordered test names and `test`/`test-error` kinds |
| Runtime semantics | generated parity tests run every logical test through four syntax readers and compare normalized outcomes |
| Negative semantics | expected failures compare lower/type stage and `JISP-LOWER`/`JISP-TYPE`, not spans or prose |
| Sensitive corpus | `parity-boundaries` covers nested data, escaped Unicode, quote/unquote expansion, and a type failure |
| UI exclusion | UI stays on its renderer-aware portable runner; it is not silently compared as a language-only outcome |

## Cut line

Do not add raw JSON objects, YAML maps, a source formatter/converter, snapshot
baselines for text diagnostics, UI renderer parity, or native codegen parity in
this slice. Those require separate contracts.

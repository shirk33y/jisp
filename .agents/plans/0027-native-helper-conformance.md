# Plan: native helper conformance (3-hour slice)

**Status:** proposed. **Timebox:** about 3 hours.  No source syntax, runtime
ABI, or fallback-to-`Value` change is in scope.

## Goal

Turn the current coarse native inventory rows into precise behavioural
obligations for the highest-risk supported helpers.  Each selected operation
must either prove interpreter/native equivalence or have an explicit,
source-mapped rejection test.

## Work plan

### 0:00–0:25 — establish the gap ledger

1. Read `docs/native-support.json`, native differential tests, compile-fail
   fixtures, and the emitted helper surface.
2. Make a compact checked-in ledger in this plan or the inventory notes:
   `helper -> current proof -> gap -> disposition`.
3. Prioritise behaviour that can silently diverge despite a passing broad row:
   immutable collection updates, missing map keys, callback helpers, result
   error paths, and pattern/case fallthrough.
4. Do not count a generic fixture assertion as proof of an operation unless it
   exercises its observable success or failure path.

### 0:25–1:55 — add focused differential cases

Add small named native fixtures/tests (reuse the existing shared fixture only
when the assertion remains obvious) for the gaps found above.  Target at least
five independent boundaries:

1. list callback edge path: empty input and callback result;
2. immutable object/map update: old value remains readable after update/delete;
3. map lookup: present and missing key behaviour;
4. `result` helpers: `ok` transformation and `err` preservation/recovery;
5. nested `case`: payload binding plus non-matching branch.

For each, execute the interpreter and proc-macro/native export, then compare
the structural result.  Keep test names public enough to become inventory
runner targets.  Do not make native support broader merely to pass a test.

### 1:55–2:25 — rejection and diagnostic edges

Audit the selected helpers for an intentional native boundary.  Where one is
already supported by the frontend but impossible in the concrete ABI, add or
tighten a downstream macro compile-fail test that asserts:

- a Jisp diagnostic code/message, not rustc-only fallout;
- the Jisp source range or excerpt; and
- no generated `Value`/dynamic ABI fallback.

If no honest additional rejection is found in the timebox, record that result;
do not invent a restriction.

### 2:25–2:45 — make inventory and docs exact

Split only the coarse inventory rows whose new tests demonstrate distinct
contracts.  Every new row gets a unique runner, fixture, expected observable
result, and portable test ID when one exists.  Update `docs/NATIVE.md` and
`docs/TESTING.md` only where the matrix wording would otherwise overclaim.

### 2:45–3:00 — verification and handoff

Run:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
cargo test -p jisp-macros --quiet
```

Commit one coherent conventional-commit patch.  Finish with the ledger: tests
added, rows split, remaining helper gaps, and the next smallest conformance
slice.

## Done when

- At least five meaningful helper/edge contracts are differential-tested.
- Every changed inventory row runs through `native_conformance`.
- Any added unsupported shape fails with a Jisp diagnostic at the source.
- Existing user changes stay unstaged and all required checks pass.

## Execution ledger

| Boundary | Proof after this slice | Disposition |
| --- | --- | --- |
| `list.get` | present and out-of-bounds result branches | added native-only differential row |
| `list.slice` | valid and out-of-bounds result branches | added native-only differential row |
| list callbacks | filter creates empty typed list, then `map` preserves it | added native-only differential row |
| object views | `len`, `has`, keys, values, and `to-map` | added native-only differential row |
| map views | `cat`, `len`, `has`, keys, and values | added native-only differential row |
| nested `case` | failed alternative reaches wildcard fallback | added native-only differential row |
| result error paths | map-err/recover/try | existing differential row remains sufficient |
| concrete ABI rejections | UI values, heterogeneous dynamic access, open-row polymorphism | existing downstream compile-fail rows remain the honest boundaries; no new restriction added |

## Cut line

Stop after the selected helper families and record uncovered helpers.  Remote
packages, CRDT, new language features, open-row monomorphisation, and broad
stdlib expansion are expressly out of scope for this slice.

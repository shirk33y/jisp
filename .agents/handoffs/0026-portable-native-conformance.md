# Portable/native conformance handoff

## Goal

Make portable language fixtures the semantic source of truth for native
conformance without pretending every Jisp program belongs to the native subset.

## Contract

- A named `test` or `test-error` form has a deterministic stable ID derived
  from fixture path and test name.
- `docs/native-support.json` owns the backend obligation for each linked ID:
  `supported`, `intentionally-rejected`, or `interpreter-only`.
- A supported row runs the canonical source through the interpreter and native
  facade, then compares structural observable output.
- An intentionally rejected row proves the documented Jisp diagnostic through
  downstream native compilation.
- Native-only fixtures are retained only for proc-macro, concrete ABI, or
  generated-diagnostic integration. They are not substitutes for semantic
  coverage.

## Current state

- `tests/language/*.lisp` plus generated JSON/YAML/WS fixtures already provide
  named portable tests and Cargo-visible registration.
- `docs/native-support.json` already inventories support/rejection boundaries.
- Native differential and compile-fail tests still use mostly separate
  fixtures. The missing work is the explicit link between inventory rows and
  portable test IDs.

## Suggested implementation order

1. Extend the portable fixture build-time registry to expose the deterministic
   ID in addition to its Cargo test name.
2. Add optional `portable_test_id` and `backend_obligation` fields to the
   native inventory schema and contract test.
3. Migrate one supported scalar/list row and one rejected UI/open-row row end
   to end, including clear failure messages.
4. Generate or invoke native parity/diagnostic checks from the inventory.
5. Migrate remaining semantic rows; keep only genuine native integration tests
   outside the portable corpus.
6. Update `docs/NATIVE.md` from the checked inventory and run the full suite.

## Guardrails

- Do not change Jisp source syntax or make native support implicit.
- Do not force UI/effects or other intentionally interpreter-only behaviour
  through codegen.
- Keep the interpreter as the reference implementation.
- Preserve Cargo filtering of individual portable tests.

# Testable documentation

## Goal

Make Jisp documentation runnable enough that examples and reference snippets
cannot drift silently from parser, evaluator, codegen, or portable `.lisp`
semantics.

## Problem

The current docs are useful as a snapshot, but most examples are prose-only.
As the language surface grows, `docs/SPEC.md`, `docs/STDLIB.md`, README examples,
and portable `.lisp` tests can diverge unless one path owns the examples and the
other paths verify them.

`docs/TESTING.md` also still says the archive was written without executing
Rust. That is now stale and should be replaced when this plan is implemented.

## Design

Use executable documentation examples as the source of truth.

- Keep docs concise: signature, one-sentence semantics, one or two examples,
  edge behavior only when it changes real usage.
- Store runnable examples as fenced code blocks tagged with a stable test name.
- Support examples in normal docs first; avoid inventing a separate tutorial DSL
  unless Markdown extraction becomes painful.
- Generate one Rust `#[test]` per documentation example so `cargo test` can list,
  filter, and report individual doc examples.
- Prefer portable `.lisp` examples for language and stdlib docs. Use Rust-only
  tests only for backend, ABI, macro, or diagnostic behavior that cannot be
  expressed in Jisp yet.

Proposed Markdown shape:

````md
```jisp test=stdlib.str.slice.ok
(assert.equal (str.slice "żaba" 0 2) "ża")
```
````

Expected implementation shape:

- A small extractor reads selected Markdown files and extracts fenced `jisp`
  blocks with `test=<name>`.
- The extractor validates duplicate names, missing names, unsupported block
  attributes, and empty examples.
- Generated Rust test functions sanitize names into valid identifiers while
  keeping the original name in failure output.
- The generated tests call the existing portable `.lisp` runner or the same
  evaluator facade used by portable tests.
- Docs can include non-runnable `text`, `json`, or `yaml` blocks, but every
  normative Jisp example should eventually be runnable.

## Milestones

### P2.1 Extract And List Doc Examples

- Add a doc-example extractor for Markdown fenced code blocks.
- Generate one Rust test per `jisp test=<name>` block.
- Wire it into `cargo test --workspace --exclude jisp-macros`.
- Ensure `cargo test stdlib.str.slice` or a sanitized equivalent can filter a
  single documentation example.

Acceptance:

- `cargo test` lists doc examples as separate tests.
- Duplicate doc test names fail with a clear error.
- Non-test code blocks remain allowed.

### P2.2 Convert Core Reference Examples

- Convert representative examples in README, `docs/SPEC.md`, and
  `docs/STDLIB.md` to runnable doc tests.
- Cover at least: numbers, bigint, `case`, `use`, object lookup, list helpers,
  result helpers, and `ui.html`.
- Keep existing portable `.lisp` tests as regression fixtures; doc examples
  should be small reference examples, not exhaustive suites.

Acceptance:

- Each implemented P0/P1 language feature has at least one runnable docs
  example.
- Failing docs examples point at the originating Markdown file and test name.

### P2.3 Unify Docs And Portable Tests

- Decide which examples live primarily in docs and which live primarily in
  `tests/language`.
- Add a short convention to `docs/TESTING.md`.
- Replace stale testing text with current commands and doc-test workflow.
- Add a CI/pre-commit note only after the workflow is stable and quiet.

Acceptance:

- New language/std functions have a documented example and a regression test
  path.
- The docs workflow is cheap enough to run with the normal workspace tests.

## Non-goals

- No broad documentation rewrite before the extractor works.
- No generated website in this milestone.
- No separate docs syntax beyond small Markdown fence attributes.
- No attempt to make every diagnostic or compile-fail example runnable until
  diagnostics/source maps are back in scope.

## Open Questions

- Should doc examples be allowed to contain multiple assertions, or should each
  fenced block stay one logical assertion for better cargo filtering?
- Should expected failures use a second attribute like `error=...`, or should
  they be ordinary Jisp assertions over `result` values where possible?
- Should generated tests be checked in for grepability, or produced in
  `OUT_DIR` to avoid churn?

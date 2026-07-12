# jisp

## Project Context

- This repository is a foundation and handoff package for a Rust implementation of
  Jisp, not a finished compiler.
- Start by reading `README.md`, `docs/SPEC.md`, `docs/ARCHITECTURE.md`,
  `docs/AGENT_HANDOFF.md`, and `TODO.md` before changing language semantics.
- Do not redesign surface syntax before completing the existing seams.
- Treat the `old` branch and earlier TypeScript code as reference material only.

## Architecture Rules

- Keep parser crates limited to syntax normalization into the shared source-aware
  AST. Do not add language semantics there.
- Implement language features against `jisp-ir`, shared runtime helpers, and the
  facade crates rather than separately per syntax.
- Native Rust compilation belongs in `jisp-codegen-rust`; proc macros should
  track dependencies and call stable frontend/codegen seams, not own parsing or
  type logic.
- Do not implement raw `{}` metadata or FFI opportunistically. Write or update
  the design first.
- When porting code from another project, record a GitHub permalink for every
  copied or closely adapted source. Include repository URL, commit hash, file
  path, and line numbers.

## Workflow

- Use Conventional Commits for every commit: `type(scope): concise summary`.
  Prefer `feat`, `fix`, `refactor`, `test`, `docs`, `build`, or `ci`; omit the
  scope only when no meaningful scope applies.
- Batch related edits across files before verification. Prefer one coherent patch
  over many tiny edit/check loops.
- Prefer long, forward-moving implementation sessions over cautious
  micro-iterations. Build a coherent slice quickly, then run a repair/cleanup
  pass, then validate and commit. Auto-compaction has enough context budget for
  this style; do not prematurely stop just to preserve context.
- Prefer fewer, denser shell tool calls. Chain related read-only or validation
  commands in one shell when ordering is clear, for example `cmd1 && cmd2` for
  dependent checks or `cmd1; cmd2` for independent diagnostics. Keep output
  capped and readable.
- Use the fastest decisive diagnostic first. Avoid broad exploration once a
  focused parser/lowering/evaluator/type test proves the boundary.
- Do not run clippy, pre-commit, or broad verification after every small change.
  Use focused non-Cargo diagnostics while fixing, then run the required
  validation once near the end. When running broad checks, combine them with the
  other final validation commands instead of launching each one as a separate
  pass.
- Trust the configured pre-commit hook as the final local gate. Do not over-sample
  the same code path with repeated tiny Cargo runs before that gate unless a
  specific failure needs isolation.
- For docs-only work, stage only owned documentation changes. `git commit
  --no-verify` is acceptable for documentation-only changes, including
  `.agents/` notes, todos, plans, and this file.

## Tests

- Add regression tests as soon as a bug or behavior contract is identified.
- Do not run selective `cargo test` filters in this repo. The workspace test
  suite is fast enough that running all tests saves tool calls and catches
  cross-crate regressions. When Cargo tests are needed, run the whole allowed
  suite at once with `cargo test --workspace --exclude jisp-macros --quiet`.
- Keep test output quiet by default; surface only failures or actionable errors.
- Integration tests live in `tests/` at the crate root, as in
  `crates/jisp-eval/tests/`.
- For new or touched unit-test coverage, put tests beside the module they test
  as `<module>_test.rs` in the same directory, for example `lower.rs` uses
  `lower_test.rs`. Keep the tests only in that sibling test file; the production
  module should only declare it with:

  ```rust
  #[cfg(test)]
  mod module_test;
  ```

- Use `#[cfg(test)]` on the module declaration, not inside the whole test file.
- Existing inline or differently placed unit tests may be migrated later; do not
  expand them when adding new coverage.
- CI validation is:

  ```text
  cargo fmt --all -- --check
  cargo test --workspace --exclude jisp-macros --quiet
  ```

## File Size

- Keep new or heavily edited `.rs` files under 500 lines when practical.
- 1000 lines is the hard limit; split modules before hitting it.
- Existing larger files such as `crates/jisp-eval/src/builtins.rs` and
  `crates/jisp-ir/src/lower.rs` are not a reason to grow them further.

## Project .agents

- Use `.agents/handoffs/` for compact next-thread startup notes tied to this
  repo. Keep them committed when they are part of active project handoff.
- Use `.agents/plans/` for design or exploration notes that should survive the
  chat thread.

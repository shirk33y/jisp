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

- Batch related edits across files before verification. Prefer one coherent patch
  over many tiny edit/check loops.
- Use the fastest decisive diagnostic first. Avoid broad exploration once a
  focused parser/lowering/evaluator/type test proves the boundary.
- Do not run clippy, pre-commit, or broad verification after every small change.
  Use focused checks while fixing, then run the required validation once near the
  end.
- For docs-only work, stage only owned documentation changes. `git commit
  --no-verify` is acceptable for documentation-only changes, including
  `.agents/` notes, todos, plans, and this file.

## Tests

- Add regression tests as soon as a bug or behavior contract is identified.
- Prefer the full relevant Rust suite over a narrow `cargo test <name>` filter
  when it is fast enough.
- Keep test output quiet by default; surface only failures or actionable errors.
- Integration tests live in `tests/` at the crate root, as in
  `crates/jisp-eval/tests/`.
- Unit tests may live beside the module they test as `<module>_test.rs`, declared
  from the parent module with:

  ```rust
  #[cfg(test)]
  mod module_test;
  ```

- Use `#[cfg(test)]` on the module declaration, not inside the whole test file.
- CI validation is:

  ```text
  cargo fmt --all -- --check
  cargo test --workspace --exclude jisp-macros
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

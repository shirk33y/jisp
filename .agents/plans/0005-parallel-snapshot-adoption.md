# Parallel snapshot adoption notes

Snapshot reviewed: `jisp-mvp-current-2026-07-10.zip`, received 2026-07-10.

## Status

- Treat the snapshot as reference material, not as a direct import.
- It is a parallel implementation line centered around a new `jisp-compiler`
  crate.
- It is not buildable as delivered: its workspace lists `jisp-codegen-rust`, but
  the zip does not contain that crate.
- The snapshot itself states that Rust tooling was not run.

## Adopt

- Compiler staging names and boundaries:
  `loader -> macros -> desugar -> lower -> exhaustive -> tail -> pipeline`.
- A facade compiler pipeline that aggregates diagnostics and preserves source
  ranges before reaching the native backend.
- The idea of a runtime/intrinsic registry that records arity, type scheme, and
  native Rust lowering metadata for builtins.
- Exhaustiveness and redundancy checks as a separate compiler stage instead of
  burying all pattern policy in type inference.
- Tail-position marking as an explicit IR pass before backend-specific codegen.

## Do not adopt directly

- Do not replace the current `TypedModule -> jisp-codegen-rust` P1 seam.
- Do not use the snapshot's dynamic `Value` representation as native ABI.
  Generated Rust must keep using concrete typed layouts or fail codegen.
- Do not copy the snapshot workspace layout wholesale; it omits current crates
  and conflicts with current project milestones.

## Follow-up

- When P1 scalar/function native codegen is stable, add a small compiler facade
  that mirrors the snapshot's stage order while feeding the existing
  `TypedModule` backend contract.
- Split builtin lowering metadata out of ad hoc evaluator/codegen logic into a
  typed registry, but keep native and interpreter execution paths separate.
- Revisit the snapshot's `exhaustive` and `tail` modules during P2 case-pattern
  and optimization work.

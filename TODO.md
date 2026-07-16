# Jisp remaining work

This file lists unfinished work only. Current capabilities live in
[README.md](README.md); product order lives in [ROADMAP.md](ROADMAP.md).
Completed milestones stay in Git history and their owning documents.

## Next

1. Build a complete interpreter/native conformance matrix: differential tests,
   compile-fail fixtures, diagnostics, and executable documentation.
2. Publish the native support boundary and compatibility policy.
3. Polish deterministic packages, LSP, and diagnostics without adding hidden
   network behavior.

## Deferred by design

- Raw `{}` stays unsupported in canonical JSON and YAML. `json-data-v1` and
  `yaml-data-v1` are research only; require an accepted profile contract and
  conformance corpus before implementation.
- FFI: first define ABI, ownership, errors, dynamic-library delivery, headers,
  and optional binding generation.
- No runtime `eval`, classes, methods, Rust-surface idioms, GC, or dynamic
  `any` in the core language.
- Native open-row monomorphisation and heterogeneous dynamic selection need a
  source-visible type and ABI design. Never use `jisp_eval::Value`,
  `serde_json::Value`, or `Box<dyn Any>` as the compiled ABI.
- General compile-time evaluation needs a capability, sandbox, dependency, and
  determinism contract.
- Full arbitrary structural pattern-matrix checking remains future work.
- Remote registry lookup/downloads need complete network, checksum, lockfile,
  cache, and trust policy first.

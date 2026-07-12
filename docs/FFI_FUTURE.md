# Deferred FFI design notes

FFI is intentionally not implemented.

It is deferred behind the language and native-codegen milestones in
[ROADMAP.md](../ROADMAP.md), so no prelude API should accidentally become an
unstable host ABI.

Before implementation, specify:

- direction A: Jisp calls external Rust/host functions;
- direction B: hosts call exported compiled Jisp functions;
- stable C ABI as the universal baseline (`cdylib`, `.so/.dll/.dylib`, `.h`);
- ownership/allocation/freeing rules and panic containment;
- ABI representations for strings, bytes, lists, records, enums, and Result;
- a small generated binding IR;
- optional SWIG/UniFFI/Diplomat/PyO3/N-API adapters;
- JSON-call fallback only for coarse operations, not the fast typed ABI.

Do not attach semantics to raw `{}` metadata until a separate decision is made.

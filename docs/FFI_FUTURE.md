# Deferred FFI design notes

FFI is intentionally not implemented.

It is deferred behind the language and native-codegen milestones in
[ROADMAP.md](../ROADMAP.md), so no prelude API should accidentally become an
unstable host ABI.

For the complementary multi-host process-runner and portable-value strategy,
see [MAL and multi-host execution](research/MAL.md). That document is not an
FFI design: it deliberately keeps broad process interoperability separate from
the narrow, native ABI proposed here.

[Interop architecture](research/INTEROP.md) compares the process, component,
C-ABI, generated-adapter, and direct-codegen layers. It is the decision guide
for selecting a host integration without turning a language-specific extension
mechanism into the Jisp runtime model.

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

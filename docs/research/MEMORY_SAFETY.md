# Memory safety: current status and design contract

This note records what Jisp can truthfully claim about memory safety today and
what must exist before making a stronger claim. It is an implementation audit,
not a formal proof or a new language-semantics proposal.

Reviewed on 2026-07-15 against revision
`08dbe39bc63682edae8f4a7bb9383be278ac633a`.

## Verdict

Jisp's **currently supported language subset is memory-safe by construction**:
Jisp source exposes no pointers, manual allocation, casts, mutable aliases, or
FFI, and its native path accepts typed Jisp IR before emitting Rust.

Jisp must **not** claim to be "100% memory safe" without qualification. That
would imply a whole-process, formal guarantee covering the Jisp implementation,
Rust compiler and dependencies, generated code, Wasm/host glue, operating
system, and future extensions. The project has not established such a proof or
completed an independent security audit.

The accurate public claim is:

> The supported Jisp subset has no source-level path to memory-unsafe
> operations and its native backend emits safe Rust. This is a design and
> implementation property, not yet a formal end-to-end proof.

## What was checked

The current checkout was indexed with `agent-cmm`. Its code search for `unsafe`
and `extern` found no raw implementation matches. The `unsafe` hits are Rust
keyword filters in code generators and an HTML-sanitisation diagnostic; the
`extern` hits are likewise keyword filters, ordinary English text, and plan
documents. This is useful evidence, but it is deliberately not presented as a
substitute for a dependency audit or a compiler proof.

The supported native pipeline has two explicit gates:

1. [`generate_rust_module`](../../crates/jisp/src/lib.rs#L547-L556) resolves
   imports and infers a `TypedModule` before invoking native code generation.
2. [`generate_detailed`](../../crates/jisp-codegen-rust/src/lib.rs#L54-L58)
   classifies the module's native layout and returns a `CodegenError` before
   emission when a representation is unsupported.

The classifier and emitter reject unsupported shapes rather than smuggling
them through a universal, type-erased runtime value. Regression tests cover
that no-fallback boundary, including
[`rejects_unsupported_native_shapes_without_value_fallback`](../../crates/jisp-codegen-rust/src/emit_test.rs#L1009-L1019)
and layout rejection for open object rows.

FFI is not currently part of the Jisp surface or implementation contract. It
is deliberately deferred in [FFI future](../FFI_FUTURE.md); that document
requires ownership, allocation, freeing, ABI representation, and panic rules
before any boundary is added.

## Scope of the safety claim

| Layer | Current status | What it means |
| --- | --- | --- |
| Supported Jisp source | Strong boundary | No raw pointers, manual memory management, unchecked casts, or arbitrary host calls are available to a Jisp program. |
| Jisp implementation source | Observed safe-Rust discipline | The audited repository source has no executable `unsafe` or `extern` block. This is an observed property, not yet an enforced workspace policy. |
| Native code generation | Typed and layout-checked | The compiler emits from `TypedModule`; unsupported layouts fail before emission instead of relying on an untyped escape hatch. |
| Interpreter behaviour | Memory-safe, not panic-free | Safe Rust containers and interior mutability can still panic on a violated runtime borrow rule; a panic is an availability/correctness problem, not undefined behaviour. |
| Wasm and host integration | Trusted boundary | Wasm bindings and host APIs are implementation dependencies. They may contain internal unsafe code outside Jisp's source-level control. |
| FFI/native plugins | Not implemented | No claim can be extended across these boundaries until their ABI and ownership contract are designed and audited. |

## What this does and does not protect against

Memory safety means a normal Jisp program cannot directly cause use-after-free,
double-free, dangling-pointer dereference, out-of-bounds memory access, or a
data race through language-level primitives.

It does **not** promise any of the following:

- no panic, stack overflow, integer error, out-of-memory termination, or denial
  of service;
- semantic correctness of every Jisp program or native-codegen result;
- constant-time execution or protection from side channels;
- safety of a dependency, the Rust toolchain, the host process, or the operating
  system;
- safety across a future FFI, embedded runtime, native extension, or a new
  concurrency runtime.

This distinction matters for the async/actor work: structured task ownership,
bounded queues, cancellation safety, affine handles, and protocol checking
would improve lifecycle and state safety. They do not by themselves strengthen
the existing low-level memory-safety boundary, which is already delegated to
safe Rust and the absence of raw Jisp escape hatches.

## Why the current design is a good foundation

Jisp's architecture already makes the safe choice the natural one:

- values are immutable at the language level, so ordinary code does not create
  mutable aliases;
- type inference and typed IR sit before both interpretation and native output;
- native layout classification narrows the subset instead of inventing an
  untyped universal carrier for unsupported values;
- expected failures use `result` values rather than memory-adjacent exception or
  foreign-control-flow mechanisms;
- host/FFI work is deferred rather than exposed as an untyped convenience API.

These are stronger foundations than merely asking users to avoid dangerous
operations. They do not eliminate implementation bugs, so the boundary must
remain explicit in documentation and tests.

## Recommended hardening before a stronger claim

The following are implementation work items, not features promised by this
note:

1. **Make the source audit mechanical.** Add `#![forbid(unsafe_code)]` to every
   Jisp-owned Rust crate that does not implement an approved boundary. A CI
   check should reject an `unsafe` block or `extern` declaration outside a
   dedicated boundary crate.
2. **Test emitted Rust under the same rule.** Compile representative generated
   native fixtures with `#![forbid(unsafe_code)]` so a future emitter change
   cannot silently introduce unsafe output.
3. **Keep FFI capability-scoped.** When FFI is designed, put all unsafe code in
   one small, audited adapter crate with explicit ownership transfer, allocation
   and freeing, nullability, panic containment, versioning, and test vectors.
   Do not add raw pointers or arbitrary host objects to portable Jisp values.
4. **Audit dependencies and generated glue.** Use a dependency audit/SBOM and
   inspect Wasm/bindgen code at release time. Safe Jisp source cannot remove
   risk from a vulnerable native dependency.
5. **Fuzz the language boundary.** Fuzz every parser and the parse → lower →
   infer → evaluate/codegen pipeline. This chiefly finds panics and semantic
   inconsistencies, but it is a practical guard against unsafe-boundary bugs.
6. **Add property and differential tests.** Continue comparing interpreter and
   native results, especially around ownership-like collection updates, deeply
   nested data, malformed input, and all rejected native layouts.

## Acceptance criteria for the wording “memory safe”

It is reasonable to say **"memory-safe supported subset"** when all of these
remain true:

- no Jisp syntax exposes raw pointers, unchecked casts, manual frees, or an
  arbitrary native-code escape hatch;
- Jisp-owned implementation crates compile with `forbid(unsafe_code)` except
  for explicitly documented and reviewed boundary crates;
- emitted Rust is checked against the same prohibition;
- FFI and native plugin APIs are absent or capability-scoped with a written ABI
  and ownership specification;
- dependency and host-boundary caveats remain visible in release documentation.

Reserve **"formally memory-safe"** or **"100% memory safe"** for a later stage
only if the project can name the formal model, trusted computing base, proof or
verification method, and explicit treatment of generated code and all host
boundaries.

## Related documents

- [Architecture](../ARCHITECTURE.md) defines the compiler and runtime seams.
- [FFI future](../FFI_FUTURE.md) owns the deferred ABI and ownership design.
- [Testing](../TESTING.md) owns executable-documentation and native-conformance
  test policy.

# Optional replicated-state backend

## Decision

CRDT is a future compiler/runtime backend for a state binding. It is selected
per binding as `local` or `replicated`; it is not a second language data type,
a second schema declaration, or part of the native ABI. Start only after native
conformance is complete and `jisp-wire/1` has a versioned unary runner with
value limits and cancellation.

For a domain type `T`, business code continues to receive and return ordinary
immutable `T` values. The same reducer or `T -> T` helper must run against a
local state binding, a deterministic test replica, or a replicated binding
without changing its source type. The initial target is one offline-capable UI
state binding, not a general distributed database or shared evaluator.

## Boundary

- Jisp values remain immutable, typed data.
- A state binding owns either a local cell or an internal replicated document;
  its public snapshot is always `T`. Clock, replica handle, socket, patch, and
  causal context are never ordinary Jisp values.
- The compiler derives `Replicable<T>` from the existing type declaration when
  a binding selects `replicated`. It produces a snapshot codec, operation
  codec, merge representation, and schema fingerprint without introducing
  `defdoc` or changing `T`.
- A local transition is an ordinary pure `T -> T` computation. The replicated
  backend lowers the transition to operations (or a deterministic diff/rebase)
  and emits them. Remote merge replaces the current snapshot and re-renders; it
  must not replay an old local action through business code or duplicate effects.
- The universal fallback is an LWW register for the whole closed value `T`.
  A binding-local profile may opt into structural object/map merge and finer
  text/sequence/counter semantics without changing the domain type or helper
  signatures.
- Reject functions, closures, tasks, actors, UI values, resources, host
  handles, and unsupported numeric/float values when they reach a replicated
  binding. Pure local state may still use values not eligible for replication.

## Required design before code

Write `docs/research/COLLAB_CRDT.md` that fixes:

1. `Replicable<T>` derivation and exact type mapping, including bigints,
   variants, results, bytes, float edge cases, canonical encoding, and schema
   fingerprints;
2. the state-binding contract: the same `T -> T` reducer for local and
   replicated state, purity/determinism rules, effect ownership, rebase, and
   the rule that remote merge never replays a local action;
3. automatic merge defaults: whole-value LWW fallback, object fields, maps,
   lists, strings, deletions, and the explicitly documented cases where a
   deterministic diff cannot recover user intent;
4. optional per-binding merge profiles for text, sequence, counter, set, and
   multi-value-register behaviour. Profiles configure a binding; they never
   fork the domain `type` declaration;
5. replica identity persistence, causal context, offline queue, reconnect,
   compaction, retention, and snapshot recovery;
6. capability authority, document access control, authentication, quotas,
   maximum document/patch sizes, operation rate, nesting depth, malformed
   patch handling, and invariants that require an authoritative command;
7. `jisp-wire/1` envelopes for snapshot, patch, acknowledgement, rejection,
   cancellation, and protocol/schema-version negotiation; and
8. observability and errors: stable public error codes, correlation IDs, and
   server-only diagnostic detail.

Do not reuse `json-joy` CRDT source. Its CRDT implementation is in the
AGPL-3.0-only `json-joy` package; the Apache-2.0 `json-crdt-repo` depends on
it. Use independently derived algorithms or a separately licensed Rust
dependency after a dedicated license and interoperability review.

## Design inspiration, not a protocol dependency

`json-joy` usefully separates snapshots, atomic patches, vector-clock causal
contexts, LWW registers/maps, and RGA sequences. Its three-message delta sync
is a useful experiment baseline. Jisp must define its own versioned wire
contract and conformance corpus; it must not expose JSON Reactive RPC or the
JSON CRDT patch format as a public Jisp compatibility promise.

Reference inspected: [json-joy at 32c3b5c](https://github.com/streamich/json-joy/tree/32c3b5c270035aa26d6ec3c4c1c7366fea1a75d9).

## Delivery stages

1. **Prerequisites:** finish the portable/native conformance migration and
   ship a bounded unary `jisp-wire/1` runner. No replicated-state code before
   both gates.
2. **Paper design:** publish the design above plus a schema and binary/text
   fixture corpus. Include local-versus-replicated runs of the same `T -> T`
   reducer, two-replica merge examples, and intentionally invalid values.
3. **Compiler prototype:** add a compiler-owned state-binding seam with local
   and deterministic in-memory-replica backends. Derive `Replicable<T>` for a
   small closed type subset and prove that existing business helpers retain
   their `T` signatures.
4. **Narrow replicated prototype:** add structural objects/maps and one
   sequence/text profile, two local replicas, explicit sync, and snapshot
   restore. Keep replication metadata out of ordinary Jisp values and native
   codegen ABI.
5. **Conformance hardening:** test offline divergence/reconnect, concurrent
   insert/delete, duplicate/out-of-order delivery, restart with stable identity,
   compaction, limits, authorization rejection, cancellation, and fuzzed patch
   decoding. Assert convergence of both document state and causal frontier.
6. **UI binding trial:** choose `local` or `replicated` at UI mount/manifest
   configuration, not in the `type` or reducer. Prove that the same UI module
   and reducer render correctly in both modes, and that remote merge does not
   rerun local effects. Decide separately whether this merits persistent
   storage, a server, or a general public API.

## Exit criteria

- Existing domain `type` declarations and `T -> T` business helpers work in
  both local and replicated state modes; no `defdoc` or CRDT-shaped duplicate
  type is required.
- A documented, versioned, schema-bound replication protocol has independent
  test vectors and no implicit dynamic `Value` ABI.
- Two independent replicas converge under the conformance corpus and rejected
  input fails without corrupting state.
- Cancellation, limits, authority, identity persistence, and compaction have
  tested semantics.
- The backend is optional: ordinary interpreter, native codegen, and existing
  UI programs retain local behaviour without a collaboration host. Enabling
  replication may change concurrent conflict outcomes, but not business source
  types or the local-action/effect contract.

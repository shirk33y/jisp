# Collaborative CRDT capability

## Decision

CRDT is a future collaboration capability, not a core Jisp collection type and
not part of the native ABI. Start only after native conformance is complete and
`jisp-wire/1` has a versioned unary runner with value limits and cancellation.

The initial target is one offline-capable shared UI document. It is not a
general distributed database, a shared evaluator, or automatic replication of
ordinary `list`/`obj`/`map` values.

## Boundary

- Jisp values remain immutable, typed data.
- A collaboration document is host-owned mutable state behind an explicit
  capability, provisionally `collab.doc<T>`.
- Jisp receives typed snapshots and submits typed operations; it never receives
  a clock, replica handle, socket, or arbitrary remote object as ordinary data.
- A remote patch enters the UI/runtime as an explicit scheduled input. Reducers
  stay deterministic for a recorded sequence of local actions and delivered
  collaboration events.
- The collaboration layer serializes a declared document schema only. Reject
  functions, closures, UI values, resources, host handles, and unsupported
  numeric/float values at the boundary.

## Required design before code

Write `docs/research/COLLAB_CRDT.md` that fixes:

1. document schemas and exact value mapping, including bigints, variants,
   results, bytes, and float edge cases;
2. operation vocabulary and conflict rules per type: register/object field,
   ordered sequence, text, and deletion;
3. replica identity persistence, causal context, offline queue, reconnect,
   compaction, retention, and snapshot recovery;
4. capability authority, document access control, authentication, quotas,
   maximum document/patch sizes, operation rate, nesting depth, and malformed
   patch handling;
5. `jisp-wire/1` envelopes for snapshot, patch, acknowledgement, rejection,
   cancellation, and protocol/schema-version negotiation; and
6. observability and errors: stable public error codes, correlation IDs, and
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
   ship a bounded unary `jisp-wire/1` runner. No CRDT code before both gates.
2. **Paper design:** publish the design above plus a schema and binary/text
   fixture corpus. Include two-replica merge examples and intentionally invalid
   patches.
3. **Narrow prototype:** implement one document type with object fields and
   text/list edits, two local replicas, in-memory transport, explicit sync, and
   snapshot restore. Keep it outside the language core and native codegen.
4. **Conformance hardening:** test offline divergence/reconnect, concurrent
   insert/delete, duplicate/out-of-order delivery, restart with stable identity,
   compaction, limits, authorization rejection, cancellation, and fuzzed patch
   decoding. Assert convergence of both document state and causal frontier.
5. **UI capability trial:** expose the document only through a declared UI
   capability. Prove that replaying the same delivered events produces the same
   JUIR state/tree. Decide separately whether this merits persistent storage,
   a server, or a general public API.

## Exit criteria

- A documented, versioned, schema-bound collaboration protocol has independent
  test vectors and no implicit dynamic `Value` ABI.
- Two independent replicas converge under the conformance corpus and rejected
  input fails without corrupting state.
- Cancellation, limits, authority, identity persistence, and compaction have
  tested semantics.
- The capability is optional: ordinary interpreter, native codegen, and
  existing UI programs retain their behaviour without a collaboration host.

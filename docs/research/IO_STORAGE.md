# IO, storage, and host capabilities

> Research and design direction, 2026-07-15. This document does **not** add a
> public prelude API, an FFI ABI, or a storage-file compatibility promise.
> Those changes need separate implementation designs and tests.

## Decision summary

Jisp should keep ordinary code pure and express I/O as a declared, typed plan
that a runner executes through named host capabilities. The public namespace
for filesystem operations should eventually be `fs`; `io` should own generic
effect composition and execution, rather than nesting filesystem calls under
`io.fs`.

The first filesystem implementation should be an in-memory, deterministic
`memfs`. A persistent, single-file Rust implementation may use `redb`, but its
file must be explicitly identified as a Rust-specific backend, not initially
promised as a portable Jisp image format. SQLite remains the interoperability
option; a custom JIFS-style format is deferred until it has requirements that a
database backend cannot meet.

```text
pure Jisp module
       |
       v
typed `io.task` plan + named capability request
       |
       v
runner / host adapter
       |
       +-- `fs.mem`                 default deterministic filesystem
       +-- `fs.image.redb`          optional Rust-only durable backend
       +-- host-mounted filesystem  explicit permission, never ambient access
```

This follows the existing UI-effects rule: Jisp produces declarative work;
hosts execute it. A view or macro does not perform host work, and Jisp source
does not obtain arbitrary Node, Python, or Rust globals. See
[UI effects](../UI_EFFECTS.md) and [the multi-host report](MAL.md).

## Scope and terminology

| Term | Meaning in this proposal | Not a claim |
| --- | --- | --- |
| `memfs` | Per-run, in-memory filesystem with paths and byte contents. | A durable database or a host directory. |
| `imagefs` | A filesystem handle whose durable state is stored by one selected backend. | A universally readable archive format. |
| `redb image` | An image implemented as tables in a `.redb` file. | A JIFS or a format Node/Python can open directly. |
| host filesystem | A directory/root granted by a runner. | Unrestricted process access to the machine filesystem. |
| capability | A versioned name plus request/response schema supplied by a host. | A raw foreign object, callback, or global variable. |

The names and signatures below are sketches. They exist to establish
boundaries, not to reserve syntax before the relevant Core IR, type, runtime,
evaluator, native, and test work exists.

## Why `fs` and `io` are separate

`fs` says *what resource is being operated on*; `io` says *how declared work is
sequenced, combined, cancelled, or run*. Keeping that distinction avoids both a
flat global prelude and an ever-growing `io.fs.*` hierarchy.

```lisp
; Illustrative only; these are not implemented functions.
(io.run
  (io.and-then
    (fs.read fs.default "/config.json")
    (fn (bytes)
      (fs.write fs.default "/copy.json" bytes))))
```

The eventual boundary should resemble:

```text
io.pure / io.fail / io.map / io.and-then / io.all / io.run
fs.read / fs.write / fs.list / fs.mkdir / fs.remove / fs.rename
fs.mem.new / fs.image.redb.open / fs.host.mount
```

An `fs` handle is an opaque local resource. It must not be serialised through
JSON Schema, a process protocol, generated TypeScript declarations, or Python
models. Cross-host messages contain only closed data, task requests, and task
results.

## Baseline: `memfs`

The default must be simpler than a database:

```text
normalized path -> file bytes / directory metadata
```

An ordered map such as a Rust `BTreeMap` is sufficient for the first semantic
implementation. It provides deterministic listing and straightforward tests
without an async runtime, a disk dependency, or a native binding.

Required baseline properties:

- a fresh instance per run or test unless a caller explicitly shares it;
- normalized, relative paths; reject traversal outside the virtual root;
- byte-based read/write API first; text codecs are a separate layer;
- deterministic directory order and well-defined errors;
- no ambient access to the current working directory;
- a quota/maximum-input policy before untrusted execution is supported.

`memfs` is the semantic reference for filesystem behavior. Persistent backends
must match this observable contract; they do not define it.

## Persistent images and `redb`

`redb` is a pure-Rust embedded ACID key-value store with copy-on-write B+trees,
zero-copy reads, multiple readers, and one writer. It is attractive because a
single database file can make a filesystem operation one durable transaction.
It is not a replacement for `memfs`: it brings a database engine, a single
writer constraint, compaction/reclamation concerns, and a backend-specific file
format.

A practical `redb` layout is:

```text
meta       image format revision, root inode, next inode, quota
nodes      inode -> kind, mode, size, timestamps
dirents    (parent inode, UTF-8 name) -> child inode
blocks     (inode, block number) -> bytes
xattrs     (inode, name) -> bytes                 [future]
```

Store file contents in fixed-size blocks rather than as one `path -> bytes`
value. Updating the middle of a large file then changes only its affected
blocks and metadata. The initial block size is an implementation benchmark
choice, not a language rule.

`write`, `rename`, `mkdir`, `remove`, and `truncate` must update all affected
tables in one write transaction. A successful transaction is the only visible
state transition. A process-local `fs` operation should not expose `redb`
tables, savepoints, or access guards in the Jisp API.

### What this buys

- one durable file for a Rust-native image;
- atomic metadata-plus-data updates and crash recovery delegated to a storage
  engine rather than reimplemented as a pager and journal;
- ordered range scans suitable for directories and block ranges;
- later snapshots/deduplication can be added behind the same `fs` handle.

### What it does not buy

- direct Node.js or Python access to the image file;
- POSIX/FUSE semantics, multi-process write scalability, encryption, or a
  stable external archive specification;
- a guarantee that a future Jisp image backend uses the same on-disk format;
- cheap whole-file rewrites or unlimited concurrent writers.

Use the currently supported `redb` major and pin it. Its release history has
included recently fixed data-loss and corruption cases, so a backend must carry
crash/reopen tests, property tests for filesystem invariants, and a documented
migration/backup policy before it is offered as durable application state.

## Prior art: `mnem` and Snix

[`mnem`](https://docs.rs/mnem-core/latest/mnem_core/) is not a filesystem. It
is a content-addressed, versioned substrate for agent memory: canonical object
encoding, CIDs, operation heads, Prolly trees, commits, graph objects, and
retrieval. Its core cleanly separates these semantics from storage through
`Blockstore` and `OpHeadsStore` traits. Its `redb` backend stores
`CID -> object bytes` and operation heads in one database file.

This is strong architectural prior art for a pure core plus optional storage
adapters. It is not the right dependency or data model for baseline `memfs` or
mutable `imagefs`: adopting it would make CIDs, DAG-CBOR, operation logs, and
commit graphs part of a simple filesystem problem. A future content-addressed
snapshot/sync mode can borrow its design or integrate through a separate
`agent.memory` capability.

[`Snix`](https://snix.dev/docs/reference/snix-castore-api/) provides another
useful split. It can expose storage through FUSE/virtiofs while using `redb`
for durable directory/path metadata and a separate blob service for contents.
That pattern is evidence that `redb` is a good index and metadata store, but it
does not establish a ready-made all-in-one mutable filesystem implementation.

## Storage choice matrix

| Backend | Rust-only | One primary file | Native Node/Python readability | Recommendation |
| --- | --- | --- | --- | --- |
| In-memory map | yes | no | n/a | Default `memfs`. |
| `redb` | yes | yes | no | Optional Rust-native image/KV backend. |
| SQLite via `rusqlite` | no; SQLite is C | logically yes, but journals/WAL may be sidecars | yes | Interoperable database backend. |
| Turso Database | yes | SQLite-compatible goal | intended multi-language support | Watch; it is pre-1.0. |
| `fjall` | yes | no; LSM directory | no | Optional write-heavy KV, not `imagefs`. |
| RocksDB | no; C++ | no; directory | broad bindings | External high-scale option, not stdlib. |
| `sled` | yes | implementation-specific | no | Do not use for a durable Jisp contract while beta/format-migration warnings remain. |
| Custom JIFS | depends on Jisp | yes | only after independent implementations | Defer until its requirements are explicit. |

SQLite is often the correct answer when the same file must be inspected or
modified by the target ecosystem. It is not a strict "only one OS file during a
write" answer: rollback-journal and WAL modes use auxiliary files. A custom
JIFS should be justified only by requirements such as a strict single-file
commit protocol, a portable public image specification, content-addressed
snapshots, or a security model unavailable from existing engines.

## Code generation and foreign hosts

Jisp source should not compile to imports such as `node:fs/promises`, Python
`pathlib`, or Rust `std::fs` directly. That would make a portable module's
meaning depend on the target environment and bypass permissions.

Instead, code generation targets a small Jisp capability IDL:

```text
typed Jisp module + capability IDL
       -> target program
       -> generated host adapter
       -> target-native I/O library
```

Examples of adapters:

| Target | Adapter implementation | Boundary rule |
| --- | --- | --- |
| Rust | `jisp-host` trait; a mounted root can use a capability-oriented directory API. | Generated code receives a host parameter, never ambient `std::fs`. |
| Node.js | Adapter wraps `node:fs/promises`. | Root, quotas, abort signal, and error mapping are explicit. |
| Python | Adapter wraps `pathlib`/`io` or executes blocking work behind its runtime policy. | Do not expose arbitrary Python objects. |
| Wasm | WIT/component or browser-specific adapter. | Only capabilities available to that host are advertised. |

The IDL must contain versioned names, closed request/result types, errors,
limits, cancellation, and capability availability. A target that lacks a
capability reports a structured unsupported-capability error; it does not
silently substitute host behavior.

### Types and generated declarations

The inferred Jisp type graph is the source of truth. Generate JSON Schema only
for JSON-native values, and generate target declarations from the same graph:

- TypeScript: `.d.ts` plus runtime codecs; declarations alone do not validate
  incoming JSON.
- Python: dataclasses and decoders by default; Pydantic is an optional adapter
  when validation/coercion policy is explicitly selected.
- Rust: concrete structs/enums and typed host traits.

Do not generate an `any`/`unknown` escape hatch for values whose meaning needs a
Jisp runtime. Functions, closures, streams, and filesystem handles remain
local resources. JavaScript must preserve Jisp `int` exactly (for example with
`BigInt` or a tagged wire representation), since JS `Number` cannot represent
all `i64` values.

## Capability contract

The existing UI contract has the correct basic shape for host effects:

```text
capability name + positive version + JSON-shaped request
  -> host executes or rejects
  -> JSON-shaped result or { code, message }
```

Filesystem capabilities need the extra policies below before implementation:

1. Mount/root model and path normalization.
2. Read/write/list size caps, file-count quota, and resource cleanup.
3. Error taxonomy: not-found, already-exists, denied, invalid-path, conflict,
   quota, unavailable, cancelled, and host-failure.
4. Cancellation and timeout semantics. A cancellation does not imply that an
   already-committed durable operation rolls back.
5. Atomicity rules for a single operation and any future transaction API.
6. Deterministic test host (`memfs`) and fault-injection/crash test host.

Host handles and native file descriptors never cross this boundary. The host
owns them; Jisp receives an opaque local handle only while the invocation is
alive.

## Staged implementation plan

1. **Design `io.task` and the capability IDL.** Define types, errors, resource
   lifetime, cancellation, runner ownership, and the relation to the current
   `io.println` convenience builtin.
2. **Implement `memfs` first.** Add one shared runtime model, evaluator host,
   prelude schemes, tests, and documentation in the normal stdlib workflow.
3. **Add a Rust host adapter.** Grant only an explicit root/mount; do not
   create a universal `std.fs` module.
4. **Prototype `fs.image.redb`.** Keep it feature-gated and labelled
   Rust-native. Benchmark realistic mixes of small files, block rewrites,
   directory scans, deletes, and recovery.
5. **Choose a public image-format promise only with evidence.** If cross-host
   file access is the goal, choose SQLite or specify JIFS independently. If it
   is not, retain `redb` as an implementation detail.
6. **Generate selected host adapters and types.** Start with the canonical
   runner/process protocol; add Node/Python native bindings only after their
   FFI/distribution contracts are written.

## Non-goals and guardrails

- Do not add `fs.*`, `io.task`, a global host object, or an FFI ABI merely by
  documenting this proposal.
- Do not make `redb` a mandatory dependency of every Jisp execution.
- Do not call a `.redb` database a portable Jisp file format.
- Do not expose arbitrary Rust crates, Node modules, or Python imports to
  Jisp source.
- Do not represent external values with `jisp_eval::Value`,
  `serde_json::Value`, or `Box<dyn Any>` in the compiled ABI.
- Do not store unbounded file payloads in one KV value; use chunking or an
  explicit blob layer.

## Sources

External facts in this note were checked on 2026-07-15:

- [redb README and benchmark](https://github.com/cberner/redb)
- [redb Cargo metadata](https://docs.rs/crate/redb/latest/source/Cargo.toml)
- [redb changelog](https://docs.rs/crate/redb/latest/source/CHANGELOG.md)
- [Fjall documentation](https://docs.rs/fjall/latest/fjall/)
- [Turso Database README and FAQ](https://github.com/tursodatabase/turso)
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html) and
  [WAL documentation](https://www.sqlite.org/wal.html)
- [sled known issues](https://github.com/spacejam/sled)
- [mnem core](https://docs.rs/mnem-core/latest/mnem_core/) and
  [mnem redb backend](https://docs.rs/mnem-backend-redb/latest/mnem_backend_redb/)
- [Snix store model](https://snix.dev/docs/reference/snix-castore-api/)

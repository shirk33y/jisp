# UI commands, subscriptions, and ownership

> Design contract — not implemented source syntax.

Jisp UI keeps Model–View–Update: `view` is pure and `(update state action)`
calculates the next immutable state. Effects are data emitted by `update`; they
are never work performed by a view, macro, event expression, or renderer.
Synchronous `prevent-default`, `stop-propagation`, and `capture` remain event
policies, not commands.

## Reducer outcome

The future semantic result is:

```text
UpdateResult { state: Value, commands: [Command], subscriptions: [Subscription] }
```

An ordinary `state` return is shorthand for empty lists. Exact Jisp constructor
names are deferred: this document owns the protocol, not provisional syntax.

## Command identity

Every command has a stable program-provided `id`, lifecycle `owner`, versioned
`capability`, JSON-shaped schema-validated `request`, `on-ok`/`on-error` action
data, and explicit timeout/retry/replacement policy. The model normally creates
ids with a request counter; the host never supplies hidden randomness.

```json
{
  "protocol": "jisp-ui-effects/1",
  "kind": "command",
  "id": "save:42",
  "owner": { "kind": "app" },
  "capability": "storage.write",
  "request": { "key": "draft:42", "value": { "title": "Plan" } },
  "on-ok": { "tag": "Saved", "fields": [42] },
  "on-error": { "tag": "SaveFailed", "fields": [42] },
  "policy": { "timeout-ms": 5000, "replace": true }
}
```

No command contains a closure, DOM object, or raw host handle. A capability owns
its input, result, error, limits, and permission schema.

## Reconciliation and cancellation

After a reducer produces state and UI patches, the host normalizes desired
commands by `(owner, id)`, diagnoses duplicates, retains an active command only
when capability and normalized request match, cancels removed work, replaces
changed work when allowed, and starts new work.

Each start receives an opaque `generation`. Completion reaches `update` only if
`(owner, id, generation)` is still active. A late completion after cancellation
or replacement is deterministically ignored, even when an external system could
not cancel underlying work.

## Subscriptions and local ownership

A subscription is a desired long-lived event source, not a command that runs
forever. It uses the same identity/reconciliation rules and yields many actions
until removed. Initially only `app` owns resources. Future local resources use:

```text
OwnerPath = app | app / Component(template-id, key) / ...
```

Unkeyed dynamic lists cannot own local state, commands, or subscriptions. A
keyed `for` retains an instance on moves, disposes its resources once on
removal, and replaces it when component type changes. This avoids hook-order
semantics.

## Capability negotiation and testing

Applications declare required capability names and minimum versions. A missing
required capability is a diagnostic, never silent browser-only behavior.
Failures are JSON-shaped `result` data with stable codes; raw exceptions and
host objects cannot enter Jisp.

Every host supplies a deterministic fake trace:

```text
start(owner, id, generation, request)
cancel(owner, id, generation)
deliver(owner, id, generation, result)
dispose(owner)
```

Test success, failure, timeout, duplicate id, replacement, late completion,
owner disposal, keyed-item removal/reorder, and unsupported capability.

## Deferred decisions

- Public Jisp constructors and exact `UpdateResult` types.
- The first capability set and action-builder serialization for SSR/resume.
- Local-state source syntax.
- WIT definitions. WIT describes this coarse capability boundary, never DOM
  patch operations.

Until these invariants are implemented, `update` returns state only and UI
components remain effect-free. See also [UI.md](UI.md) and
[FFI_FUTURE.md](FFI_FUTURE.md).

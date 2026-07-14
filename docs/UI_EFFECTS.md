# UI commands, subscriptions, and ownership

> Design contract. `jisp-ui::effects::FakeHost` implements deterministic
> command and subscription reconciliation for host tests. `ui.result` now
> carries reducer-declared resources as data; runtime command execution remains
> intentionally unimplemented.

Jisp UI keeps Model–View–Update: `view` is pure and `(update state action)`
calculates the next immutable state. Effects are data emitted by `update`; they
are never work performed by a view, macro, event expression, or renderer.
Synchronous `prevent-default`, `stop-propagation`, and `capture` remain event
policies, not commands.

## Reducer outcome

The semantic result is:

```text
UpdateResult { state: Value, commands: [Command], subscriptions: [Subscription] }
```

An ordinary `state` return is shorthand for empty lists. The explicit source
form is deliberately narrow:

```lisp
(ui.result next-state commands subscriptions)
```

The two resource lists are nominal: only `ui.command` values may appear in
`commands`, and only `ui.subscription` values may appear in `subscriptions`.
Create those values with the canonical constructors:

```lisp
(ui.command "save:42" "storage.write" 1
  (obj "key" "draft:42" "value" (obj "title" "Plan"))
  true
  (ui.action-result "Saved" (list 42))
  (ui.action-error "SaveFailed" (list 42)))

(ui.subscription "clock" "timer.tick" 1
  (obj "every-ms" 1000)
  false
  (ui.action-result "Tick" (list))
  (ui.action-error "ClockFailed" (list)))
```

Their arguments are
`(id capability-name capability-version request replace on-ok on-error)`.
`id` and `capability-name` are nonempty strings, version is a positive `u32`,
`replace` is a boolean, and request is JSON-shaped portable data. The resulting
descriptor has exactly `kind`, `id`, `capability {name, version}`, `request`,
`replace`, `on-ok`, and `on-error`; callers cannot forge a partially valid
object through the typed `ui.result` surface. `ui.action` creates a tagged
variant template with static fields. `ui.action-result` and `ui.action-error`
append a reserved portable placeholder for the host result or `{code, message}`
error, respectively. They are data templates, never callbacks. `ui.result`
only declares work. It neither runs a capability nor lets a view register work.

Type checking connects all three `ui.app` bindings: `update` receives the init
state as its first argument and returns either that state or
`ui.update-result(state)`; `app` receives that state and must return `ui.node`.
This makes `(ui.result ...)` invalid as a component result and preserves a pure
view boundary.

## Command identity

Every command has a stable program-provided `id`, lifecycle `owner`, versioned
`capability`, JSON-shaped schema-validated `request`, `on-ok`/`on-error` action
data, and explicit timeout/retry/replacement policy. The model normally creates
ids with a request counter; the host never supplies hidden randomness.

An `id` is unique within `(kind, owner)`: a command and a subscription may use
the same text id because their lifecycle kinds are separate.

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

`FakeHost` implements `start`, `cancel`, and delivery/late-delivery
classification for commands and subscriptions. It preserves explicit
JSON-shaped success/failure deliveries, validates versioned capabilities,
rejects duplicate `(kind, owner, id)` triples atomically, preserves equal active
requests, requires explicit replacement permission, and ignores a completion
whose generation is no longer active. `reconcile_resources` validates both
desired lists before changing either one. `dispose(owner)` cancels every command
and subscription of exactly that keyed component/app owner and is idempotent.
It is an in-memory reference implementation, not a browser or native capability
provider.

Tests cover success/failure, duplicate id, replacement, late completion,
owner disposal, subscription removal, cross-kind atomic validation, and
unsupported capability. Timeout and real host cancellation remain capability
provider behavior to add when source-level commands exist.

## WIT boundary prototype

[`../wit/jisp-ui-capabilities.wit`](../wit/jisp-ui-capabilities.wit) declares
the first Component Model boundary: version negotiation plus coarse
`storage-write`, `timer-sleep`, and `navigate` operations. It intentionally
does not expose the structural UI tree, DOM nodes, CSS tokens, event objects,
or individual patch writes. Each request is typed by its named capability; the
storage payload is canonical JSON selected and validated by that capability,
not a universal evaluator `Value` ABI.

This WIT package is a source-of-truth prototype. Binding generation for two
real host languages and a component-toolchain validation gate remain M6 work;
the in-memory `FakeHost` exercises the same capability/version discipline but
is not a generated WIT binding.

## Deferred decisions

- Public Jisp constructors and exact `UpdateResult` types.
- The first capability set and action-builder serialization for SSR/resume.
- Local-state source syntax.
- Generated WIT bindings and a component-toolchain validation gate. WIT
  describes this coarse capability boundary, never DOM patch operations.

The browser Wasm session exposes the most recent declarations through
`desired_resources`; it does not execute them. A real command host must still
validate capability schemas, reconcile against `FakeHost`-equivalent lifecycle
rules, and dispatch completions back through `update`. UI components remain
effect-free. See also [UI.md](UI.md) and [FFI_FUTURE.md](FFI_FUTURE.md).

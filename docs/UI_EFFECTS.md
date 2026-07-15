# UI commands, subscriptions, and ownership

> Design contract. `jisp-ui::effects::FakeHost` implements deterministic
> command and subscription reconciliation for host tests. `ui.result` carries
> reducer-declared resources as data. Fixture-only `ui.test` can deterministically
> deliver their completion actions. The playground also provides two deliberately
> narrow browser capabilities; it is not a general browser command runtime.

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
until removed. Initially only `app` owns resources. Future local resources use a
complete, not leaf-only, instance path:

```text
OwnerPath = app | app / Component(template-id, key) / ...
```

The entire ancestry is part of the key, so two `todo-row` instances with key
`"42"` below different parent components cannot collide. Unkeyed dynamic lists
cannot own local state, commands, or subscriptions. A keyed `for` retains an
instance on moves, disposes its resources once on removal, and replaces it when
component type changes. This avoids hook-order semantics.

## Capability negotiation and testing

Applications declare required capability names and minimum versions. A missing
required capability is a diagnostic, never silent browser-only behavior.
Failures are JSON-shaped `result` data with stable codes; raw exceptions and
host objects cannot enter Jisp.

Every test host supplies a deterministic fake trace:

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
and subscription in that full keyed component/app ownership subtree and is
idempotent. Disposing an ancestor therefore also disposes its descendants, but
not a sibling with the same terminal `(template, key)`. It is an in-memory
reference implementation, not a browser or native capability provider.

Tests cover success/failure, duplicate id, replacement, late completion,
owner disposal, subscription removal, cross-kind atomic validation, and
unsupported capability. Portable `ui.test` scenarios configure a fixture host
with `(supports "name" version)`, then use `(deliver command|subscription id
result)` or `(deliver-error command|subscription id code message)` to feed a
completion action through the ordinary reducer. Timeout and real host
cancellation remain capability-provider behavior to add to browser/native
hosts.

## Playground browser provider

The playground opts in to the Wasm effect-host boundary and implements exactly
two capabilities. They are useful executable examples of the lifecycle, not a
permission model for arbitrary browser APIs:

| Capability | Resource kind | Exact request | Completion |
| --- | --- | --- | --- |
| `storage.write@1` | command | `{ "key": nonempty-string, "value": json }` | `{ "ok": { "key": string } }` after writing the JSON-encoded value to the playground page's `localStorage` |
| `timer.tick@1` | subscription | `{ "every-ms": integer, 10..86400000 }` | `{ "ok": 1 }`, `{ "ok": 2 }`, … at that interval |

Unknown fields, a wrong kind, a wrong version, or an invalid request complete
with a stable JSON-shaped error. A `localStorage` exception becomes
`host-failure`. Removing or replacing a timer clears its interval. The host
keys active work by `(kind, id)` and only delivers a completion when the Wasm
runtime still reports the same opaque generation; this is defense in depth on
top of the runtime's own stale-delivery check.

The provider deliberately does **not** expose network, navigation, arbitrary
storage reads, DOM handles, or an evaluator. Add each future capability with a
versioned schema, explicit permission and cancellation policy, deterministic
fake-host tests, and a browser integration test.

## WIT boundary prototype

[`../wit/jisp-ui-capabilities.wit`](../wit/jisp-ui-capabilities.wit) declares
the first Component Model boundary: version negotiation plus coarse
`storage-write`, `timer-sleep`, and `navigate` operations. It intentionally
does not expose the structural UI tree, DOM nodes, CSS tokens, event objects,
or individual patch writes. Each request is typed by its named capability; the
storage payload is canonical JSON selected and validated by that capability,
not a universal evaluator `Value` ABI.

`jisp-wit-check` is the local/CI conformance gate for this package. Its build
script generates independent Rust and C bindings for the exported
`jisp-ui-host` world into Cargo's `OUT_DIR`; its test verifies that both carry
the three operations and stable unsupported/permission error codes. CI also
asks the configured C compiler to syntax-check the generated C source. The
generated sources are deliberately ephemeral: WIT, not a checked-in binding,
is the source of truth. `FakeHost` exercises the same capability/version
discipline at runtime, but is not itself a generated WIT binding.

The separate `jisp-ui-capability-component` crate compiles a deterministic
`wasm32-wasip2` implementation of that exported host world. It supports only
request validation for `storage.write@1` and `timer.sleep@1`; navigation
returns `unsupported-capability`, and it intentionally does no I/O. CI builds
the component and invokes that exact artifact through two independent Component
Model hosts: the pinned Wasmtime CLI and JCO-transpiled JavaScript on Node.
[`scripts/verify-ui-capability-component-hosts.sh`](../scripts/verify-ui-capability-component-hosts.sh)
checks the advertised capability list, successful storage/timer calls, invalid
requests, and the stable unsupported-navigation error in both hosts. The script
uses temporary generated JavaScript and npm dependencies only; WIT remains the
single checked-in ABI contract.

## Deferred decisions

- Local-state source syntax.
- Concrete browser/native capability schemas, permissions, timeout policies,
  and providers beyond the two narrow playground capabilities.
- Capability serialization choices for SSR/resume beyond the current
  JSON-shaped descriptors and completion templates.
- Optional AOT lowering of stable JUIR templates into host-language UI code.
  WIT describes this coarse capability boundary, never DOM patch operations.

The browser Wasm session exposes the most recent declarations through
`desired_resources`; it does not execute them. An embedding host may opt in to
the generation-safe Wasm boundary by calling
`configure_effect_host([{name, version}])`. Once configured, each active
resource in `jisp-ui-resources/1` includes an opaque `generation`; return it to
`deliverEffect(kind, id, generation, completion)`, where `completion` is
either `{"ok": value}` or
`{"error": {"code": "permission-denied", "message": "..."}}`. Wasm
checks that the resource is still current, expands the source-declared action
template, runs `update`, and atomically reconciles the next declarations.
Duplicate configuration is rejected so active generations cannot be reset.

The Wasm boundary itself performs no I/O. The playground attaches the limited
`storage.write@1` and `timer.tick@1` provider described above; its
fixture-only `run_tests` entry point still executes the deterministic `ui.test`
simulation rather than real browser I/O. Any other command host must validate
its capability schemas and implement cancellation against the same lifecycle
rules. UI components remain effect-free. See also [UI.md](UI.md) and
[FFI_FUTURE.md](FFI_FUTURE.md).

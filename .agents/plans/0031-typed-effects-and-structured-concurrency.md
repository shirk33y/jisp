# Plan: typed effects and structured concurrency

**Status:** proposed. This is a language/runtime design followed by staged
implementation. It generalizes the lifecycle contract already proven for UI
commands; it does not add ambient I/O, threads, arbitrary futures, or a second
language runtime.

## Goal

Add a portable, typed task model that lets Jisp describe asynchronous host work
and compose it with deterministic structured concurrency:

```text
pure Jisp code
  -> typed task plan
  -> local deterministic scheduler
  -> versioned capability leaf requests
  -> browser / native / process host
  -> typed completion
  -> local continuation or reducer action
```

The same program must have one observable task/cancellation/failure contract in
the interpreter, portable tests, Wasm, native code, and foreign hosts. A host
may execute leaves differently, but it may not redefine task composition,
ownership, tie-breaking, or error delivery.

## Existing foundation

This plan extends rather than replaces the M4 work in
[`0019-compiled-portable-ui-runtime.md`](0019-compiled-portable-ui-runtime.md)
and the contract in
[`../../docs/UI_EFFECTS.md`](../../docs/UI_EFFECTS.md):

- reducers declare `ui.command` and `ui.subscription` values instead of
  performing work;
- resources have `(kind, owner, id, generation)` identity;
- the complete desired resource set is validated before reconciliation;
- removal, replacement, and owner disposal cancel active resources;
- late completion from an obsolete generation is ignored;
- full keyed component ancestry supplies stable local ownership;
- a deterministic fake host records start, cancel, deliver, and late-deliver
  behavior; and
- capability leaves use versioned names and JSON-shaped request/result schemas.

[`../../docs/research/IO_STORAGE.md`](../../docs/research/IO_STORAGE.md) already
proposes `io.task`, `io.map`, `io.and-then`, and `io.all`, but does not yet
specify an executable scheduler, task tree, deterministic race behavior, effect
inference, or integration with the existing UI resource runtime.

The current implementation has only a `replace` flag on command/subscription
descriptors. Text mentioning timeout or retry policy is aspirational until
this plan gives those concepts a typed representation and tests.

## Four separate contracts

Do not collapse these into one vague notion of "async":

| Contract | Question | Jisp representation |
| --- | --- | --- |
| Capability effect | What external authority can a computation use? | inferred versioned capability set |
| Task composition | In what order can results become available? | typed `io.task<A, E>` plan |
| Lifecycle | Who owns work and when is it cancelled? | scheduler scope tree plus generation |
| Failure | How does an unsuccessful operation reach source code? | typed data (`result`/task error), never a raw host exception |

Concurrency is not parallelism. Hosts may run independent leaves in parallel,
but source semantics describe only starts, completions, ordering, cancellation,
and results. No program may observe the host thread count.

## What to adapt from Verse

Verse makes time flow structured: `sync`, `race`, `rush`, and `branch` constrain
concurrent work to an async context, while `spawn` is an explicit unstructured
escape hatch. Jisp should adopt the lifetime discipline, not the surface syntax
or mutable transactional runtime.

| Verse idea | Jisp adaptation | Initial decision |
| --- | --- | --- |
| `sync` | `io.all` | include |
| `race` | `io.race` with deterministic loser cancellation | include |
| `rush` | return the first result while siblings continue in the parent scope | defer; easy to leak work and rarely needed initially |
| `branch` | scoped child task owned by its parent scope | internal scheduler primitive first |
| `spawn` | detached task outliving its lexical caller | exclude from the portable baseline |
| `suspends` | task-producing/awaiting expression | infer through task/effect typing; no manual annotation initially |
| failure context rollback | atomic state/resource commit after validation | adapt only at reducer/runner commit boundaries |
| `defer` | deterministic scope finalizer | use owner/scope disposal, not arbitrary user cleanup callbacks |

Verse's current effect hierarchy also distinguishes convergence, computation,
variation, transactions, failure, and suspension. Jisp should not copy that
hierarchy before it can prove those properties. The useful first static fact is
the exact set of host capabilities a function or task may request.

## Source and type direction

### Task type

Start with a nominal public type:

```text
io.task<A, E>
```

`A` is the success value and `E` is a closed portable error value. Capability
requirements are compiler metadata attached to typed expressions and
definitions, not a source-visible third generic parameter in the first
implementation:

```text
EffectSet =
  Pure
  | Capabilities({ capability-name@minimum-version, ... })
  | Unknown
```

This avoids blocking the task runtime on general effect-row syntax. The
compiler may later expose effect rows if real programs need polymorphism over
capability sets.

Effect inference rules:

1. literals, constructors, immutable collection operations, and pure calls
   contribute no capabilities;
2. a capability leaf contributes its exact name and minimum version;
3. composition unions child effect sets;
4. a call contributes the inferred set of the callee;
5. recursion reaches a fixed point over the definition call graph;
6. a dynamically unresolved call is `Unknown`;
7. a portable public export with `Unknown` effects is rejected or must declare
   an explicit conservative capability bound before execution; and
8. effect metadata must be identical across Lisp, `ws`, JSON, and YAML source.

### Task constructors and combinators

The exact names remain provisional until Core IR/type seams are traced. The
semantic surface should cover:

```text
io.pure       : A -> io.task<A, never>
io.fail       : E -> io.task<never, E>
io.request    : capability<A, E> -> request -> io.task<A, E>
io.map        : io.task<A, E> -> (A -> B) -> io.task<B, E>
io.map-error  : io.task<A, E1> -> (E1 -> E2) -> io.task<A, E2>
io.and-then   : io.task<A, E1> -> (A -> io.task<B, E2>)
              -> io.task<B, E1 | E2>
io.all        : list<io.task<A, E>> -> io.task<list<A>, E>
io.race       : nonempty-list<io.task<A, E>> -> io.task<A, E>
io.timeout    : duration -> io.task<A, E>
              -> io.task<A, E | io.timeout-error>
```

Do not put Jisp closures into JSON/WIT/process messages. `map` and `and-then`
continuations remain in the local Jisp executor (or locally generated native
code). Only a leaf's capability name/version, closed request data, identity,
and completion cross the host boundary.

`ui.subscription` remains a long-lived stream-like resource. Do not force a
multi-delivery subscription into single-completion `io.task`; a future
`io.stream<A, E>` can share scheduler scopes and capability leaves after task
semantics are stable.

## Deterministic task semantics

The interpreter and fake scheduler are the semantic reference.

### Sequential composition

- `io.pure` completes without consulting a host.
- `io.fail` completes with typed error data.
- `io.map` runs its pure continuation exactly once after success.
- `io.and-then` creates its second child only after the first succeeds.
- failure skips success continuations and flows to the task result.
- cancellation is a terminal scheduler state, not a host exception.

### `io.all`

- all children enter the same child scope;
- results preserve source-list order, never completion order;
- success requires every child to succeed;
- the first observed failure cancels unfinished siblings;
- simultaneous failures are selected by stable child index; and
- an empty input succeeds immediately with an empty list.

### `io.race`

- input must be nonempty;
- all children start in source order in one child scope;
- the first terminal success or failure wins;
- a same-turn tie is resolved by lowest source child index;
- every unfinished loser receives cancellation exactly once;
- late loser completions are ignored by generation; and
- the parent completes only after cancellation has been issued, not after an
  uncooperative external provider confirms it.

### Timeout

`io.timeout` is defined semantically as a race against `clock.sleep@1`, but is
a named combinator so hosts and diagnostics can report it clearly.

- virtual and real hosts use the same monotonic duration unit;
- at the exact deadline, an already queued task completion wins over timeout;
- timeout cancels the child scope and returns a typed timeout error;
- cancellation does not claim to undo an external operation already committed;
  it only prevents obsolete completion from entering Jisp.

### Scheduling turns

Define a scheduler turn explicitly:

1. accept one ordered batch of leaf completions/timer advances;
2. ignore completions whose generation is not current;
3. reduce all newly unblocked pure task nodes to quiescence;
4. collect new leaf starts and cancellations in deterministic tree order;
5. expose one trace entry/batch to the host; and
6. if a UI action was produced, run one reducer turn and atomically validate
   the resulting state/resource snapshot before publishing it.

No callback may re-enter the scheduler in the middle of a turn.

## Runtime representation

Use a local task arena/tree rather than host futures:

```text
TaskId = executor-private monotonic identity

TaskNode =
  Pure(value)
  | Fail(error)
  | Leaf(capability, request, generation)
  | Map(child, continuation)
  | AndThen(child, continuation)
  | All(children)
  | Race(children)
  | Timeout(child, timer-child)

TaskState =
  Pending | Running | Succeeded(value) | Failed(error) | Cancelled

Scope =
  id + optional parent + owner + ordered children + lifecycle state
```

Task IDs, generations, continuations, and local resource handles are opaque and
never appear in user JSON. Stable source-level resource IDs remain necessary
where a reducer reconciles desired UI resources across turns.

Cancellation walks the scope tree once, marks nodes terminal, emits leaf
cancellation, disposes owned subscriptions, and rejects later completions.
Disposal is idempotent.

## Capability manifests

The compiler should expose a versioned manifest separate from a concrete task
run:

```json
{
  "protocol": "jisp-capabilities/1",
  "required": [
    { "name": "clock.sleep", "minimumVersion": 1 },
    { "name": "http.get", "minimumVersion": 1 }
  ]
}
```

A host negotiates the complete statically known set before starting a portable
entry point. A dynamic optional capability may use an explicit source
operation that returns availability as data; it must not silently fall back to
ambient host access.

Each capability owns:

- request and result/error schemas;
- version compatibility;
- permission policy;
- payload and rate limits;
- whether physical cancellation is supported;
- idempotency guidance; and
- deterministic fake-provider behavior.

Retry is not a generic boolean. A later `io.retry` combinator must require an
explicit policy (attempt limit, delay/backoff, retryable error predicate) and
must document the capability's idempotency consequences.

## UI integration

Keep the existing source API working while moving implementation underneath it:

1. lower one `ui.command` to a single capability-leaf task with the existing
   owner/id/generation and completion action templates;
2. keep `ui.subscription` on its existing multi-delivery resource path;
3. let a future typed UI command accept a composed `io.task`, while action
   mapping remains local;
4. reconcile the root task as part of the reducer's complete desired resource
   set;
5. unmount/owner disposal cancels the task scope and every descendant; and
6. reject `io.run`, task start, or capability execution from a view, macro, or
   static renderer.

State and the desired resource/task snapshot become visible together only
after both decode and reconciliation validation succeed. A failed validation
must retain the previous state, UI tree, resources, task generations, and
trace.

## Portable testing model

Add a deterministic scheduler host before real providers.

Fixture operations should be able to:

```text
support capability@version
start entry point / dispatch action
deliver task leaf id with success/error
advance virtual time by duration
cancel owner/scope
assert task state/result
assert ordered scheduler trace
```

Required portable cases:

1. pure/map/and-then success and typed failure;
2. `all` preserves input order under reverse completion;
3. `all` fail-fast cancels unfinished siblings exactly once;
4. `race` winner and deterministic same-turn tie;
5. race loser delivers late and is ignored;
6. timeout before completion, completion before timeout, and exact-deadline
   tie;
7. parent cancellation recursively disposes descendants;
8. sibling scopes with identical leaf IDs never collide;
9. duplicate resource identity rejects the entire reducer commit;
10. unsupported capability fails before any leaf starts;
11. continuation creates a second capability leaf only after the first result;
12. cancellation of a physically uninterruptible provider still prevents
    delivery;
13. capability effect inference across direct calls, higher-order calls,
    recursion, and unknown dynamic calls;
14. Lisp, `ws`, JSON, and YAML fixtures normalize to the same typed task IR and
    scheduler trace;
15. interpreter and supported native/Wasm execution produce the same result
    and trace; and
16. randomized completion ordering never changes an outcome except where
    documented race order makes completion time observable.

Use a virtual monotonic clock; tests must never sleep in wall-clock time.

## Milestones

### M0 — freeze semantics and repair documentation

- Add the task/effect terminology to architecture and stability docs.
- Reconcile `UI_EFFECTS.md` claims with actual `replace`-only descriptors.
- Specify scheduler turns, cancellation, failure, tie-breaking, and atomic
  commit behavior.
- Add failing portable fixtures for the desired task surface before providers.

**Exit criteria:** every observable rule above has one named test or an explicit
deferred marker; docs no longer imply implemented timeout/retry fields.

### M1 — typed task IR and deterministic single-leaf scheduler

- Add nominal `io.task<A, E>` to shared types/Core IR.
- Implement `Pure`, `Fail`, and one typed capability leaf.
- Add executor-private task/scope identity and generation.
- Implement deterministic start, deliver, cancel, and late-deliver traces.
- Keep the first host entirely in memory.

**Exit criteria:** a portable task can complete successfully, fail, cancel, and
reject a late completion identically in all source syntaxes.

### M2 — sequential composition

- Implement `map`, `map-error`, and `and-then`.
- Keep continuations local and prove no closure enters a host payload.
- Add capability-set inference through definitions and recursive call graphs.
- Emit the first versioned required-capability manifest.

**Exit criteria:** a two-capability pipeline runs deterministically and a host
can reject its missing capability before starting either leaf.

### M3 — structured concurrency and virtual time

- Implement task scopes, `all`, `race`, and `timeout`.
- Add recursive idempotent cancellation and stable same-turn ordering.
- Add virtual clock and property/model tests over completion permutations.
- Measure and cap task depth, child count, pending leaves, and trace size.

**Exit criteria:** all/race/timeout and nested cancellation pass the portable
matrix without wall-clock sleeps or detached work.

### M4 — UI command bridge

- Lower current `ui.command` through the common leaf/scope runtime.
- Preserve the public descriptor and existing portable/browser tests.
- Make reducer state/resource/task publication atomic on validation failure.
- Prove keyed owner moves retain work and unmount cancels descendants once.

**Exit criteria:** existing UI examples remain source-compatible and use one
common generation/cancellation implementation.

### M5 — host and ABI conformance

- Express one small task capability interface through the appropriate
  WIT/process/native boundary.
- Add independent Wasm and native/process host conformance runs.
- Validate cancellation, unsupported capability, payload limits, and stable
  error codes at the real boundary.
- Add one real timer provider only after the virtual scheduler is complete.

**Exit criteria:** at least two host implementations pass the same task
protocol fixtures, including race cancellation and late completion.

### M6 — ergonomics only after semantics

- Evaluate `io.retry`, scoped branch syntax, heterogeneous `all`, and an
  `io.stream` abstraction.
- Consider source-visible effect rows only if inferred manifest metadata cannot
  express real library APIs.
- Evaluate detached work only with an explicit application owner, shutdown
  policy, quotas, and observability.

**Exit criteria:** additions remain reducible to the established task/scope
semantics and do not add a parallel scheduler contract.

## Limits and security

Before enabling untrusted task programs, every runner must enforce:

- maximum task nodes, depth, children per combinator, and active leaves;
- maximum request/result bytes and trace length;
- virtual or real duration limits;
- capability allowlist and version negotiation;
- cancellation on runner shutdown;
- no ambient filesystem, network, clock, randomness, environment, or FFI;
- panic containment at embedding boundaries; and
- deterministic diagnostics for limit exhaustion.

These limits are runner policy, but exceeding one must become stable typed host
failure data rather than a hang or raw process exception.

## Explicit non-goals

- OS threads or shared-memory primitives in Jisp source.
- Observable parallel execution or host thread count.
- General software transactional memory.
- Verse-style mutable failure contexts.
- Arbitrary promises/futures supplied by JavaScript, Python, or Rust.
- Serializing closures, task arenas, or host handles through JSON/WIT.
- Treating subscriptions as tasks before stream semantics exist.
- Detached `spawn` in the portable baseline.
- Exactly-once external effects; cancellation cannot undo committed I/O.
- Network/filesystem providers before their individual capability designs.

## Done when

- Jisp has one typed, renderer-independent task representation.
- `map`, `and-then`, `all`, `race`, and `timeout` have deterministic portable
  semantics.
- Every task belongs to a scope and every scope has explicit disposal.
- Losing/cancelled task generations can never deliver into current state.
- Required host capabilities are inferred and negotiated before execution.
- reducers publish state and desired work atomically after validation.
- UI commands reuse the common runtime without breaking existing source.
- virtual-time portable tests and at least two real host boundaries agree.
- no parser, syntax frontend, JavaScript glue, or individual provider owns task
  semantics.

## Sources

External behavior was checked on 2026-07-24:

- [Verse structured concurrency](https://dev.epicgames.com/documentation/fortnite/structured-concurrency)
- [Verse time flow and concurrency](https://dev.epicgames.com/documentation/fortnite/time-flow)
- [Verse effect and concurrency terminology](https://dev.epicgames.com/documentation/en-us/fortnite/verse-glossary)
- [Verse race, rush, and sync examples](https://dev.epicgames.com/documentation/fortnite/debugging-and-troubleshooting-in-verse)
- [Verse failure contexts and rollback](https://dev.epicgames.com/documentation/en-us/fortnite/basics-of-writing-code-9-failure-and-control-flow-in-verse)
- [The Verse Calculus](https://simon.peytonjones.org/verse-calculus/), which is
  relevant to deterministic functional-logic semantics but does not define the
  UEFN structured-concurrency runtime adopted here.

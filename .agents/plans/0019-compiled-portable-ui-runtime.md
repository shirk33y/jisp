# Compiled portable UI runtime

## Status

Design and staged implementation plan. This document deliberately does not
change the current UI language contract. It defines the work required to move
from the playground's correct-but-full-render host to a portable, incremental
UI runtime.

Progress: M0 and M1 are complete on `master`. M2 now has a typed JUIR compiler
and structural executor enabled by the browser Wasm crate's default `juir`
feature. It evaluates Jisp expressions through the canonical evaluator and
materializes the existing structural-tree contract before the DOM reconciler.
Event descriptors retain explicit synchronous `prevent-default`,
`stop-propagation`, and `capture` policy for the browser host. The playground
now mounts the versioned `jisp-ui-mount-plan/1` static skeleton directly through
DOM APIs, while Jisp/Wasm supplies values for every dynamic slot/block; the
structural tree remains the conformance oracle and recovery format.
The compiled plan now also carries a stable `jisp-ui-source-map/1` manifest for
template nodes, dynamic slots, event descriptors, blocks, and source
expressions. It is exposed through Wasm for diagnostics/tooling without making
browser DOM paths or JavaScript parsing part of UI semantics.
M3 now conservatively reuses JUIR scalar slots, unaffected element subtrees,
and whole `for` blocks. Proven
parameter field paths are compared with immutable reducer changes; opaque,
local, and module-level expressions are explicitly `Unknown` and therefore
cannot be skipped. The browser Wasm executor retains the previous structural
value, so this path is exercised by the playground rather than only by unit
tests. Component calls with unaffected, statically tracked inputs are skipped
as whole subtrees. Per-render metrics now explain every component-call decision
as `reused` or `executed` with no-cache, unknown-change, opaque-dependency, or
changed-input reason. The playground surfaces per-render reuse/skip diagnostics
from the Wasm executor plus DOM mount/replacement/write counters from the
browser host. Browser event updates now receive a batched structural patch
protocol rather than a complete tree; a full snapshot is reserved for initial
mount and recovery. Keyed `for` blocks also retain rows whose immutable item
value is unchanged when a collection changes or reorders; a changed external
dependency still conservatively rerenders every affected row. The cache is
internal to the JUIR executor and does not alter the structural-tree contract.
It also retains prior input values at component boundaries: a component whose
complete plan reads only its own inputs can now be reused when a conservatively
opaque caller expression evaluates to an equal immutable value. A component
with any opaque/module-level dependency remains re-executable.
Portable differential scenarios now cover keyed `for` rows, dynamic properties
and classes, conditional branches, and nested component calls in all four
source syntaxes; they compare the reference structural value to JUIR after
each reducer action, including flattened dynamic child lists. A changed `if`
condition explicitly discards the prior branch output before execution, so an
otherwise static replacement branch cannot inherit stale structure.
M4's pre-implementation ownership and capability contract is documented in
[`docs/UI_EFFECTS.md`](../../docs/UI_EFFECTS.md): effects remain reducer data,
carry stable owner/id/generation identity, and require deterministic fake-host
tests before a source-level command API is introduced. `jisp-ui::effects`
now implements a deterministic command/subscription fake host: it reconciles
the whole desired resource set atomically by kind/owner/id, validates versioned
capabilities, cancels/replaces work, preserves JSON-shaped success/failure
deliveries, ignores late generations, and disposes every resource owned by a
keyed component instance exactly once. The source-level
`(ui.result state commands subscriptions)` constructor now carries only
portable data; type validation ties `init`, `update`, and `app` together and
rejects effect values from a view. Source constructors `ui.command` and
`ui.subscription` produce exact versioned descriptors and give the two resource
lists distinct nominal types. Portable `ui.test` scenarios configure a
deterministic fake-host capability set and deliver current command/subscription
successes or stable errors through declared action templates and the ordinary
reducer. The browser Wasm session exposes declarations without executing them;
its fixture-only test entry point runs that deterministic simulation. Real host
execution beyond the narrow providers and local commands/subscriptions remain
subsequent M4 work.
`ui.local` now provides the first opt-in component state boundary for stable
static component paths: it binds `(state set-state)` lexically, routes an
event-scoped setter directly through the JUIR executor rather than the app
reducer, scopes sibling instances independently, and drops the cell on
unmount. An enclosing keyed `for` now derives the component instance path from
its evaluated root key, so a reordered row retains its cell; an unkeyed dynamic
row resets on a collection change rather than bleeding state by index.
The fake host's component owner identity is now a complete ordered ancestry,
not merely the terminal `(template, key)`, so equal keyed descendants beneath
different parents cannot collide before local ownership is exposed in source.
The JUIR executor now carries that full opaque owner identity alongside every
mounted `ui.local` cell and passes it through the browser event boundary; a
keyed row reused from the render cache retains both its local state and owner,
rather than displaying stale output and then losing the scope on its next
event. This is the ownership foundation for local commands/subscriptions;
their source syntax and completion routing remain subsequent M4 work.
`PlaygroundSession` now also exposes an opt-in generation-safe effect-host
boundary: an embedding host configures immutable versioned capabilities, reads
active resource generations from `jisp-ui-resources/1`, and returns an
`{ok}`/`{error}` completion to Wasm. Wasm verifies the live generation,
materializes the source-declared action template, runs the normal reducer, and
reconciles the next desired set. The playground now attaches a deliberately
narrow browser provider for `storage.write@1` commands and `timer.tick@1`
subscriptions. It validates their exact portable schemas, cancels
removed/replaced timers, and returns generation-safe completions through this
embedding protocol. Browser I/O beyond those two demonstrator capabilities and
local commands/subscriptions remain subsequent M4 work.
M6 now has a first WIT package at
[`wit/jisp-ui-capabilities.wit`](../../wit/jisp-ui-capabilities.wit), limited to
coarse versioned storage/timer/navigation capabilities. The workspace's
`jisp-wit-check` build gate generates Rust and C bindings for the exported
host world from that single WIT source on every normal test build and asserts
their operations plus the stable unsupported/permission error diagnostics.
CI syntax-checks the generated C source as well. The generated sources live
only in `OUT_DIR`, so there is no hand-maintained parallel ABI.
`jisp-ui-capability-component` additionally compiles a
deterministic implementation of that host world to a real `wasm32-wasip2`
Component. CI installs the target, builds the artifact, then invokes it through
both a pinned Wasmtime CLI and JCO-transpiled JavaScript on Node. The shared
smoke test validates `storage.write@1` and `timer.sleep@1` requests, their
stable invalid-request diagnostics, and an explicitly unsupported navigation
request without claiming browser/native I/O.
M5 has a versioned SSR payload (`jisp-ui-ssr/1`) containing escaped HTML,
serializable state, and the structural tree. Its generated `data-jisp-path` and
`data-jisp-key` markers provide stable element anchors without allowing source
attributes to spoof them. The playground hydrates a matching existing tree by
attaching paths/listeners in place and preserves pre-hydration form
`value`/`checked` state until an actual reducer change writes the property.
Production server delivery and block-level anchors remain subsequent M5 work.
`jisp-ui::native` now supplies an in-memory semantic-widget prototype with a
deliberately small registry and explicit unsupported-element/metadata
diagnostics; it is deliberately independent of DOM, CSS, and any GUI toolkit.

## Goal

Make Jisp a practical portable language for declarative interactive UI:

- a program has one source form in Lisp, JSON, YAML, or WS;
- UI state transitions are deterministic and testable without a browser;
- common updates mutate only the affected host nodes, rather than replacing the
  entire root or diffing a freshly allocated general-purpose virtual tree;
- browser DOM, static HTML/SSR, tests, and later native widgets use the same UI
  semantics;
- the implementation permits future SSR, hydration, lazy loading, and native
  hosts without making JavaScript closures, DOM objects, or a single platform's
  state model part of Jisp.

The target is **not** React syntax, React hooks, or a generic virtual DOM. It
is a compiled update plan: Elm-style application semantics, Svelte-like static
templates, Compose-like component skipping, and a fine-grained dependency graph
hidden behind an immutable reducer API.

## Current baseline

The current language and playground establish useful seams:

- `(ui.app init update app)` declares immutable initial state, a reducer, and a
  `(state) -> ui.node` app component.
- Explicit elements and directives lower to renderer-neutral structural nodes.
- Events are either delayed `emit` actions or explicit handlers; the browser
  host evaluates them through `jisp-wasm` rather than reimplementing Jisp in
  JavaScript.
- `key` is preserved in the lowered tree, but does not yet preserve host
  instances.
- The browser playground evaluates `app` after every event and replaces the
  preview root. This is the reference behavior to preserve, not the final
  performance model.

The existing static renderer and the current full-render playground remain the
semantic oracle while incremental execution is introduced.

## Architectural decision

Introduce a versioned, renderer-neutral **Jisp UI IR** (`JUIR`) and compile UI
components into it. A JUIR module describes static templates, dynamic slots,
control-flow blocks, component boundaries, and serializable event messages.
It does not contain host objects, JavaScript closures, or arbitrary FFI values.

```text
source readers
    -> shared AST -> macro expansion -> typed Core IR
    -> UI validation and UI lowering -> JUIR
    -> host executor
         DOM patches | HTML/SSR | test tree | native widgets
```

JUIR is an implementation protocol, not a new user-facing source syntax.
Lowering remains syntax-independent and belongs after typed Core IR rather than
in any parser crate.

## Alternatives evaluated

### Compiled retained UI is the default

Slint is the closest native-UI precedent: it uses a declarative language whose
expressions are pure, compiles ahead of time, and targets several host-language
backends. Its compiler can inline or remove constant/unchanged properties. This
supports Jisp's core direction: a portable declarative language should compile
to host operations, rather than expose a host framework's object model.

Jisp differs in one important way. Slint compiles `.slint` files into the target
host language, whereas Jisp must also execute JSON-shaped programs loaded at
runtime and retain one semantic implementation across hosts. Therefore JUIR is
a portable execution plan first; AOT direct-host code generation is an optional
optimization layered on top of that plan.

### Cross-framework transpilation is not our runtime model

Mitosis shows that one component source can be translated into React, Vue,
Svelte, Qwik, Solid, and React Native implementations. It is useful evidence
that a renderer-neutral component IR and target-specific code generation are
valuable. It should not define Jisp's runtime: Mitosis inherits JSX, JavaScript
closures, hooks, and the semantic differences of each target framework.

Jisp should compile to its own host protocol. A React or native-framework output
can be an optional backend, but must not become the meaning of a Jisp component.

### Retained versus immediate hosts

The primary Jisp target is retained declarative UI: mounted blocks have stable
identity and can preserve focus, selection, scrolling, accessibility state, and
native-widget resources. Immediate-mode GUI libraries such as egui reissue UI
instructions every frame and retain only selected interaction/layout state.
They may be a useful adapter for developer tools, canvases, or embedded
debugging panels, but are not the canonical executor for application UI.

This distinction is host-level. An immediate-mode host can interpret a JUIR view
on every frame, while DOM/native retained hosts use mount/update/dispose. It
must not force immediate-mode limitations into portable Jisp semantics.

### Dynamic dependency graphs are an internal primitive

Jane Street's `Incremental`/`Incr_dom` demonstrates a relevant hybrid: an
incremental graph recomputes only descendants of changed inputs and a `cutoff`
stops propagation if recomputation yields an equal value. Angular Signals and
Swift Observation likewise track reads to determine consumers of changed state.

This confirms the hybrid JUIR strategy: conservative static paths plus
runtime-tracked reads and equality cutoffs. It does **not** require users to
manually construct a signal graph or replace reducer-based application state.

### A portable mutation protocol is viable, but not sufficient by itself

Dioxus separates renderer work from its core by emitting mutations and accepting
user events. This validates the mount/update/event boundary for multiple
renderers. Dioxus still maintains a virtual DOM and performs its diffing there;
Jisp should use a compiler-generated JUIR plan to produce equivalent host
operations without a general VDOM in the common path.

### Interface portability and UI portability are separate layers

The WebAssembly Component Model's WIT describes typed, language-neutral
interfaces and worlds. It is promising for future Jisp capabilities, commands,
and host/guest bindings across Rust, JavaScript, Python, and other languages.
It does not specify DOM manipulation, layout, reconciliation, or UI semantics.

JUIR must therefore be specified independently of WIT. Once stable, selected
JUIR execution/capability interfaces may be expressed in WIT for non-browser
hosts. The browser still needs a small DOM bridge because Wasm has no direct DOM
API.

### Why not a general runtime VDOM?

A structural tree is still valuable as an oracle, a test renderer, and a
fallback for deliberately dynamic UI. It should not be allocated and diffed on
every ordinary interaction. In the hot path, a compiled component mounts its
static nodes once and evaluates only dirty slots or dirty blocks.

`key` remains necessary. A keyed `for` block owns a map from stable key to
mounted block instance so it can insert, move, retain, and dispose instances.
No framework can preserve input focus, scroll, local state, or animation
identity through reordered dynamic lists without an equivalent identity model.

## Language contract to preserve and extend

### Application semantics

Keep the Model-View-Update shape, but reserve room for explicit effects:

```lisp
(ui.app init update app)

;; phase 1
(update state action) -> state

;; planned extension
(update state action) -> (result next-state commands)
```

`view`/`app` must be pure and deterministic for a supplied state. It must not
perform FFI, network IO, time reads, random reads, mutation, or subscription
registration. Effects are declared data and deliver their completion as a later
action. This keeps rendered output portable, reproducible, and testable.

Event handlers should prefer data messages over closures:

```lisp
(button (on click (emit (TodoDelete todo.id))) (text "Delete"))
(input (on input (emit (DraftChanged (. event "value")))))
```

The host may use event delegation because it needs only an event descriptor,
the event payload projection, and a Jisp action expression. FFI-backed explicit
handlers remain an eventual, capability-gated escape hatch; they are not part
of portable UI semantics.

### Event cancellation and host effects

`preventDefault` and propagation control are **synchronous host policies**, not
reducer effects. A reducer receives an already-projected, JSON-shaped event
value, which is intentionally detached from the browser/native event object.
By then a browser default action may already have run, and an async, remote, or
out-of-process host cannot retroactively cancel it. Dioxus documents this exact
constraint for `prevent_default`; it is unavailable to its websocket LiveView
renderer because it must happen during native event dispatch. React and the DOM
also distinguish cancellation from propagation: stopping propagation does not
cancel the default action, and vice versa. [Dioxus event API](https://docs.rs/dioxus/latest/dioxus/prelude/struct.Event.html), [React event guide](https://react.dev/learn/responding-to-events), [MDN DOM events](https://developer.mozilla.org/en-US/docs/Web/API/Document_Object_Model/Events).

Therefore the portable surface should use declarative, statically visible
metadata on `on`, evaluated by the host before it sends an action to `update`:

```lisp
(button
  (on click (stop-propagation) (emit (MenuToggle menu.id)))
  (text menu.title))

(form
  (on submit (prevent-default) (emit (Save draft)))
  ...)
```

The lowering accepts at most one handler expression and the opt-in modifiers
`prevent-default`, `stop-propagation`, and, only when a host supports it,
`capture`. Their order is not semantic. A named pure action builder remains
valid, but it receives the portable event snapshot, not a host object:

```lisp
(defn menu-click (event) (MenuToggle menu.id))
(button (on click (stop-propagation) menu-click) (text menu.title))
```

The per-dispatch order is: host applies supported policies synchronously;
projects the safe event payload; evaluates the Jisp handler; sends its action
to `update`; then executes any commands returned by `update`. A host must
diagnose a required unsupported policy rather than silently ignore it. `capture`
is deliberately advanced/rare (primarily router and analytics use), while
`stop-immediate-propagation`, arbitrary DOM method calls, host event object
storage, and arbitrary view-side FFI are excluded from portable Jisp. The DOM
has separate same-target listener ordering semantics, so exposing
`stop-immediate-propagation` would create a portability trap.

This is intentionally less flexible than React's
`event => { event.stopPropagation(); dispatch(...) }`: React handlers are
host-language closures over a synthetic DOM event. That flexibility is useful
inside a browser app, but it would make a Jisp UI program browser-specific,
non-serializable, and hard to execute on native, SSR, or remote hosts. If a
native extension genuinely needs imperative handling, it belongs in an explicit
host capability with a documented fallback, never in `view` or a normal
portable event handler. Network, timers, storage, navigation, focus, clipboard,
and FFI likewise belong to data commands/subscriptions owned by `update`, not
to an event callback or view.

### Components and identity

Keep ordinary `(component name (args...) root)` calls valid. Add an opt-in
`defui`/`ui.component` declaration only when it has a clear advantage: it is a
compiler boundary with an explicit pure input contract, diagnostics, and
per-instance identity. Do not make ordinary functions silently stateful.

The initial compiler can infer a boundary for existing `component` definitions.
Later it may expose annotations such as `pure`, `memo`, or `dynamic`; these are
hints and diagnostics controls, not a requirement for normal application code.

Dynamic lists must use a direct child key:

```lisp
(for todo state.todos
  (key todo.id)
  (todo-row todo))
```

The compiler should reject a syntactically missing key in dynamic lists and
diagnose duplicate keys in development mode. It must not use position as an
implicit identity after a list has been declared keyed.

### Styling

The existing `(class ...)` and `(class-if ...)` directives remain structured
utility tokens. The UI compiler should retain static tokens for build-time CSS
extraction and represent dynamic token toggles as boolean slots. Arbitrary
runtime-built class strings are a fallback, not the primary Tailwind-like
styling path. This preserves portable inspection, validation, and eventual
native style-token mapping.

## JUIR design

JUIR needs a compact, versioned data model. Its exact Rust representation and
wire encoding are implementation details, but it must express the following.

| Item | Meaning |
| --- | --- |
| `Template` | Static node skeleton and stable template identifier. |
| `Slot` | A typed dynamic text, attribute, property, class, or style update. |
| `IfBlock` | Anchored conditional region with mount/update/dispose branches. |
| `EachBlock` | Anchored keyed collection with per-item template and key expression. |
| `ComponentCall` | Mounted child component with stable input slots and identity. |
| `Event` | Event type, required synchronous policy, safe payload projection, and action expression. |
| `Anchor` | Host-neutral position at which a dynamic region can be mounted. |
| `SourceMap` | Source span and component/template provenance for diagnostics/tools. |

Conceptual output for a todo row:

```text
template TodoRow(todo)
  static: div.todo-row > input[type=checkbox] + span + button
  slot:   input.checked <- todo.done
  slot:   span.class.done <- todo.done
  slot:   span.text <- todo.title
  event:  button.click -> TodoDelete(todo.id)
```

The browser executor creates this DOM once. On `TodoToggle`, it re-evaluates
only invalid slots and writes the property/text/class that actually changed.
The HTML executor walks the same representation to produce escaped markup. The
test executor materializes the existing structural representation.

### Expressions, dependencies, and equality

The Jisp runtime, not a JavaScript host, evaluates JUIR expressions. Dependency
information is an optimization and must never change observable semantics.

Use a hybrid strategy:

1. Compile direct immutable state reads into conservative dependency paths,
   e.g. `state.todos[*].done`.
2. During reducer execution, preserve structural sharing and record changed
   paths when the runtime can do so precisely.
3. Invalidate slots and component calls whose dependency paths intersect those
   changes.
4. For dynamic lookup or opaque user functions, track actual reads at runtime
   and cache the resulting dependency set; fall back to re-evaluation if a read
   cannot be tracked safely.
5. Compare stable primitive values by value and persistent structures by
   identity/version before writing a host slot.

This gives Solid/Leptos-style targeted updates internally without exposing
mutable signals as the public programming model. It also avoids promising that
all valid Jisp programs are statically dependency-analyzable.

### Component skipping

For a component call with stable inputs, retain the previous input values and
skip the component when its inputs are equal. Components that read app state
directly, use dynamic dependency tracking, or call an explicitly dynamic
function remain safely re-evaluable. Report why a component was not skipped in
development tooling.

This is analogous to Compose's skippable/restartable component boundaries and
React Compiler's automatic memoization, but Jisp can begin with a much smaller
analysis because UI components and immutable values have explicit language
semantics.

## Host protocol

### Browser DOM host

The first production executor should:

- create static DOM directly with DOM APIs;
- keep per-template/block instance records private to the host;
- apply property, attribute, text, class, and listener updates directly;
- use comment or equivalent anchors for `if` and `for` regions;
- reconcile keyed blocks with a stable `key -> instance` map;
- delegate serializable events from the root where compatible with the event
  type, while preserving required DOM event semantics; install a direct listener
  when a required synchronous policy or non-delegatable event needs one;
- preserve browser-controlled input values, focus, selection, and scroll unless
  the Jisp program intentionally updates the relevant property.

The DOM host must not parse source text or implement a second evaluator. It
receives JUIR plus updates from `jisp-wasm`/the Jisp runtime.

### Static HTML and test hosts

`ui.html` stays a pure, escaped renderer. It ignores events but renders the
same tree shape as the initial JUIR mount. Tests must compare:

- structural renderer output;
- initial JUIR host output;
- sequences of JUIR updates against full reference rerenders;
- all supported source syntaxes.

The reference renderer is a correctness tool and fallback, not a hidden second
set of UI semantics.

### Future native hosts

Native hosts implement the same lifecycle operations—mount, slot update,
keyed block reconciliation, dispose—but map templates to the native widget
registry. No DOM tag, JavaScript event object, CSS string, or browser FFI type
may leak into the core JUIR contract. Host-specific capabilities belong in a
separate registry and FFI/ABI design.

For cross-language or out-of-process hosts, expose capability and command
interfaces through a small, versioned protocol. WIT is a strong candidate once
the interface is stable because it defines language-neutral typed interfaces and
composition worlds. Do not use a WIT interface for every individual DOM slot
write: that would make the ABI, rather than the local host executor, the hot
path. Batch UI execution locally and use the ABI at capability/module boundaries.

## Effects, async work, and local state

Do not add effects before their lifecycle contract exists. The eventual minimum
should be data-oriented:

```text
update(state, action) -> { state: next_state, commands: [command...] }
command completion -> action
```

Commands need cancellation identity, ownership, error delivery, and disposal
on component/app unmount. Subscriptions need equivalent ownership and cleanup.
All capabilities must be explicit so static tests and non-browser hosts can
provide deterministic fakes.

Start with one root reducer. Component-local state is useful later, but must be
owned by a component instance path plus keyed identity and follow the same
message/effect discipline. Do not introduce hook-order semantics. A component
that disappears must deterministically dispose local state, subscriptions, and
effects.

## Delivery and SSR direction

Do not make SSR a prerequisite for the interactive runtime. Design now for it:

- template/component IDs are stable and versioned;
- state and event descriptors are serializable JSON-shaped data;
- server output may include inert block anchors and event metadata;
- client activation can use one small delegated listener before loading
  application-specific code.

This enables a later Qwik/Marko-inspired mode: static HTML is available
immediately, while interactive component code or a Jisp module loads only on
interaction/visibility. Full resumability is explicitly deferred: it requires
strict serializability of code references, state, and execution boundaries and
must not be claimed merely because hydration works.

## Milestones

### M0 — semantic baseline and measurements

**Scope**

- Document the pure app/view contract, key semantics, event payload shape, and
  current full-render limitations in `docs/UI.md`.
- Add reference UI scenarios: controlled input, focused input, reordered list,
  conditional subtree, nested components, and class/property changes.
- Add browser regression tests for focus, selection, scrollbar stability, and
  key-preserved list identity.
- Instrument the playground/reference host with mount, replace, and event
  counters for baseline measurements.

**Exit criteria**

- Every scenario runs from Lisp, JSON, YAML, and WS where the syntax can
  express it.
- Full rerender output is the documented semantic oracle.
- Known UX failures from root replacement are reproducible by tests.

### M1 — keys and reference reconciliation

**Scope**

- Specify and validate key uniqueness and type constraints.
- Implement a small keyed reconciler for the existing structural UI tree in the
  browser host.
- Preserve nodes through keyed inserts, deletes, and moves; update only changed
  scalar attributes/properties/text when possible.
- Retain full replacement as a debug/reference mode.

**Exit criteria**

- Reordering a keyed todo list retains each row's DOM identity and input focus.
- Updating an unrelated sibling does not recreate a controlled input.
- Differential tests show the reconciler has the same observable tree as the
  full renderer.

M1 is intentionally a correctness bridge, not the final runtime. It gives a
safe behavioral baseline for JUIR and fixes the playground's immediate UX
problems before deeper compiler work.

### M2 — JUIR and static-template compiler

**Scope**

- Add a UI-specific typed lowering pass after Core IR/type inference.
- Compile static elements, text, static attrs/classes, dynamic scalar slots,
  event descriptors (including cancellation/propagation policy), source maps,
  and component calls into JUIR.
- Implement a structural/JUIR test executor and an initial DOM executor.
- Use the JUIR executor behind a feature flag in the playground.

**Exit criteria**

- Initial JUIR mount equals `ui.html`/reference structural output for the
  supported subset.
- Static subtrees are created once rather than regenerated on every event.
- No syntax reader, parser crate, or JavaScript glue owns UI semantics.

### M3 — blocks and fine-grained invalidation

**Scope**

- Add anchored `if`, keyed `for`, nested component blocks, mount/update/dispose
  behavior, and keyed instance maps to JUIR.
- Preserve structural sharing/change-path metadata in the interpreter runtime.
- Add static dependency extraction plus runtime read-tracking fallback.
- Add component-input equality and skip diagnostics.
- Batch compatible writes per event/reducer turn without delaying controlled
  input writes.

**Exit criteria**

- A scalar action updates only its dependent slots in instrumented examples.
- Unchanged components and keyed row instances are skipped/retained.
- Dynamic/opaque expressions remain correct through tracked or conservative
  fallback evaluation.
- No stale UI is possible when dependency analysis is incomplete.

### M4 — commands, subscriptions, and local ownership

**Scope**

- Design a capability-based command/subscription protocol and its type-level
  representation.
- Keep host-event cancellation separate from commands: `prevent-default`,
  `stop-propagation`, and supported capture policy run synchronously at event
  dispatch; navigation, focus, clipboard, network, timers, storage, and FFI
  are explicit command capabilities with ownership and fallbacks.
- Add deterministic fake hosts for tests and explicit cancellation/disposal.
- Introduce opt-in component-local state only after instance identity and
  unmount semantics are proven.

**Exit criteria**

- Async completion, cancellation, errors, and disposal are deterministic in
  tests.
- Effects cannot execute from `view`.
- Component removal cleans up owned resources exactly once.

The first `ui.local` implementation satisfies this for stable static component
paths and keyed dynamic rows; it validates independent sibling instances,
reset-on-unmount, and keyed-list retention across reorder. Local
commands/subscriptions remain required before this milestone is complete.

### M5 — SSR, hydration, and host adapters

**Scope**

- Render JUIR to HTML with stable anchors/IDs and serializable initial state.
- Hydrate without replacing matching DOM or losing form state.
- Prototype one native host adapter against a deliberately small widget
  registry.
- Evaluate lazy component/module delivery only after the above is correct.

**Exit criteria**

- Server-first and client-only output agree structurally.
- Hydrated controls retain browser state unless Jisp intentionally updates it.
- Native-host capability gaps are explicit diagnostics, never silent DOM
  assumptions.

### M6 — portable component and capability ABI

**Scope**

- Specify version negotiation for JUIR modules, command capabilities, and host
  widget registries.
- Prototype WIT definitions for coarse-grained Jisp capability boundaries, such
  as storage, network, timer, navigation, and module loading.
- Generate at least two host bindings from the same contract and prove that an
  unsupported capability fails diagnostically.
- Evaluate optional AOT lowering of stable JUIR templates into a host language
  only after the interpreted executor is conformant.

**Exit criteria**

- Host language bindings are generated from one versioned contract rather than
  duplicated hand-written JSON conventions.
- The deterministic fixture component executes through Wasmtime and JCO/Node
  with matching capability and error behavior.
- The browser DOM bridge, native executor, and test executor retain identical
  UI semantics.
- UI patches remain local to each host; the cross-language ABI is not placed in
  the ordinary frame/update hot path.

## Validation strategy

The UI runtime needs more than snapshots.

- **Differential sequences:** generate action sequences, execute both the
  full-render oracle and incremental executor, then compare normalized trees.
- **Identity tests:** assert that keyed host instances survive insert/move/delete
  operations as specified.
- **DOM behavior tests:** focus, selection, scroll, input composition, boolean
  properties, event propagation, and listener replacement.
- **Portable fixture tests:** run equivalent programs through Lisp, JSON, YAML,
  and WS readers and compare the lowered JUIR.
- **Compiler property tests:** JUIR dependency metadata may over-invalidate but
  must never under-invalidate.
- **Performance guardrails:** record node creations, writes, evaluations, and
  retained instances; do not use wall-clock microbenchmarks as correctness
  gates.
- **Source maps/diagnostics:** invalid keys, unsupported event payload access,
  duplicate directives, and host capability failures report source ranges in
  the original syntax.

## Explicit non-goals for the first implementation

- A React-compatible API, hooks, JSX, or JavaScript component model.
- General runtime `eval` inside UI.
- Arbitrary browser DOM objects or closures as serializable state/actions.
- A universal dynamic `any`/`Value` ABI for native widget bindings.
- CSS compatibility with all Tailwind runtime string patterns.
- Concurrent rendering/scheduling semantics equivalent to React Fiber.
- Full resumability, code splitting, or server components before a correct
  mount/update/dispose protocol exists.

## Research basis

- [The Elm Architecture](https://guide.elm-lang.org/architecture/) motivates a
  model/view/update public contract.
- [Svelte](https://svelte.dev/) demonstrates compiler-oriented UI that performs
  minimal browser work rather than requiring a general VDOM runtime.
- [React Compiler](https://react.dev/learn/react-compiler/introduction) shows
  automatic memoization through data-flow and mutability analysis; its compiler
  lowers to an HIR and also provides validation diagnostics.
- [Solid fine-grained reactivity](https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity)
  and [Leptos reactivity](https://book.leptos.dev/reactivity/index.html) show
  targeted reactive updates without a VDOM.
- [Jetpack Compose stability](https://developer.android.com/develop/ui/compose/performance/stability)
  motivates restartable/skippable component boundaries and stable input checks.
- [Glimmer](https://glimmerjs.com/) and
  [Incremental DOM](https://github.com/google/incremental-dom) are useful
  references for compiled template VMs and in-place DOM update targets.
- [Slint](https://github.com/slint-ui/slint) is the strongest direct native
  precedent for declarative, pure UI expressions compiled to optimized
  cross-platform code.
- [Mitosis](https://mitosis.builder.io/docs/overview/) validates a common
  component IR with multiple framework targets, while also illustrating why
  Jisp must not inherit the semantic constraints of those frameworks.
- [Jane Street Incremental](https://github.com/janestreet/incremental) and
  [Incr_dom](https://www.janestreet.com/tech-talks/intro-to-incr-dom/) inform
  dependency graphs, equality cutoffs, and efficient dynamic collections.
- [Angular Signals](https://angular.dev/guide/signals) and
  [Swift Observation](https://developer.apple.com/documentation/observation)
  are current production examples of read tracking as a rendering optimization.
- [Dioxus custom renderers](https://dioxuslabs.com/learn/0.7/guides/depth/custom_renderer/)
  illustrate the portability of a mutation/event protocol even though Dioxus
  itself retains a VDOM.
- [Iced](https://github.com/iced-rs/iced) reinforces the choice of Elm-style
  state/messages/view/update for native applications, while
  [egui](https://github.com/emilk/egui) clarifies why immediate mode is a useful
  adapter rather than Jisp's canonical UI model.
- [Qwik resumability](https://qwik.dev/docs/concepts/resumable/) and
  [Marko lazy loading](https://markojs.com/docs/reference/lazy-loading) inform
  later serializable event metadata, lazy delivery, and SSR work.
- [The WebAssembly Component Model](https://component-model.bytecodealliance.org/design/component-model-concepts.html)
  and [WIT](https://component-model.bytecodealliance.org/design/wit.html)
  inform future cross-language capability bindings, not the renderer hot path.
- [Adapton](https://www.cs.umd.edu/~mwh/papers/adapton-submit.pdf) and
  [Nominal Adapton](https://research.cs.queensu.ca/home/jana/papers/noma/)
  provide the incremental-computation rationale for demand tracking, reuse,
  and stable identities.

## Decision gate

Before M2, review the M1 reconciler evidence and approve the exact public
surface for `defui`, effects, and local state. The compiler should be built
only after that review; its internal representation can evolve, but Jisp's
portable source and reducer semantics must not churn with each host experiment.

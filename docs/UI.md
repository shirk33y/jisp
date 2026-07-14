# Declarative UI

Jisp's default UI source syntax is a component tree with explicit host
elements. It lowers to renderer-neutral structural data, so the same program
can target the interpreter's static HTML renderer and a host-managed
interactive runtime without changing its source form.

```lisp test=ui.component-syntax mode=run
(component todo-row (title)
  (li
    (attr "data-id" title)
    (class "rounded" "px-2")
    (span (text title))))

(component todo-list (titles)
  (ul
    (attr "aria-label" "Tasks")
    (for title titles
      (todo-row title))))

(export main
  (fn ()
    (ui.html (todo-list (list "Plan" "Ship")))))
```

## Components and elements

`(component name (parameters...) root)` defines a private function whose body
is one UI root. A call such as `(todo-row title)` is an ordinary function call
and, when placed in an element body, is a child. Component calls never imply a
property, attribute, or CSS class.

Host element names are intentionally finite and explicit. The current registry
contains `a`, `article`, `aside`, `button`, `div`, `footer`, `form`, `h1`,
`h2`, `h3`, `header`, `img`, `input`, `label`, `li`, `main`, `nav`, `ol`,
`option`, `p`, `section`, `select`, `span`, `strong`, `textarea`, and `ul`.
Those names are reserved as component names. Extending the registry is a core
language change, so every parser and host observes the same element set.

`(text expression)` creates an escaped text child. Any other expression in an
element body is a child expression; use a component call or a standard Jisp
expression that evaluates to a node. `(for ...)` is the list-producing child
form. `(if condition then-node else-node)` is valid both as a component root
and as a child expression; each branch is lowered as UI, so it may contain an
explicit host element or component call.

```lisp
(component status (visible)
  (if visible
    (div (class "status-ok") (text "Connected"))
    (div (class "status-warn") (text "Offline"))))
```

## Explicit directives

Metadata is never inferred from spelling. In particular, a hyphen does not
turn a name into a class: `aria-label`, `data-id`, and `http-equiv` belong in
`attr` when they are HTML attributes.

| Form | Meaning |
| --- | --- |
| `(attr name value)` | HTML attribute. `name` is a symbol or string; static HTML renders scalar values and omits `null`/`false`. |
| `(prop name value)` | Renderer property. The static HTML renderer currently serializes scalar props as attributes. |
| `(class name...)` | Enables each named utility class. |
| `(class-if name condition)` | Enables the named utility class only when the boolean condition is true. |
| `(on event (emit action))` | Creates a delayed handler. The interactive host supplies `event`, evaluates `action`, then passes that value to the app update function. Static HTML deliberately does not serialize it. |
| `(on event modifier... handler)` | Adds zero or more synchronous host policies before a delayed or explicit handler. `prevent-default`, `stop-propagation`, and `capture` are the supported modifiers. |
| `(on event handler)` | Stores an explicit function handler for an interactive host. Use this when the handler needs more than one expression. |
| `(key value)` | Stores a scalar (`str`, number, or `bool`) identity key for reconciliation. Static HTML deliberately ignores it. |
| `(for binding collection child)` | Maps `child` over `collection`; nested result lists are flattened as children. |

Each directive belongs directly inside a host element. Names must be unique
within their own directive category; a duplicate `attr`, `prop`, `class`, or
`on` name is a lowering error. There can be only one `key` directive. Keys are
unique among rendered sibling children in an interactive host; duplicate keys
are rejected before a tree is sent to that host. Use a stable domain identifier,
not a list index, whenever list items may be inserted, removed, or reordered.

`emit` is only valid as the handler argument of `on`. It introduces an implicit
single `event` argument, so input values can be turned into actions without
browser state leaking into the update function:

```lisp
(input
  (prop value (. state "draft"))
  (on input (emit (Draft (. event "value")))))
```

Event modifiers are evaluated by the host **before** Jisp receives its portable
event snapshot. They are not reducer effects, because a reducer cannot cancel
a browser default action after dispatch has completed. Use them only for the
corresponding host-event behavior:

```lisp
(form
  (on submit (prevent-default) (emit (Save draft)))
  (button (prop type "submit") (text "Save")))

(button
  (on click (stop-propagation) (emit (MenuToggle menu.id)))
  (text menu.title))
```

`capture` installs the listener during the capture phase; it is mainly for
router or analytics boundaries. The browser playground applies these policies
synchronously and then sends only `type`, `value`, `checked`, and `key` to
Jisp. It never passes a DOM event object or exposes arbitrary DOM methods.

## Update-driven applications

An interactive module declares its three host entry points with:

```lisp
(ui.app init update app)
```

`init` is an immutable initial value, `update` has the shape
`(state action) -> state`, and `app` is a component with the shape
`(state) -> ui.node`. The third argument names that component directly, so no
extra `(def view app)` alias is needed.
The declaration does not create a JavaScript store or an effect system: it is
metadata that lets each host keep the same execution contract.

```lisp test=ui.update-example mode=check
(type Action (Increment))

(def init (obj "count" 0))

(defn update (state action)
  (case action
    ((Increment) (obj.set state "count" (+ (. state "count") 1)))))

(component counter (state)
  (button
    (on click (emit Increment))
    (text (str "Count: " ,(str.from (. state "count"))))))

(ui.app init update counter)
```

The browser playground currently uses this contract. On each event it calls
the selected Jisp handler with a small JSON-shaped event object, calls
`update(state, action)`, then evaluates the typed JUIR plan for
`app(next-state)` through the canonical Jisp evaluator. The initial client-only
mount receives a versioned static JUIR skeleton plus the current structural
values for dynamic slots/blocks; the browser creates matching static elements
directly and never evaluates a Jisp expression. Later events send a batch of
text, element-metadata, child-list, or replacement patches. The browser host
applies those patches in place: it updates changed text, attributes,
properties, classes, and handlers, and retains/moves keyed sibling nodes. It
also numbers input events so an older patch cannot overwrite a newer in-progress
controlled edit. This preserves focused controls and their selection through
ordinary updates. The structural tree remains the semantic oracle and recovery
snapshot while the compiled JUIR runtime evolves.

Executable effects, subscriptions, async commands, persistence, and lifecycle
boundaries are not implemented; reducer resource declarations and their
proposed ownership/capability contract live in [UI_EFFECTS.md](UI_EFFECTS.md).
A UI component remains a pure function of its supplied state and props.

For development diagnostics, `PlaygroundSession.metrics()` reports render
counts plus the latest JUIR slot, block, keyed-item, and component reuse counts. The
browser host separately records DOM mounts, replacements, text writes,
element/child patches, and forwarded events. The playground exposes the latest
decision in its status pill and both JSON payloads as that pill's tooltip.
These counters are observability data, never a part of a component's public
result.

`PlaygroundSession.dispatch_patches(handler, event)` exposes the same update
batch for another browser or native host. `snapshot()` returns the complete
current tree only for initial mount or host recovery; it is not the normal
event-update path.

`PlaygroundSession.source_map()` returns the versioned
`jisp-ui-source-map/1` manifest for the compiled JUIR plan. Every entry carries
the component name, a stable compiler-plan path, source id, byte start/end, and
kind (`element`, `text`, `slot`, `event`, `if`, `each`, and related expression
locations). Paths describe immutable JUIR templates such as
`root.children.1.props.value`; they are intentionally not DOM paths or dynamic
list indices. A browser, native host, or editor can attach diagnostics to the
original source without parsing/evaluating Jisp or treating transient host nodes
as source identity.

`PlaygroundSession.mount_plan()` returns `jisp-ui-mount-plan/1`: a static
skeleton of elements/text plus explicit dynamic holes for `if`, `for`, component
calls, and opaque node values. Static attributes, properties, and class tokens
are included for inspection. The plan contains no expression code, closures, or
host objects; a host combines it with the initial structural value returned by
the canonical executor and falls back to that value for every dynamic hole.

### Native adapter prototype

`jisp-ui::native` is an in-memory reference adapter for a deliberately small
semantic widget registry. It maps the structural tree to `Container`, `Text`,
`Button`, `TextInput`, `List`, `ListItem`, `Form`, and `Label` widgets, retains
utility classes as opaque style tokens, and retains event names as declarative
bindings. It has no DOM, CSS, JavaScript event, or toolkit dependency.

The registry intentionally does not support every Jisp element yet: for
example `img`, `a`, and `select` fail with `native host does not support Jisp
element ...`. Unsupported native attributes, properties, and events fail just
as explicitly. This is a contract/prototyping host, not a GUI backend; a future
platform adapter owns retained-widget mount/update/dispose operations and maps
only supported style tokens and capabilities.

`render_ssr(source)` and `PlaygroundSession.ssr()` return the versioned
`jisp-ui-ssr/1` payload: escaped `html`, serializable initial `state`, and the
same structural `tree` used by the interactive host. The embedding server owns
safe document serialization and activation. SSR HTML carries generated,
reserved `data-jisp-path` markers on elements and `data-jisp-key` markers on
keyed elements. They come from the tree rather than source attributes, so an
application cannot spoof a hydration anchor.

When a matching SSR tree already exists, the playground host attaches paths,
properties, and listeners in place; it does not overwrite `innerHTML`, replace
matching nodes, or reset browser-entered `value`/`checked` control state during
that first attachment. A later reducer update still writes a changed declared
property as usual. The playground's ordinary preview is client-only and mounts
the compiled static skeleton directly; an embedding server may instead seed a
matching SSR payload before invoking the same hydration path. A mismatching
server tree is rejected and recovered through the ordinary full-tree mount
rather than silently claiming hydration succeeded.

This proves the hydration contract locally; a production server-delivery
adapter, block-level SSR anchors, and resumability remain future work.

## Lowered contract and host status

The lowerer produces an ordinary structural node with `tag` and, when present,
`attrs`, `props`, `classes`, `events`, `key`, and `children`. This is an
implementation contract for renderers, not the recommended source syntax.
`ui.html` renders the tag, attributes, properties, classes, text, and flattened
children with HTML escaping. It purposefully ignores `events` and `key`.

## Portable UI tests

Fixture-only `ui.test` scenarios exercise the declared `ui.app` without a DOM:

```lisp
(ui.test "counter updates"
  (assert (= "<button>0</button>" (ui.test.html)))
  (dispatch Increment)
  (assert (= 1 (ui.test.state))))
```

`dispatch` sends a plain Jisp action to `update`. Assertions observe the next
state, static HTML, or the renderer-neutral `ui.test.tree`; each observation
also verifies that the reference component value and JUIR agree. These forms
are removed before an app is rendered, and the same runner is available from
the playground's **Run tests** button. See [TESTING.md](TESTING.md) for fixture
locations and cross-syntax generation.

This is a declarative UI language with a deliberately small interactive host
contract, not yet a React-equivalent runtime. Effect/lifecycle semantics,
subscriptions, async commands, native widget registries, and Tailwind-style
token validation remain future runtime work. The static `ui.html` renderer
still preserves neither event handlers nor keys; it rejects inline `on*`
attributes, so portable events must use the `on` directive.

The GitHub Pages playground runs this same interpreter through the
`jisp-wasm` WebAssembly entry point. JavaScript consumes a JUIR static mount
skeleton plus renderer-neutral dynamic values in an isolated preview and
forwards browser events; it does not parse or evaluate a second UI language,
and it does not implement the update function. Its Lisp, JSON, YAML, and
indentation-based WS selector converts the parsed module before reloading it
through the selected reader. Comments are not part of the shared syntax tree,
so conversion intentionally drops them.

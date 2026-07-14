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
form.

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
| `(on event handler)` | Stores an explicit function handler for an interactive host. Use this when the handler needs more than one expression. |
| `(key value)` | Stores an identity key for reconciliation. Static HTML deliberately ignores it. |
| `(for binding collection child)` | Maps `child` over `collection`; nested result lists are flattened as children. |

Each directive belongs directly inside a host element. Names must be unique
within their own directive category; a duplicate `attr`, `prop`, `class`, or
`on` name is a lowering error. There can be only one `key` directive.

`emit` is only valid as the handler argument of `on`. It introduces an implicit
single `event` argument, so input values can be turned into actions without
browser state leaking into the update function:

```lisp
(input
  (prop value (. state "draft"))
  (on input (emit (Draft (. event "value")))))
```

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
`update(state, action)`, then calls `app(next-state)` and replaces the preview
tree. This is deliberately a simple full-render host: keyed reconciliation,
effects, subscriptions, async commands, persistence, and native widget
adapters are not defined yet.

## Lowered contract and host status

The lowerer produces an ordinary structural node with `tag` and, when present,
`attrs`, `props`, `classes`, `events`, `key`, and `children`. This is an
implementation contract for renderers, not the recommended source syntax.
`ui.html` renders the tag, attributes, properties, classes, text, and flattened
children with HTML escaping. It purposefully ignores `events` and `key`.

This is a declarative UI language with a deliberately small interactive host
contract, not yet a React-equivalent runtime. Effect/lifecycle semantics,
subscriptions, async commands, keyed reconciliation, native widget registries,
and Tailwind-style token validation remain future runtime work. The static
`ui.html` renderer still preserves neither event handlers nor keys.

The GitHub Pages playground runs this same interpreter through the
`jisp-wasm` WebAssembly entry point. JavaScript loads the module, renders the
returned structural tree in an isolated preview, and forwards browser events;
it does not parse or evaluate a second UI language, and it does not implement
the update function. Its Lisp, JSON, and YAML selector converts the parsed
module before reloading it through the selected reader. Comments are not part
of the shared syntax tree, so conversion intentionally drops them.

# Declarative UI

Jisp's default UI source syntax is a component tree with explicit host
elements. It lowers to renderer-neutral structural data, so the same program
can target the interpreter's static HTML renderer today and a future native or
interactive host runtime without changing its source form.

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
| `(on event handler)` | Stores a named event handler for an interactive host. Static HTML deliberately does not serialize it. |
| `(key value)` | Stores an identity key for reconciliation. Static HTML deliberately ignores it. |
| `(for binding collection child)` | Maps `child` over `collection`; nested result lists are flattened as children. |

Each directive belongs directly inside a host element. Names must be unique
within their own directive category; a duplicate `attr`, `prop`, `class`, or
`on` name is a lowering error. There can be only one `key` directive.

## Lowered contract and host status

The lowerer produces an ordinary structural node with `tag` and, when present,
`attrs`, `props`, `classes`, `events`, `key`, and `children`. This is an
implementation contract for renderers, not the recommended source syntax.
`ui.html` renders the tag, attributes, properties, classes, text, and flattened
children with HTML escaping. It purposefully ignores `events` and `key`.

This is a declarative UI language and a static-rendering proof, not yet a
React-equivalent runtime. State cells, update scheduling, lifecycle/effect
semantics, event dispatch, a reconciler, native widget registries, and
Tailwind-style token validation remain future runtime work. Until those
contracts exist, handlers and keys are preserved as data for a host rather than
executed by Jisp itself.

# UI language validation

## Goal

Validate whether Jisp is useful as a portable UI description language, roughly
in the space of React plus Tailwind-like utility styling.

This is P1 validation work, not P0 compiler/runtime work. It should pressure
test existing syntax, lowering, type inference, imports, diagnostics, and future
renderer targets without forcing the core language to become a web framework.

## Prototype evidence

Existing prototype files in the legacy misc bundle already explore:

- React-like component composition with props, hooks, state, and render helpers.
- HTML-like nodes such as `html`, `body`, `div`, `ul`, `li`, `a`, `span`.
- DOM event bindings such as `on-click`, `on-scroll`, `on-focus`, and
  `on-mouseover`.
- Tailwind-like utility tokens written directly in node forms.
- Conditional class activation, including syntax shaped like
  `px-2: [eq, name, "kompot"]`.
- Alternate surface forms: bracketed lists, YAML sequence/mapping blends,
  brace-heavy component forms, and indentation-oriented sketches.

Useful source examples from that bundle include `example tw.yaml`,
`minimalite.yaml`, `poc.yaml`, `example.js`, `example.ffffff`,
`example copy 2.yaml`, `example copy 4.yaml`, and `example.square.yaml`.

## First-class utility classes

Utility classes should be first-class UI data, not hidden behind `class` or
`className`, and not represented as one whitespace-separated string. The core
model should preserve each utility token separately so renderers, type checks,
diffing, tooling, and conditional activation can inspect them.

Candidate normalized shape:

```lisp
[div,
  [px-1,
   mx-5: [., user, is-active],
   [id: "my-div", title: blog-title],
   [span, [], [], [text, "Content"]]]]
```

The intended class-set semantics are closer to:

```text
{
  "px-1": true,
  "mx-5": user.is-active
}
```

Attributes remain ordinary structured data:

```text
{
  "id": "my-div",
  "title": blog-title
}
```

## Syntax tradeoff

Using mapping entries inside lists is worth validating because it keeps common
UI forms compact and readable:

```lisp
[div, px-1, mx-5: active?, id: "my-div", [text, "Content"]]
```

The cost is that `[id: "my-div", title: blog-title]` is no longer visually just
a list of expressions; it is syntax sugar for mapping entries. That compromise
is acceptable only if normalization makes the shape explicit early:

- utility class entries become a typed class-set node;
- attribute entries become an attribute map;
- child nodes remain children;
- ambiguous entries produce source-ranged diagnostics instead of guessing.

The project should validate this with real examples before committing the
surface syntax to the language spec.

## Milestone shape

P1 should prove:

- one normalized UI AST for all source syntaxes;
- one renderer target, probably HTML/React first;
- utility class sets with boolean activation expressions;
- event/state binding representation in IR or a UI-specific lowered layer;
- diagnostics that distinguish unknown tag, unknown attribute, invalid class
  entry, invalid event handler, and child/attribute/class ambiguity.

P2 can then refine ergonomics: formatter support, LSP/schema completions,
component package conventions, more render targets, and syntax sugar.

# Indentation reader syntax evaluation

**Status:** Historical design rationale. The `ws` reader is implemented; the
current source contract belongs in [`docs/SPEC.md`](../../docs/SPEC.md).

## Context

This evaluates an indentation-oriented surface syntax sketch for Jisp, not an
implementation plan. Current project rules still say Lisp is the primary
human-written syntax and new readers must only normalize into the shared
source-aware `Node` AST.

Useful prior art:

- SRFI-110 sweet-expressions:
  https://srfi.schemers.org/srfi-110/srfi-110.html
- SRFI-119 wisp:
  https://srfi.schemers.org/srfi-119/srfi-119.html
- Wisp overview:
  https://www.draketo.de/software/wisp
- Rhombus macro-extensible infix syntax:
  https://dl.acm.org/doi/abs/10.1145/3622818
- Honu algebraic notation through enforestation:
  https://www-old.cs.utah.edu/plt/publications/gpce12-rf.pdf
- Python lexical indentation rules:
  https://docs.python.org/3/reference/lexical_analysis.html#indentation
- Haskell 2010 layout rule:
  https://www.haskell.org/onlinereport/haskell2010/haskellch10.html#x17-17800010.3

## Short verdict

The syntax can work as a reader experiment if it stays a transparent S-expression
layout notation. It should not change language semantics, introduce shell-like
execution, or redefine core forms. The parser must produce exactly the same AST
as explicit Lisp/JSON/YAML-like inputs, including spans.

The strongest version is a small Wisp-like layout reader:

- each non-empty line starts a form;
- indented child lines become additional arguments to the parent line's form;
- a small continuation marker can append more arguments to the current parent;
- explicit parenthesized Lisp remains legal for hard cases;
- ambiguous indentation is rejected instead of guessed.

The weakest version is a shell/Python hybrid where indentation, `|`, flags,
`..`, `...`, and line breaks all imply different semantic behavior. That would
make parser crates own semantics and would conflict with the current architecture.

## Prior-art comparison

`ws` is closest to Wisp/SRFI-119, not to sweet-expressions or Rhombus. The common
core is a layout reader that remains general and homoiconic: indentation changes
tree shape, but the reader does not know whether a form is a function call,
binding, object literal, UI component, or macro form.

SRFI-49 I-expressions are the older baseline. They preserve the important idea
that indentation can be the grouping mechanism without adding semantic special
cases, and they freely mix with explicit S-expressions. Their weak spot is
argument continuation after nested calls: without a marker, continuing flat
arguments often degenerates into one argument per line or ambiguous layout.

SRFI-110 sweet-expressions solve more readability cases, but by combining
several features: indentation, neoteric calls, curly infix, sublist markers,
group/split markers, and collecting lists. That breadth is useful prior art, but
it is the wrong default direction for Jisp. Jisp already has multiple equivalent
readers, so the safer move is a small additional reader, not a larger competing
surface language.

Wisp/SRFI-119 is the most relevant predecessor because it deliberately reduces
the added syntax. Its leading-period continuation is the key idea to keep:
flat argument continuation must be explicit when indentation would otherwise
create a nested form. `ws` uses a line-leading `...` for the same role because
Jisp already uses `...` inside parameter lists and patterns, and the marker can
be reserved only at the beginning of a layout line.

Rhombus and Honu are a different family. They do not merely remove parentheses
from S-expressions; they preserve macro extensibility with conventional or infix
notation. The useful lesson for Jisp is not to put that complexity in `ws`.
Instead, context-specific sublanguages can live above the reader: for example,
a `component` lowerer or macro can decide that a two-item child form means a UI
property pair, while the generic `ws` reader still emits ordinary forms.

## What to borrow for Jisp

Borrow Wisp's explicit continuation concept, but keep Jisp's current spelling:
line-leading `...` appends the remaining tokens to the immediate layout parent.
This keeps object/UI/value forms expressible without making the reader know
about `obj`, `tag`, `component`, `let`, or `case`.

Borrow SRFI-49 and Wisp's escape hatch: explicit S-expressions remain legal
inside layout source. This is the right answer for parameter lists, rest
patterns, calls used as callees, quasiquote-heavy code, and any case where
layout would obscure the tree.

Borrow Python and Haskell's specification discipline, not their surface syntax.
The reader should reject tabs, non-space indentation, odd indentation widths,
indent jumps, top-level continuations, empty continuations, and line-leading
ellipsis-like typos. Layout syntax needs better diagnostics than a parenthesized
reader because visual shape is the user's parse model.

Borrow SRFI-110's round-trip mindset without its extra markers. A formatter or
writer should be able to render canonical Lisp fixtures into `ws`, and tests
should prove that `.lisp`, `.ws`, `.json`, and `.yaml` samples agree at the
AST/IR/evaluation layers.

Borrow Rhombus's idea of context-specific sublanguages only after parsing. UI
ergonomics should be improved in the `component`/macro/lowering layer, not by
making the generic `ws` reader context-sensitive. That keeps the reader simple
while leaving room for component bodies to accept friendlier property syntax.

Do not borrow sweet-expression infix, neoteric calls, `$`, `\`, `<* *>`, Wisp's
colon grouping, shell pipelines, or named-function sugar for the initial reader.
Those features may be good language designs elsewhere, but in Jisp they would
turn a normalizing reader into a second language surface with its own semantics.

## Strong sides

- It attacks the main readability cost of Lisp examples: closing parens and
  rightward drift in deeply nested calls.
- It maps naturally to the existing AST because Jisp already has only scalar
  nodes and `Form(Vec<Node>)`.
- It can preserve homoiconicity if every layout construct has an obvious
  explicit S-expression equivalent.
- It is friendly to structural UI/object programs where nested call shapes are
  common.
- It could be implemented as one extra `SyntaxParser` crate without touching
  lowering, typing, evaluation, or codegen.
- It can coexist with explicit Lisp, matching the SRFI-110/SRFI-119 migration
  story.

## Weak sides

- `fn foo` is not current core Jisp. Current functions are values, usually
  `(def name (fn (params) body))`. A named-function line would be extra sugar
  and should normalize before lowering.
- Layout syntax needs formal line handling: blank lines, comments, tabs, string
  continuations, dedent errors, and EOF dedents. Python and Haskell both show
  that this must be specified, not left to intuition.
- Continuation is the hard part. Pure indentation cannot express every useful
  argument layout cleanly. Wisp needed a leading continuation marker for this.
- Reusing `...` as a generic continuation/spread marker is risky because Jisp
  already uses `...` for rest parameters and list rest patterns.
- `..` is also weak: it has no current meaning, visually resembles ranges or
  member access in other languages, and would add a second rest/continuation
  family next to `...`.
- Alternating `obj key value` remains compact but fragile. Multi-line object
  entries need parity diagnostics or explicit pair grouping, otherwise one
  misplaced line changes the object shape silently.
- Shell-looking examples such as `adb shell`, flags, and `| grep` are misleading
  unless they are ordinary Jisp calls. If they imply shell tokenization or
  process pipelines, the reader is doing semantics.
- Inconsistent indentation can be parseable but confusing. A formatter must be
  part of the feature before the syntax becomes default human-written source.

## Sketch-specific notes

This is good and should be a target shape:

```text
fn foo
  x y z ... rest
  print
    + x y z
```

But it should probably normalize to current core shape, not redefine `fn`:

```lisp
(def foo
  (fn (x y z ... rest)
    (print (+ x y z))))
```

These are the useful everyday shapes to preserve in examples and tests:

```text
def answer
  fn ()
    + 40 2
```

Equivalent Lisp:

```lisp
(def answer
  (fn ()
    (+ 40 2)))
```

Simple nested call trees are where `ws` helps most:

```text
export main
  fn ()
    str.upper
      str "hello"
```

Equivalent Lisp:

```lisp
(export main
  (fn ()
    (str.upper (str "hello"))))
```

Flat continuation is the important explicit marker:

```text
list
  ... 1 2 3
  + 2 2
  ... 5 6
```

Equivalent Lisp:

```lisp
(list 1 2 3 (+ 2 2) 5 6)
```

This is readable but requires a continuation rule:

```text
sum 1 2 3
  + 5 6
```

Recommended meaning:

```lisp
(sum 1 2 3 (+ 5 6))
```

This should be rejected unless a formal continuation/dedent rule makes it
unambiguous:

```text
sum
      1 2 3
  + 5 6
```

For objects, prefer either same-line pairs or explicit pair blocks:

```text
let object
  obj k0 v0
    k1 v1
    k2
      + 2 2
```

Avoid `.. k v` unless `..` is formally reserved for one purpose. If the user
preference is "use `...` instead of `..`", restrict it to current rest/spread
roles and avoid making it the generic layout continuation marker.

Pipelines should start as ordinary calls, not shell syntax:

```text
pipe
  adb.shell "dumpsys" "power"
  grep "state=OFF"
```

or stay explicit Lisp until a pipeline design exists.

## Edge cases to resolve

The core smell is that layout syntax has two independent choices:

- flat argument to the parent versus nested child form;
- atom/value versus single-item form or zero-argument call.

`...` can solve the first choice, but not the second. A single-token line is the
pressure point:

```text
fn id
  x
  x
```

If `x` is an atom, the first `x` is not a valid one-parameter list. If `x` is a
form, the second `x` calls `x` instead of returning the value `x`. One of these
must use an explicit escape hatch, such as `(x)` for a singleton form or an
explicit zero-arg call syntax.

Header-hoisting `...` is risky:

```text
call-this a b
  ... c d
  foo 123
  ... e f
  bar 222
```

If this becomes `(call-this a b c d e f (foo 123) (bar 222))`, source order no
longer matches evaluation order. A safer generic rule is source-order flattening:

```lisp
(call-this a b c d (foo 123) e f (bar 222))
```

or a stricter rule where `...` is allowed only before the first nested child.

Object and binding forms expose the same issue. A normal child line is one
argument, not two:

```text
obj
  "k" v
```

Generic layout would read this as `(obj ("k" v))`, not `(obj "k" v)`. Either
object examples must use flat continuations:

```text
obj
  ... "k" v
```

or the reader becomes context-aware, which conflicts with parser crates being
syntax normalizers only.

Source-order flattening helps mixed pair/value shapes:

```text
obj
  ... "sum"
  + 2 2
  ... "label" label
```

This can map to `(obj "sum" (+ 2 2) "label" label)`. Header-hoisting cannot
support this shape without reordering fields or rejecting useful layouts.

For UI-like code, the generic reader stays the same. Props that should remain
flat arguments need continuations; child elements use ordinary indentation:

```text
component metric-card (title value delta trend)
  article
    ... id title
    ... rounded-lg true
    class positive
      = trend "up"
    header
      p text-sm true muted true
        text title
      strong value
      span trend true
        text trend
```

Equivalent shape:

```lisp
(component metric-card (title value delta trend)
  (article
    id title
    rounded-lg true
    (class positive (= trend "up"))
    (header
      (p text-sm true muted true (text title))
      (strong value)
      (span trend true (text trend)))))
```

This is deliberately a little noisy around props. If component bodies later need
friendlier prop syntax, that should be a component-body sublanguage after
parsing, not a context-sensitive rule in the generic `ws` reader.

Nested continuation must attach to the immediate layout parent, not the nearest
visual function name:

```text
outer
  inner a
    ... b
```

should affect `inner`, not `outer`. After dedent, it affects the dedented parent:

```text
outer
  inner a
  ... b
```

should affect `outer`.

`...` already has core meaning in function parameters and list patterns:

```lisp
(fn (head ... tail) ...)
((list first ... rest) ...)
```

A layout reader can still use leading `...` as a line marker, but diagnostics
must clearly distinguish "layout continuation" from "rest marker inside a form".

Zero-argument calls need an explicit spelling if a one-token line means a value:

```text
now      # value
(now)    # call, using explicit Lisp escape hatch
```

This is probably acceptable, but it must be a deliberate rule.

Additional model-checking in `.agents/scripts/0008-layout-roundtrip-model.py`
found the most important completeness condition: `...` must preserve source
order. With source-order continuation, all generated datums up to 8 nodes
round-tripped through `render -> parse` in the bounded model, including
form/atom/form interleavings and forms used as callees through explicit
S-expression islands.

## Expanded adversarial search

The model now checks 64 adversarial cases. The search is still bounded and not a
proof for the real lexer, but it is useful because it forces edge decisions
before implementation.

The user's multiline explicit island example is rejected:

```text
defn foo (a b)
  ... (x y z
    ... k l
```

Reason: `(x y z` starts an explicit S-expression island that does not close on
the same physical line. In the recommended MVP, layout is line-oriented and
explicit `(...)` islands must be complete inside one line. The literal one-space
indent version is rejected even earlier as invalid indentation.

If multiline explicit islands are later allowed, the reader needs a second mode:

- while inside unmatched `(...)`, indentation must not create layout structure;
- a leading `...` inside that island is an ordinary token/rest marker, not a
  layout continuation;
- comments and strings must be lexed by the explicit S-expression lexer, not the
  layout lexer;
- source spans must still point to the original multiline region.

That is implementable, but it is a larger lexer contract. It should not be in the
first layout-reader experiment.

The expanded cases also lock these rules:

- `...` at the beginning of a layout line appends flat arguments to the immediate
  parent at that source position.
- `...` after another token is just an atom, so `a b ... rest` can remain the
  existing rest-marker spelling.
- `...` inside a same-line explicit island, such as `(... k l)`, is parsed by the
  explicit S-expression parser, not by layout.
- an empty continuation line is invalid, because otherwise `f / ...` silently
  becomes a zero-argument call shape.
- tabs outside strings are invalid in the model; tabs plus layout are too easy to
  make editor-dependent.
- comments and blank lines may be skipped only after a real lexer rule is chosen.
  The model treats `#` after whitespace as a line comment and keeps `#` inside
  strings.
- top-level continuation is invalid; continuation must have a layout parent.
- multi-token child lines are nested forms, not flat parent arguments.
- a one-token child line is a value unless it has children; with children it
  becomes a form.
- line-leading tokens that merely start with `...`, such as `...rest` or
  `....`, are invalid. This prevents visually ambiguous typos next to the layout
  continuation marker.
- stray `(` or `)` inside non-explicit atoms are invalid. Without this, examples
  like `f x)` are too easy to accept as symbols by accident.
- non-space indentation characters are invalid in layout mode. The model rejects
  tabs and non-breaking spaces outside strings instead of normalizing them.

The last two rules are the main ergonomic cost:

```text
f
  x y
```

means:

```lisp
(f (x y))
```

not:

```lisp
(f x y)
```

To get flat arguments after a line break, the spelling must be explicit:

```text
f
  ... x y
```

This is the price of keeping the reader context-free. If object literals or
binding forms want prettier pair syntax without `...`, that should be syntax
sugar after parsing or a separate surface form, not generic layout behavior.

The model also marks these as deliberate failures:

```text
defn foo (a b)
  ... (x y z
    ... k l
```

This should be rejected in a layout reader unless the design explicitly allows
multi-line S-expression islands. If multi-line islands are allowed, layout
processing must be disabled until the matching `)`, and inner leading `...`
must be treated as an ordinary S-expression symbol/rest marker, not as layout
continuation. That rule is harder to teach and easier to misread, so the safer
MVP is: explicit `(...)` escape hatches must close on the same physical line.

There is one real completeness caveat: making leading `...` syntax means an
ordinary atom named `...` at the start of a layout line needs an escape syntax or
must be reserved. Jisp already uses `...` inside parameter lists and list
patterns, so those remain expressible as non-leading tokens, for example:

```text
fn pack
  (head ... tail)
  list.prepend head tail
```

but a bare value expression whose source text is only `...` is not expressible
without a symbol escape.

## Recommendation

Do not promote this to the language spec yet. If explored, make it a separate
experimental reader with golden equivalence tests:

1. normalize each layout sample to explicit Lisp;
2. assert that layout and Lisp produce identical AST/IR/evaluation;
3. reject all ambiguous indentation examples with source-ranged errors;
4. require a formatter before using it in committed examples;
5. document that shell-looking tokens are still normal Jisp symbols/calls.

The design is strongest when it is "Wisp for Jisp forms". It is weakest when it
tries to become a command DSL, pipeline language, object literal syntax, and
function-definition sugar at the same time.

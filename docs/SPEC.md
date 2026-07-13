# Jisp language specification — foundation snapshot

## Goals

Jisp is an ultra-small, statically oriented Lisp with JSON-native data shapes.
Rust is an implementation backend and runner, not part of the language surface.
The compiled language has no universal dynamic `Value` representation.

## Equivalent source syntaxes

- `.json`: canonical interchange syntax.
- `.yaml`/`.yml`: restricted flow-style YAML-like syntax. Plain scalars are
  symbols; quoted scalars are strings.
- `.lisp`/`.jisp`: conventional S-expression syntax.

All readers produce the same source-aware AST.

## Canonical JSON

```text
"name"                       symbol
["str", "name"]             string
["f", "x"]                  call
["list", 1, 2]              list value
["obj", ["str", "x"], 1]   object value
["quote", form]             unevaluated syntax
["`", form]                 quasiquote
[",", expr]                 unquote
[",@", expr]                unquote-splicing
```

`["str", ...]` concatenates literal fragments. Nested unquote evaluates an
expression that must produce a string.

## Core forms

- `def`, `export`, `import`, `type`
- `fn`, `let`, `do`, `if`, `case`, `use`
- `quote`, `quasiquote`, `unquote`, `unquote-splicing`, `macro` (alias `~`)
- `.`, `and`, `or`, `not`

Arguments evaluate left-to-right. Only `false` and `null` are falsey.
Top-level executable expressions are forbidden; execution begins at `main`.

## Compile-time macros

A module can define an ordered, compile-time macro with `~` (or `macro`) around
one `fn`. Macro definitions are consumed before lowering: they do not create a
runtime value and must appear before their first use. Parameters receive raw
syntax nodes. The function body is exactly a `quote` or `quasiquote` template;
in a quasiquote, `,parameter` inserts one argument node and `,@rest` splices a
final `... rest` parameter.

```lisp test=spec.user-macro mode=run
(def unless
  (~ (fn (condition then otherwise)
       `(if ,condition ,otherwise ,then))))

(export main
  (fn ()
    (unless false 1 2)))
```

Macros can be used locally or imported explicitly at compile time with
`macro-import`. Imported macros are namespaced under the chosen alias and are
called as `alias.name`; ordinary runtime `import` does not import macro
bindings. The path-aware facade resolves `macro-import` before Core IR lowering,
so a raw `macro-import` that reaches the lowerer is an error.

```lisp
(macro-import m "macros.lisp")

(export main
  (fn ()
    (m.wrap 7)))
```

Macros cannot be exported; exporting a macro is an expansion error in every
source syntax. Transitive `macro-import` source files are tracked as module
dependencies, and transitive import cycles are rejected before expansion.
Template bindings introduced by `fn`, `let`, `use`, list-rest patterns,
aliases, and ordinary `case` pattern bindings are hygienic: each expansion gives
them a fresh internal name, while unquoted and spliced caller syntax keeps its
original spelling and scope. The macro body is not a general compile-time Jisp
evaluator; this keeps expansion deterministic and avoids a second prelude, IO,
and host capability contract. Future macro work needs sandboxing rules only if
a general compile-time evaluator is added.
The full design and future boundaries are recorded in
[`.agents/plans/0010-user-macros.md`](../.agents/plans/0010-user-macros.md) and
[`.agents/plans/0018-macro-hygiene.md`](../.agents/plans/0018-macro-hygiene.md).

## Case alias patterns

`(as pattern name)` both applies `pattern` and binds `name` to the entire
matched value. It is transparent to exhaustiveness checking, so aliases do not
make a case less complete.

```lisp test=spec.case-alias mode=run
(type response
  (ok int)
  (err int))

(export main
  (fn ()
    (case (ok 7)
      ((as (ok value) whole)
        (case whole
          ((ok repeated) (+ value repeated))
          ((err _) 0)))
      ((err _) 0))))
```

## Case guards

Wrap a pattern in `(when pattern condition)` to evaluate a boolean condition
after its bindings are available. A guarded branch does not by itself establish
exhaustiveness: retain an unguarded branch for the remaining values.

```lisp test=spec.case-guard mode=run
(export main
  (fn ()
    (case 7
      ((when value (> value 10)) 1)
      (_ 2))))
```

## Case alternatives

`(or first second ...)` matches any alternative. Every alternative must bind
the same names, so the case body is valid regardless of which one matched.

```lisp test=spec.case-or mode=run
(type response
  (ok int)
  (pending int)
  (err int))

(export main
  (fn ()
    (case (pending 7)
      ((or (ok value) (pending value)) (+ value 1))
      ((err _) 0))))
```

## Case exhaustiveness and redundancy

The current checker is intentionally conservative and only proves coverage for
domains it can enumerate cheaply:

- closed algebraic data constructors, `bool`, and `null`;
- list lengths with irrefutable rest patterns;
- exact-length list patterns whose element type has a finite domain, such as
  `bool`, `null`, or a closed algebraic data type;
- object fields whose value type has a finite domain, including nested fields
  and products of up to 256 finite field combinations.

When a finite list or object product is incomplete, diagnostics name the
missing combinations, for example `list [false]` or
`object {active: false, visible: true}`. Open structural domains still use a
conservative generic missing-pattern message.

Guarded branches are type-checked but do not contribute new exhaustiveness
coverage, because the checker does not prove guard predicates. They can still
be reported as redundant when earlier unguarded branches already cover the
whole finite domain. Keep an unguarded fallback for the remaining values.

The deferred, fuller option is a general pattern-matrix coverage algorithm. It
would model list/object shapes, nested constructors, overlapping alternatives,
and missing-pattern reporting uniformly instead of using the current
finite-domain shortcuts. That is a future compatibility target, not required by
the present language contract.

## Definitions and modules

```yaml
[def, private-name, value]
[export, public-name, value]
[export, existing-name]
[import, "std/list"]
[import, xs, "some/long/module"]
```

A directory is intended to be one module. Files in that directory share a
namespace and may use any supported syntax. Imported modules are accessed with
qualified symbols such as `list.map`.

Definition names, type names, constructors, and import aliases must each be
unique within a module. Constructors share the value namespace with
definitions. Reusing one is a lowering error rather than a shadowing rule.

## Functions and calls

Functions are values and use lexical scope. Their parameters are inferred; a
definition may have a final rest binding introduced by `...`. The rest binding
is a list of the remaining arguments, including an empty list when no
arguments remain.

```lisp test=spec.variadic-function mode=run
(def sum-rest
  (fn (head ... tail)
    (+ head (list.fold + 0 tail))))

(export main
  (fn () (sum-rest 40 1 1)))
```

Calls evaluate the callee and arguments left to right. A typed function value
can be called directly, returned, or passed to a fixed-arity callback helper.
`str.cat`, `list.cat`, and `obj.cat` are prelude functions that are themselves
variadic; see [STDLIB.md](STDLIB.md) for their signatures.

## Types

Types are inferred. User-visible Rust types, traits, borrows, and lifetimes are
not part of the language. The intended system includes parametric polymorphism,
closed algebraic data types, structural object rows, and monomorphisation.

```yaml
[type, result,
  [ok, value]
  [err, error]]
```

Constructors and patterns use the same shape.

```lisp test=spec.case mode=run
(type response
  (ok int)
  (err str))

(export main
  (fn ()
    (case (ok 42)
      ((ok value) (str "ok: " ,(str.from value)))
      ((err message) (str "failed: " ,message)))))
```

## Errors as values

Ordinary failure is represented as `result` values, not exceptions. `case`
handles variants exhaustively. `use` is a general callback-last sugar, suitable
for `result.try`, resource scopes, transactions, and parsers.

## Data

`list` corresponds to a JSON array and is expected to compile to a contiguous
collection where practical. `obj` is created with alternating key/value
arguments. Repeated statically known object keys, including keys in object case
patterns, are rejected. Raw `{}` syntax is reserved and currently rejected.

UI source uses explicit component and host-element forms such as
`(component row (title) (li (class "rounded") (text title)))`, not an `el`
escape hatch or attribute-name heuristics. The canonical set of host names and
the directive grammar are defined in [UI.md](UI.md). Lowering creates a
renderer-neutral structural node with `tag`, optional `attrs`, `props`,
`classes`, `events`, `key`, and `children`; text becomes `{tag: "text", value:
"..."}`. The prototype `ui.html` builtin renders escaped static HTML and
intentionally ignores events and keys. Reactive state, reconciliation, and
event dispatch are deferred runtime contracts.

`[., object, key]` is field/map lookup only. Jisp has no method syntax or
implicit receiver. A function stored in a field is called normally.

## Numbers

Integers are signed 64-bit values. Integer arithmetic is checked: overflow is a
runtime error in the evaluator and must remain a compile-time or runtime error
in native backends. `/` is truncating integer division for integer operands.
`//` and `%` use Euclidean division and modulo. Division or modulo by zero is an
error. `i64::MIN / -1`, `i64::MIN // -1`, and `i64::MIN % -1` are overflow
errors.

BigInts are arbitrary-precision signed integers constructed explicitly with
`(bigint "...")`, where the string is a base-10 integer. Plain integer literals
remain checked `i64` literals. BigInts support `+`, `-`, `*`, `/`, `//`, `%`,
numeric comparisons, `math.abs`, `math.min`, and `math.max`. `/` truncates
toward zero, while `//` and `%` use Euclidean division and modulo. Division or
modulo by zero is an error. `math.pow` does not currently accept bigints.

Floats are IEEE-754 `f64` values. Float arithmetic follows host `f64`
semantics except that `/`, `//`, and `%` reject a zero divisor instead of
producing infinities or NaN from division by zero. Other invalid float
operations, such as `math.sqrt(-1.0)`, may produce NaN.

Numeric operations do not implicitly coerce between integers, bigints, and
floats. A numeric builtin receives operands of one numeric type. Mixed numeric
arguments are type errors in checked code and runtime errors in the interpreter.

NaN follows `f64` equality: it is not equal to itself. Structural equality does
not normalise or canonicalise NaN payloads.

```lisp test=spec.bigint mode=run
(export main
  (fn ()
    (+ (bigint "9223372036854775808") (bigint "4"))))
```

## Object lookup

Field lookup is explicit and works on structural objects.

```lisp test=spec.object-lookup mode=run
(export main
  (fn ()
    (. (obj "name" "Ada") "name")))
```

## Maps

`obj` describes structural objects whose known keys are part of the inferred
row. Use `map` for runtime-sized homogeneous dictionaries:

```lisp test=spec.map mode=run
(export main
  (fn ()
    (case (map.get (map "primary" 40 "secondary" 2) "secondary")
      ((ok value) value)
      ((err _) 0))))
```

The source type is `(map str A)`. Native Rust uses
`indexmap::IndexMap<String, A>` for this shape. Dynamic object lookup on
heterogeneous closed rows is a type error unless the key is statically known. A
heterogeneous dynamic JSON value is not implicit in map or object lookup; if the
language needs that, it must be a future source-visible sum type consumed with
`case`.

Homogeneous closed objects can be converted explicitly to maps with
`obj.to-map` before using runtime-sized helpers such as `map.del`:

```lisp test=spec.obj-to-map mode=run
(export main
  (fn ()
    (let (scores (obj.to-map (obj "primary" 40 "secondary" 2)))
      (case (map.get (map.del scores "primary") "secondary")
        ((ok value) value)
        ((err _) 0)))))
```

## Equality and mutability

Equality is structural for data values. Functions and opaque native handles are
not comparable. Values are semantically immutable: every list/object/map update
returns a new value and leaves aliases to the input unchanged. The interpreter
currently copies the affected container; evaluators may later use COW and native
code may reuse unique allocations when observably equivalent.

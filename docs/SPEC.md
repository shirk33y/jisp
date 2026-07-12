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

Macros are currently module-local and unhygienic. They cannot be imported or
exported, and their templates use ordinary lexical name resolution after
lowering. The macro body is not a general compile-time Jisp evaluator; this
keeps expansion deterministic and avoids a second prelude, IO, and module
loading contract. The full design and future boundaries are recorded in
[`.agents/plans/0010-user-macros.md`](../.agents/plans/0010-user-macros.md).

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

The current UI proof uses ordinary objects rather than new syntax. A node has a
string `tag`, optional scalar attributes, optional `classes` object whose keys
are utility class names and whose values are booleans, and optional `children`
list. Text is represented as `{tag: "text", value: "..."}` in object form. The
prototype `ui.html` builtin renders this data shape to an escaped HTML string.

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

## Equality and mutability

Equality is structural for data values. Functions and opaque native handles are
not comparable. Values are semantically immutable: every list/object update
returns a new value and leaves aliases to the input unchanged. The interpreter
currently copies the affected container; evaluators may later use COW and native
code may reuse unique allocations when observably equivalent.

# Case alternative patterns

## Intended surface

`(or first second ...)` accepts a value matched by any alternative:

```lisp
(case response
  ((or (ok value) (pending value)) value)
  ((err message) message))
```

Every alternative must bind exactly the same names with unifiable types. An
alternative with no bindings may be combined only with alternatives that also
have no bindings. This prevents a branch body from observing a name that is
absent on a successful path.

## Inference design

Do not infer alternatives by cloning `Inferencer`: its unifier carries shared
constraints for the case subject. Instead, pattern inference needs a temporary
binding map (`name -> Type`) separate from the lexical environment. Each
alternative is inferred against the same subject type and unifier; its map is
compared with the first map, then the agreed bindings are installed once for
the case body. The existing duplicate-binding error remains local to each
alternative.

## Coverage and execution

An `or` pattern covers the union of its alternatives. Finite enum/bool/null and
the current refined list/object coverage code must flatten alternatives before
testing redundancy or completeness; it must not treat a guarded or alternative
branch as an unconditional catch-all. The evaluator tries alternatives with a
fresh binding collection and retains only the successful one. Native emission
should lower non-enum alternatives to an `||` condition with branch-local
bindings only after a common-binding representation exists; enum alternatives
can use Rust's `|` only when they introduce no bindings. Keep unsupported
native forms explicit rather than emitting unsound bindings.

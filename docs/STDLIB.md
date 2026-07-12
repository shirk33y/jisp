# Minimal standard library

Compiler primitives: arithmetic/comparison, `def`, `export`, `import`, `fn`,
`let`, `do`, `if`, `case`, `type`, quoting, boolean short-circuiting, and field
lookup. `bigint` constructs arbitrary-precision integers from decimal strings.

Initial modules:

- `math`: abs, min, max, pow, sqrt, floor, ceil, round, log, sin, cos, atan2.
  `abs`, `min`, and `max` accept bigints; `pow` does not yet.
- `str`: is, from, cat, lines, len, join, split, trim, upper, lower, has,
  starts, ends, replace, slice.
- `list`: is, len, get, first, last, rest, slice, map, filter, fold, some,
  every, has, cat, prepend, append.
- `obj`: is, len, has, get, set, del, keys, values, cat.
- `result`: map, map-err, try, recover.
- `ui`: html. `ui.html` is a prototype renderer from structural UI objects to
  escaped HTML strings; it is not a full UI framework.

Not every listed function is implemented in the evaluator yet. Keep the surface
small and add functions with tests rather than copying all of Gleam stdlib.

## Examples

`list` helpers compose with ordinary functions.

```lisp test=stdlib.list-pipeline mode=run
(export main
  (fn ()
    (list.fold
      (fn (total value) (+ total value))
      0
      (list.map
        (fn (value) (+ value 1))
        (list 1 2 3)))))
```

`use` makes callback-last result propagation direct.

```lisp test=stdlib.result-use mode=run
(def parse-count
  (fn (raw)
    (case raw
      ("two" (ok 2))
      (_ (err "bad count")))))

(export main
  (fn ()
    (use value (result.try (parse-count "two"))
      (ok (+ value 1)))))
```

`ui.html` renders structural UI data and escapes text and attributes.

```lisp test=stdlib.ui-html mode=run
(export main
  (fn ()
    (ui.html
      (obj
        "tag" "button"
        "title" "Save <draft>"
        "children" (list (obj "tag" "text" "value" "Save & close"))))))
```

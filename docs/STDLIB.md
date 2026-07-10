# Minimal standard library

Compiler primitives: arithmetic/comparison, `def`, `export`, `import`, `fn`,
`let`, `do`, `if`, `case`, `type`, quoting, boolean short-circuiting, and field
lookup.

Initial modules:

- `math`: abs, min, max, pow, sqrt, floor, ceil, round, log, sin, cos, atan2.
- `str`: is, from, cat, lines, len, join, split, trim, upper, lower, has,
  starts, ends, replace, slice.
- `list`: is, len, get, first, last, rest, slice, map, filter, fold, some,
  every, has, cat, prepend, append.
- `obj`: is, len, has, get, set, del, keys, values, cat.
- `result`: map, map-err, try, recover.

Not every listed function is implemented in the evaluator yet. Keep the surface
small and add functions with tests rather than copying all of Gleam stdlib.

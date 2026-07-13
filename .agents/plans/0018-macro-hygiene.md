# Hygienic user macros

Current `(~ (fn ...))` macros are intentionally local, ordered syntax
templates. Unquoted arguments retain caller syntax. Local template-introduced
bindings are now hygienic so literal binding identifiers from a template cannot
capture caller bindings.

## Implemented default

Hygiene applies to identifiers *introduced in binding positions by the
template*: `fn` parameters, `let` names, `use` bindings, list-rest patterns,
aliases, and ordinary case-pattern bindings. Each expansion assigns a
deterministic fresh internal name and rewrites only references in that
introduced lexical scope. Unquoted and spliced caller nodes are never renamed.
Core forms, prelude names, known constructors, object keys, and import-qualified
names are not bindings and remain literal symbols.

This is lexical hygiene, not an unrestricted compile-time evaluator. It keeps
the current template model and source syntax intact while preventing accidental
capture such as a macro-introduced `value` shadowing a caller's `value`.

## Module visibility

Macro definitions remain compile-time-only. A future module interface must
separate value exports from macro exports explicitly; ordinary runtime `import`
cannot implicitly import macros because expansion occurs before runtime/type
module resolution. The first cross-module design should use an explicit
compile-time import form, dependency tracking, cycle detection, and a fixed
expansion order.

## Acceptance tests

- Done: a template `let` binding cannot capture an unquoted caller identifier.
- Done: a caller-supplied binding keeps its original spelling and scope.
- Fresh names are deterministic for one source/module order, but never appear
  in user diagnostics; diagnostics show the template/call origin chain.
- Done: Lisp, JSON, and YAML reject exported macro bindings at expansion time,
  preserving the module-local macro contract before lowering/type checking.
- Done: Lisp, JSON, and YAML normalize to the same hygienic expanded Core IR
  for a macro-introduced binding plus caller-supplied syntax.
- Pending: macro-import cycles produce source-ranged errors once macro imports
  exist. Expansion-step limits already produce source-ranged errors.

## Non-goals

- Executing arbitrary Jisp during compilation.
- Implicit runtime import of macro definitions.
- Exposing generated fresh names as a public ABI or schema surface.

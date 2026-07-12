# Compile-time user macros

## Decision

Support module-local, ordered user macros in the existing surface form:

```lisp
(def unless
  (~ (fn (condition then else)
       `(if ,condition ,else ,then))))
```

A macro definition is consumed by the expansion phase and is never lowered as
a runtime definition. A macro must appear before its first use in the same
module. Imports and exports do not carry macro bindings in this first version.
That preserves the resolver contract and avoids an implicit cross-module macro
evaluation protocol.

## Evaluation boundary

The macro marker wraps a single `fn`. Its body must be exactly one `quote` or
`quasiquote` expression. Macro parameters bind to the call's *raw syntax
nodes*, not values. A final `... rest` parameter binds to all remaining syntax
nodes. In a quasiquote, `,name` inserts one bound syntax node and `,@name`
splices the bound rest nodes. `quote` returns literal syntax.

This is deliberately not a second evaluator. Running arbitrary Jisp at
expansion time would require values for syntax, a separate prelude/IO security
boundary, module loading semantics, and a phase-aware type contract. Those
belong in a later, explicit macro-runtime design rather than in a hidden
implementation detail.

## Diagnostics and provenance

Malformed macro definitions, unknown macro parameters, arity mismatches, and
invalid splice positions are `JISP-EXPAND` diagnostics. Generated syntax is
recorded as originating at the macro invocation, then expanded recursively, so
existing detailed facade diagnostics can follow nested expansion chains.

## Hygiene and follow-up

This version is intentionally unhygienic: symbols introduced by a template use
ordinary lexical resolution after lowering. Macro authors should use local
bindings carefully. Hygienic identifiers, imported/exported macros, and a
general compile-time evaluator require a separate phase/name-resolution design.

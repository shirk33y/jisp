# Gleam reference

Jisp uses the Gleam compiler as a design reference for a small, statically typed,
friendly language implemented in Rust. This file tracks what is ported or closely
inspired so the relationship stays explicit.

Reference checkout:

- Repository: `https://github.com/gleam-lang/gleam.git`
- Commit: `833732c523441043868877d159988ba2d21538cd`
- Local checkout: `~/stuff/gleam`
- CMM project: `home-shirk3y-stuff-gleam`

No Gleam source code is vendored in this repository.

## Ported or inspired features

| Feature | Jisp status | Gleam reference | Rationale |
| --- | --- | --- | --- |
| Algebraic data types and constructors | Ported in the Core IR and evaluator; type inference now registers constructor schemes from `type` declarations. | [`prelude.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/prelude.rs#L425-L436), [`environment.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/environment.rs#L499-L544) | ADTs give Jisp precise user data without a catch-all dynamic value model in compiled output. |
| `Result`-style errors as values | Planned as a first-class stdlib convention; current enum machinery can model it. | [`prelude.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/prelude.rs#L28-L61), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L698-L705) | Keeps ordinary failures visible in types and avoids exception-driven control flow in portable Jisp code. |
| `case` expressions over typed patterns | Runtime support exists; static branch typing is partial; exhaustiveness is P0. | [`exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/exhaustiveness.rs#L65-L87), [`missing_patterns.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/exhaustiveness/missing_patterns.rs#L11-L24) | Exhaustive matching is the main safety payoff of ADTs and should produce source-ranged, actionable diagnostics. |
| Hindley-Milner-style inference with an explicit type environment | Partially ported in `jisp-types` for core expressions, modules, let-generalisation, and constructors. | [`environment.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/environment.rs#L38-L63), [`hydrator.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/hydrator.rs#L30-L47), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L409-L420) | Jisp should infer common code without annotations while keeping a stable typed seam for evaluation and Rust codegen. |
| Top-level dependency ordering and recursive SCCs | Ported as a small Jisp-local dependency pass in `jisp-types`; independent top-level definitions are generalised before dependents, while recursive groups stay monomorphic until the group is solved. | [`call_graph.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/call_graph.rs#L530-L587), [`type_.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_.rs#L1732-L1779), [`analyse.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/analyse.rs#L1924-L2009) | This keeps recursive functions possible without preventing polymorphic helpers from being reused at multiple types in later definitions. |
| Module graph, imports, stale tracking, and cycle checks | Partially ported for facade/type-checking imports and cycle detection; CLI/proc-macro dependency tracking remains. | [`module_loader.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/module_loader.rs#L45-L84), [`project_compiler.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/project_compiler.rs#L105-L151), [`call_graph.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/call_graph.rs#L530-L544) | Directory-as-module loading needs deterministic resolution, useful cycle errors, and future incremental compilation. |
| Source-ranged diagnostics | Ported as source-aware AST and diagnostic foundations; rendering needs more work. | [`diagnostic.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/diagnostic.rs), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L5355-L5395) | Multi-syntax input only works if errors stay attached to original source spans through parsing, lowering, macros, and typing. |
| Immutable values with backend-friendly representation | Partially ported in evaluator/runtime helpers; native ABI remains intentionally undesigned. | [`typed.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/ast/typed.rs), [`project_compiler.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/project_compiler.rs#L202-L238) | Runtime semantics should remain portable while Rust codegen gets a typed representation instead of mirroring interpreter internals. |

## Agent response review

The agent response recommending Gleam as a source of compiler design patterns
was reviewed against the pinned checkout on commit
`833732c523441043868877d159988ba2d21538cd`. The recommendation is accepted with
one boundary: Jisp should borrow seams and invariants, not copy Gleam's full
package/compiler architecture.

Accepted findings:

- ADT constructor schemes, top-level SCC grouping, and finite-domain
  exhaustiveness are the right P0 imports from Gleam's type checker.
- Import/type-environment installation should stay explicit and module-path
  keyed. Jisp already has the facade/type-checking side of this; native
  compilation still needs dependency tracking at the same seam.
- Source-ranged missing-pattern and type-error diagnostics are worth matching in
  spirit because Jisp has multiple syntaxes and macro expansion.
- The response correctly separates small compiler invariants from heavyweight
  build-system machinery.

Status after review:

- Already ported: ADT constructor schemes, top-level recursive SCC grouping,
  module import environments, mixed-syntax resolver behavior, and finite
  `bool`/`null`/variant exhaustiveness foundations.
- Still actionable in P0: richer `case` checking, source-range rendering through
  macro origins, and remaining stdlib schemes.
- Later work: native compiler dependency tracking should use the existing
  resolver seam rather than a second import implementation.

Deferred findings:

- Gleam's full package/module loader, stale tracking, and incremental build
  machinery.
- Opaque/private constructor policy and full module interface publication.
- Exhaustiveness over every pattern family before Jisp's object/list pattern
  typing is more complete.
- Full row-polymorphic/object-heavy typing and advanced overload machinery.
- Diagnostic polish that depends on richer renderer and source plumbing.

Current risk boundary:

- Jisp's top-level SCC pass is intentionally definition-local. Imports and type
  constructors are installed before grouping; future module work should preserve
  that ordering.
- `let` bindings still generalise immediately, while top-level definitions use
  placeholders and group-level generalisation. Keep this split explicit if new
  binding forms are added.
- Richer Gleam-style exhaustiveness should wait until list/object pattern
  semantics are specified enough to avoid coupling diagnostics to unstable IR.

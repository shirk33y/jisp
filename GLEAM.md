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
| Algebraic data types and constructors | Ported in the Core IR and evaluator; type inference registers constructor schemes from `type` declarations, including zero-field variants and imported constructor schemes. | [`prelude.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/prelude.rs#L425-L436), [`environment.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/environment.rs#L499-L544), [`tests.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests.rs#L2036-L2059) | ADTs give Jisp precise user data without a catch-all dynamic value model in compiled output. |
| `Result`-style errors as values | Planned as a first-class stdlib convention; current enum machinery can model it. | [`prelude.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/prelude.rs#L28-L61), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L698-L705) | Keeps ordinary failures visible in types and avoids exception-driven control flow in portable Jisp code. |
| `case` expressions over typed patterns | Runtime support exists; static branch typing covers core pattern bindings; exhaustiveness covers finite ADT/bool/null domains and conservative list/object irrefutable-pattern coverage. | [`exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/exhaustiveness.rs#L65-L87), [`missing_patterns.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/exhaustiveness/missing_patterns.rs#L11-L24) | Exhaustive matching is the main safety payoff of ADTs and should produce source-ranged, actionable diagnostics. |
| Pattern exhaustiveness and redundancy | Partially ported for finite domains, catch-all redundancy, repeated literals/constructors, empty cases, and nested list/object pattern typing; guards, alternatives, string prefixes, and multi-subject cases remain future design work. | [`exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/exhaustiveness.rs#L822-L920), [`exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/exhaustiveness.rs#L1482-L1854), [`clause_guard_test.gleam`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/test/language/test/language/clause_guard_test.gleam#L4-L220) | Pattern safety needs its own coverage because syntax, type checking, reachability, diagnostics, runtime matching, and future codegen can regress independently. |
| Hindley-Milner-style inference with an explicit type environment | Partially ported in `jisp-types` for core expressions, modules, let-generalisation, and constructors. | [`environment.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/environment.rs#L38-L63), [`hydrator.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/hydrator.rs#L30-L47), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L409-L420) | Jisp should infer common code without annotations while keeping a stable typed seam for evaluation and Rust codegen. |
| Top-level dependency ordering and recursive SCCs | Ported as a small Jisp-local dependency pass in `jisp-types`; independent top-level definitions are generalised before dependents, while recursive groups stay monomorphic until the group is solved. | [`call_graph.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/call_graph.rs#L530-L587), [`type_.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_.rs#L1732-L1779), [`analyse.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/analyse.rs#L1924-L2009) | This keeps recursive functions possible without preventing polymorphic helpers from being reused at multiple types in later definitions. |
| Module graph, imports, stale tracking, and cycle checks | Ported for facade/type-checking imports, cycle detection, CLI dependency listing, and proc-macro dependency tracking; native token emission remains P1. | [`module_loader.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/module_loader.rs#L45-L84), [`project_compiler.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/project_compiler.rs#L105-L151), [`call_graph.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/call_graph.rs#L530-L544) | Directory-as-module loading needs deterministic resolution, useful cycle errors, and future incremental compilation. |
| Module visibility and private API boundaries | Partially planned; exported-only import visibility exists, while private-type leak prevention and interface publication remain future work. | [`errors.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/errors.rs#L835-L892), [`dead_code_detection.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/dead_code_detection.rs#L197-L258) | Native compilation and packages need a stable public API boundary rather than exposing every constructor or structural detail by accident. |
| Source-ranged diagnostics | Ported as source-aware AST, diagnostic foundations, CLI rendering for parser/lowerer errors, and macro-origin labels through `ExpansionMap`; primary/secondary labels and constructor/type mismatch hints remain a quality target. | [`diagnostic.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/diagnostic.rs#L88-L135), [`expression.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/expression.rs#L5355-L5395), [`exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/exhaustiveness.rs#L1130-L1157) | Multi-syntax input only works if errors stay attached to original source spans through parsing, lowering, macros, and typing. |
| Immutable values with backend-friendly representation | Partially ported in evaluator/runtime helpers; native ABI remains intentionally undesigned. | [`typed.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/ast/typed.rs), [`project_compiler.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/build/project_compiler.rs#L202-L238) | Runtime semantics should remain portable while Rust codegen gets a typed representation instead of mirroring interpreter internals. |

## `case` test reference

Gleam treats `case` as a compiler-wide seam, not just a type-checker feature:
parser errors, subject/pattern unification, exhaustiveness, redundancy warnings,
code generation, scope behavior, and editor actions all have focused tests.

Relevant pinned test surfaces:

- [`compiler-core/src/type_/tests/exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/exhaustiveness.rs#L1048-L1315) covers empty and inexhaustive `case` expressions, including finite and open domains.
- [`compiler-core/src/type_/tests/exhaustiveness.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/exhaustiveness.rs#L1482-L1850) covers unreachable and redundant patterns, alternatives, prefixes, and overlapping branches.
- [`compiler-core/src/type_/tests/errors.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/errors.rs#L288-L299) covers subject and pattern type disagreement.
- [`compiler-core/src/type_/tests/warnings.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/type_/tests/warnings.rs#L2040-L2064) covers unreachable code when a `case` subject diverges.
- [`compiler-core/src/parse/tests.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/parse/tests.rs#L1189-L1212) covers malformed `case` syntax before type checking.
- [`compiler-core/src/javascript/tests/case.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/compiler-core/src/javascript/tests/case.rs#L451-L790) covers backend output for list, tuple, record, string, alias, and label patterns.
- [`language-server/src/tests/action.rs`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/language-server/src/tests/action.rs#L3008-L3155) covers editor fixes for adding missing patterns and removing unreachable clauses.
- [`test/language/test/language/directly_matching_case_subject_test.gleam`](https://github.com/gleam-lang/gleam/blob/833732c523441043868877d159988ba2d21538cd/test/language/test/language/directly_matching_case_subject_test.gleam#L17-L92) covers subject binding and scope behavior around `case`.

The portable lesson for Jisp is that `case` tests should be grouped by compiler
boundary: parser shape, IR lowering, type unification, exhaustiveness,
redundancy, runtime behavior, backend output, and future editor actions. The
highest-value test ideas to port are empty cases over finite and open subjects,
duplicate and overlapping alternatives, guarded branches that keep later arms
reachable, nested list/object/constructor patterns, imported constructor names
under aliasing and shadowing, and exact source ranges for pattern type errors.

## Jisp test backlog from Gleam

P0-compatible tests, because they match current Jisp semantics:

- `case` over imported ADTs with qualified constructors and inferred branch
  result type.
- exhaustive `case` over `result` and `option`-like enums, including zero-field
  variants;
- redundant branch detection for catch-all patterns, repeated constructors, and
  repeated literals;
- refined `case` coverage over finite list items and object fields, including
  nested fields, duplicate refinements, exact-length list refinements, and
  conservative handling of enum variants with payload patterns;
- pattern and subject type mismatches with source-ranged diagnostics;
- nested list, object, and constructor pattern bindings;
- `case` arm bindings that do not leak out or mutate outer lexical bindings;
- recursive top-level groups where recursive SCCs stay monomorphic but
  independent helpers still generalise before dependents;
- import aliasing and shadowing around constructor names.

Future tests, because they need feature design before implementation:

- guarded `case` branches and guard-sensitive reachability;
- alternative patterns and overlap diagnostics;
- multi-subject `case`;
- `case` in every expression position that codegen must preserve, including
  assignment, calls, and pipelines;
- `let assert`-style destructuring if Jisp adopts an assertion-binding form;
- string-prefix pattern matching;
- bit-array pattern matching with typed segments, nested data, and endian-aware
  encodings if Jisp adopts bit-array syntax/runtime support;
- non-UTF8 string/bit-array interop if encoding-aware matching becomes part of
  the language;
- tuple/record-specific access and update semantics beyond current object rows;
- private type leak diagnostics for public module APIs;
- editor-code-action tests for adding missing patterns and removing unreachable
  clauses;
- backend-specific matching/codegen shape checks once native Rust emission has
  typed IR.

## Agent response review

An agent response recommending Gleam as a source of compiler design patterns was
reviewed against the pinned checkout on commit
`833732c523441043868877d159988ba2d21538cd`. The response is directionally
correct, but only as a reference review: Jisp should borrow compiler seams,
invariants, and diagnostic shape, not Gleam's full package/compiler
architecture.

Review verdict:

- Correct: Gleam is a strong reference for ADT constructor environments,
  Hindley-Milner inference, recursive top-level grouping, pattern
  exhaustiveness, and precise diagnostics.
- Correct: the useful ideas live mostly below surface syntax. They belong in
  shared IR, type, resolver, diagnostic, and codegen seams rather than in Lisp,
  JSON, or YAML parser crates.
- Needs narrowing: Gleam's project compiler, package manager assumptions,
  stale-module tracking, and backend-specific codegen should remain deferred
  unless a Jisp feature directly requires them.
- Needs local ownership: imported behavior should be adapted to Jisp's syntax
  normalization, multi-source modules, and portable fixture tests instead of
  copied wholesale.

Claim-level review:

- ADTs, constructors, and enum-like user data are valid Gleam imports for Jisp.
  The useful part is the split between type constructors, value constructors,
  and constructor schemes installed in the type environment.
- `case` is a valid reference point, but Gleam's checker should be treated as
  an exhaustiveness and diagnostic model rather than a syntax model. Jisp still
  needs local rules for list, object, and multi-syntax pattern shapes.
- Module loading and stale tracking are only partially relevant. The agent
  response is correct that dependency tracking matters, but Jisp should route
  that through the existing facade resolver instead of adopting Gleam's full
  project compiler.
- Variadic stdlib schemes and object-row typing are not direct Gleam ports.
  They remain Jisp-owned type-system work; Gleam can inform the environment and
  diagnostic structure, not the exact type representation.
- Macro hygiene is not substantially answered by Gleam. Any quote,
  quasiquote, unquote, and splicing design should be documented and implemented
  in Jisp's macro/origin pipeline.

Accepted findings:

- ADT constructor schemes, top-level SCC grouping, and finite-domain
  exhaustiveness are the right P0 imports from Gleam's type checker. These map
  to Jisp's existing `jisp-types` inference seam rather than to parser crates.
- Import/type-environment installation should stay explicit and module-path
  keyed. Jisp now has the facade/type-checking, CLI listing, and proc-macro
  dependency tracking side of this; native token emission remains P1.
- Source-ranged missing-pattern and type-error diagnostics are worth matching in
  spirit because Jisp has multiple syntaxes and macro expansion. The important
  part is preserving origin ranges and explaining the failed invariant, not
  matching Gleam's exact message text.
- The response correctly separates small compiler invariants from heavyweight
  build-system machinery.

Adoption decisions:

- Keep Gleam-style constructor/value environments as the model for user ADTs and
  stdlib enum conventions such as `result` and `option`.
- Keep Jisp's facade resolver as the only source of module-loading truth. CLI,
  proc-macro, and native compilation should consume dependency information from
  that seam rather than implement their own import scanners.
- Use Gleam's exhaustiveness work as the reference for missing-pattern and
  redundant-pattern diagnostics, but only after each Jisp pattern family has
  stable typing semantics.
- Use Gleam's diagnostic style as a quality bar: errors should name the failed
  invariant, point at original source ranges, and preserve macro-origin context.

Status after review:

- Already ported: ADT constructor schemes, top-level recursive SCC grouping,
  module import environments, mixed-syntax resolver behavior, exported-only
  import visibility, proc-macro dependency tracking through the facade resolver,
  finite `bool`/`null`/variant exhaustiveness foundations, conservative
  list/object exhaustiveness for irrefutable patterns, and
  variadic function schemes for lambda rest parameters plus runtime-variadic
  `str.cat`/`list.cat`, with broad object-row schemes for runtime object
  helpers.
- Still actionable in P0: richer `case` checking and structural object
  refinements.
- Later work: native compiler token emission should consume the dependency
  information already exposed by the facade instead of adding a second import
  implementation.

Deferred findings:

- Gleam's full package/module loader, stale tracking, and incremental build
  machinery.
- Opaque/private constructor policy and full module interface publication.
- Exhaustiveness over every pattern family before Jisp's object/list pattern
  typing is more complete.
- Full row-polymorphic/object-heavy typing and advanced overload machinery.
- Backend-specific assumptions from Gleam's Erlang and JavaScript codegen.
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

# Jisp roadmap

Jisp is a compiler foundation for statically checked, JSON-shaped programs.
The interpreter is the reference execution path; native Rust emission is a
deliberately smaller, concrete-type subset. This roadmap describes direction,
not dates. The detailed engineering queue remains [TODO.md](TODO.md).

## Current baseline

- One source-aware frontend accepts Lisp, `ws`, canonical JSON, and restricted
  YAML-like syntax and lowers them to the same typed Core IR.
- The interpreter supports immutable data, algebraic data types, imports,
  pattern matching, results/options, bigint values, maps, and the complete
  prelude.
- Native Rust generation supports the P2 monomorphic subset: typed functions
  and function values, captured closures, user-defined variadics, lists, closed
  objects, explicit homogeneous maps, selected `case` patterns, imports,
  diagnostics remapping, proc-macro expression/item integration, and selected
  list/result/object/map helpers. It rejects unsupported programs instead of
  using a universal dynamic runtime value.
- `jisp fmt`, `jisp repl --state`, `jisp lsp`, manifests, local path
  dependencies, offline registry cache entries, and deterministic lockfiles are
  available as deliberately bounded project tooling.
- The UI runtime has a typed JUIR compiler and structural executor, portable
  reducer/resource tests, a browser patch host, SSR hydration checks, and a
  narrow WIT capability boundary. The structural tree remains the conformance
  oracle.

## Next: harden the complete P2 surface

1. **Conformance depth.** Grow interpreter-versus-native differential,
   compile-fail, and executable documentation tests around every supported
   value shape and helper.
2. **Make the supported native subset legible.** Publish its support matrix,
   compatibility policy, and intentional rejection boundaries alongside the
   conformance suite.
3. **Package, editor, and diagnostic polish.** Keep local/offline registry
   behavior deterministic, improve lock/cache and LSP diagnostics, and defer
   network fetching until the trust and checksum policy is implemented end to
   end.

## Then: extend language seams deliberately

1. **User macros.** Template macros now have hygiene, origin diagnostics, and
   path-aware namespaced `macro-import`. Add a general compile-time evaluator
   only after its sandboxing, dependency, determinism, and capability contract
   is designed.
2. **Pattern matching.** The current checker covers finite enum/bool/null,
   finite list, finite object-product, aliases, alternatives, and conservative
   guards. A full pattern-matrix algorithm for arbitrary structural domains is
   a future compatibility target.
3. **Dynamic structural data.** Keep `map<str, A>` as the homogeneous runtime
   dictionary. Native open rows and heterogeneous dynamic selection require a
   source-visible type/ABI proposal before implementation.
4. **Portable UI hosts.** Keep the completed JUIR/effect contract as the
   semantic baseline. A native renderer, Jispwind utility profile, richer host
   capabilities, and server-delivery adapters each need a bounded design and
   conformance gate before they become product claims.

## Tooling and project workflow

- Maintain JSON Schema generation from resolved module information, not just
  core syntax.
- Keep formatter, REPL, LSP, and package tooling compatible with the shared
  module and diagnostic contracts as those contracts evolve.
- Keep runnable documentation examples in the normal test suite and preserve
  equivalence across all three source syntaxes.
- Treat cross-host execution as a protocol and conformance problem before
  adding bindings; see [the MAL research report](docs/research/MAL.md).

## Intentionally deferred

- Raw `{}` remains rejected in canonical JSON and YAML. The proposed
  `json-data-v1` / `yaml-data-v1` profiles in
  [the data-dialect research](docs/research/JSON_DATA_DIALECTS.md) are not a
  language change until their source-profile contract and conformance corpus
  are accepted.
- FFI and native bindings require a written ABI, ownership, error, and binding
  generation design first; see [docs/FFI_FUTURE.md](docs/FFI_FUTURE.md).
- Runtime `eval`, classes, methods, a general dynamic `any`, and garbage
  collection are not planned for the core language.
- Remote registry network lookup/downloads are deferred until they have a
  complete trust, checksum, lockfile, and offline-cache design.

## How priorities are chosen

Prefer work that strengthens a shared seam: Core IR, type inference, runtime
semantics, source diagnostics, or the concrete native ABI. A feature is done
only when its contract, tests, and relevant documentation agree. Consult
[TODO.md](TODO.md) for the ordered, implementation-level backlog.

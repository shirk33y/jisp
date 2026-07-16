# Rust Lisp implementations: targeted inspiration

Status: research snapshot, 2026-07-16. This report changes no Jisp contract.

## Scope

Reviewed general-purpose Lisp/Scheme implementations written in Rust. Selection
used current GitHub popularity plus practical relevance to Jisp: embedding,
modules, diagnostics, native compilation, or resource bounds.

| Project | Stars at review | Checked commit | Why included |
| --- | ---: | --- | --- |
| [Steel](https://github.com/mattwparas/steel) | 2,479 | `dec633b` | largest active embedded Scheme implementation |
| [ClojureRS](https://github.com/clojure-rs/ClojureRS) | 978 | `cb64b43` | Clojure-style immutable values and namespaces |
| [Ketos](https://github.com/murarth/ketos) | 770 | `0112875` | explicit execution restriction model |
| [scheme-rs](https://github.com/maplant/scheme-rs) | 323 | `dc7ca08` | active Scheme, spans, LSP, compiler pipeline |
| [Stak](https://github.com/raviqqe/stak) | 134 | `95de673` | active embeddable R7RS compiler and benchmark suite |

Shallow checkouts live outside this repository at
`/home/codex/work/research/rust-lisps/`. Each was indexed with
`~/bin/agent-cmm` before review.

## Worth adopting as a pattern

### 1. Named sandbox profile and resource budget

Steel constructs a sandboxed engine from a restricted prelude and disables
dynamic-library loading. Ketos exposes one `RestrictConfig` with time, call and
value stack, namespace, memory, integer-size, and syntax-depth limits.

Jisp should use this pattern only for a future `jisp-wire` runner or a future
compile-time evaluator: define a named capability profile and explicit limits;
test every denied capability and exceeded budget. Do not add ambient host access
to ordinary `check`, `run`, codegen, or proc-macro expansion.

- [Steel sandbox constructor](https://github.com/mattwparas/steel/blob/dec633b908afeafeaf62bab457a92e2bf873745a/crates/steel-core/src/steel_vm/engine.rs#L1289-L1309)
- [Ketos restriction configuration](https://github.com/murarth/ketos/blob/011287590ebeb6e6a199e34c8b9da14e2daeb1ce/src/ketos/restrict.rs#L41-L65)
- [Ketos time-limit check](https://github.com/murarth/ketos/blob/011287590ebeb6e6a199e34c8b9da14e2daeb1ce/src/ketos/exec.rs#L729-L739)

### 2. Explicit module resolver boundary

Steel installs a host-supplied `ModuleResolver` through an explicit engine API.
This is compatible with Jisp's direction: language imports stay language
imports; host modules, registries, and bindings require a declared resolver or
capability contract.

- [Steel resolver registration](https://github.com/mattwparas/steel/blob/dec633b908afeafeaf62bab457a92e2bf873745a/crates/steel-core/src/steel_vm/engine.rs#L662-L668)

### 3. Benchmark the public embedding seam

Stak benchmarks an actual embedding API, including comparison workloads. Its
proc macro also tracks source changes with `include_str!`.

Add Jisp baselines after native conformance: cold parse/check/eval, proc-macro
expansion, native compile/run, and public embedding calls. Benchmark public
facade/proc-macro paths, not private implementation functions.

- [Stak embedding benchmark](https://github.com/raviqqe/stak/blob/95de673518d213a9f7a458d18c765fce1fb26a8d/bench/benches/embed.rs#L64-L73)
- [Stak proc-macro dependency tracking](https://github.com/raviqqe/stak/blob/95de673518d213a9f7a458d18c765fce1fb26a8d/macro/src/lib.rs#L89-L102)

### 4. Keep spans central to editor work

scheme-rs converts retained source spans directly to LSP ranges. Jisp already
has source-aware ASTs, expansion origins, and generated-Rust source maps. The
useful lesson is operational: add LSP and diagnostic regression fixtures before
adding more editor features.

- [scheme-rs span-to-range conversion](https://github.com/maplant/scheme-rs/blob/dc7ca0800f2aef31053653a1df64cad682bae5ab/src/lsp/document.rs#L113-L124)

## Do not copy

| Project pattern | Why Jisp should avoid it |
| --- | --- |
| Dynamic VM value as universal native boundary | Jisp's native ABI must keep concrete typed layouts and explicit rejection. |
| ClojureRS linked persistent map representation | It demonstrates immutable `assoc`, but Jisp already chose COW/concrete layouts; changing representation would widen the ABI without proving a need. |
| Full Scheme runtime features: `eval`, GC-driven host values, broad FFI | They conflict with Jisp's explicit deferred-feature and capability boundaries. |
| Steel's broad dynamic extension surface | Useful as host-API inspiration, not as portable Jisp semantics. |

- [ClojureRS immutable map update](https://github.com/clojure-rs/ClojureRS/blob/cb64b43409542f575d42d479a9a0b35c2132b950/src/persistent_list_map.rs#L120-L122)

## Recommendation

1. Execute the native conformance and example plan first.
2. Add public embedding benchmarks next.
3. Write a `jisp-wire` capability/budget design before implementing a runner.
4. Keep FFI, remote registry, general compile-time evaluation, and dynamic ABI
   values out of scope until their own contracts are accepted.

# MAL, miniMAL, and multi-host execution

## Decision summary

Jisp should retain JSON as its canonical source representation and Rust as the
only source of truth for full language semantics. JSON removes the need for a
hand-written S-expression lexer, parser, and printer in every host language; it
does **not** remove the need for a typed AST decoder, evaluator or compiler,
diagnostics, module resolver, macro system, value codec, or security boundary.

The practical portability strategy is layered:

1. publish a versioned Jisp JSON AST and JSON Schema;
2. run the canonical Rust implementation behind a stable stdio protocol for
   every host language;
3. add native extensions only for high-value hosts with mature extension
   tooling; and
4. consider independent interpreters or WebAssembly components only when their
   offline, deployment, or latency benefits justify their long-term conformance
   cost.

Do not try to reproduce MAL's full set of host implementations as an initial
product goal. A host that can launch a process and encode/decode JSON can use
Jisp correctly through the canonical runner. That reaches far more environments
than native FFI while avoiding semantic forks.

## Scope and sources reviewed

This report is based on shallow local clones made on 2026-07-12:

| Project | Revision reviewed | What it demonstrates |
| --- | --- | --- |
| [`kanaka/mal`](https://github.com/kanaka/mal) | `2bbfaa54cca4908efc90b4173b1406e260788e8a` | A pedagogical Lisp independently implemented across many host languages. |
| [`kanaka/miniMAL`](https://github.com/kanaka/miniMAL) | `c64353fa9d2da4f279c0611586077fb1c3fe4039` | A compact JSON-source Lisp with JavaScript interop and small JavaScript, Python, and ClojureScript implementations. |

MAL's README describes 11 incremental, testable interpreter steps and 89
languages, 95 implementations, and 118 runtime modes. The checked-out tree has
97 implementation directories because several languages and runtime modes have
more than one directory. This is evidence both that a small Lisp can be hosted
widely and that each port carries a substantial maintenance surface. See the
[MAL README](https://github.com/kanaka/mal#description) and its
[reader step](https://github.com/kanaka/mal/blob/master/process/guide.md#step-1-read-and-print).

miniMAL makes the opposite trade-off: JSON arrays are the program form and a
Node or browser library evaluates an already decoded host array. Its README
shows that the same representation can be executed from a host application.
However, miniMAL is deliberately Lisp-0: an ordinary JSON string is interpreted
as a symbol, while a quoted form is required to create a string literal. See
[miniMAL's JSON-source and embedding examples](https://github.com/kanaka/miniMAL#usage).

## What JSON removes, and what it does not

### Work removed

For the canonical `.json` syntax, a host can rely on a mature JSON parser and
serializer. It no longer needs a custom tokenizer for parentheses, quote marks,
comments, escaping, numeric literals, or delimiter matching. It also gets
standard tooling for syntax highlighting, transport, storage, inspection, and
schema validation.

This is especially valuable for the large tail of MAL languages: many can parse
JSON but would need a bespoke reader and printer for an S-expression syntax.
MAL's own guide notes that adapting a JSON encoder/decoder can cover much of
its reader step; Jisp makes that an architectural property rather than a
shortcut.

### Work retained

A JSON parser produces generic arrays, objects, strings, booleans, nulls, and
numbers. It does not know Jisp's language semantics. Every complete host still
needs to perform these tasks:

- distinguish an identifier from a string literal; Jisp deliberately uses
  `["str", ...]` for string literals in canonical JSON, avoiding miniMAL's
  string/symbol ambiguity;
- decode arrays and objects into a source-aware AST, validate special-form
  arities, and preserve JSON Pointer/source locations for diagnostics;
- resolve imports, expand quote/quasiquote forms and future user macros, lower
  to Core IR, infer types, and evaluate or compile;
- define exact numeric semantics. A JavaScript JSON decoder's `Number` cannot
  represent all Jisp `i64` values exactly, and JSON has no bigint or non-finite
  float value;
- represent Jisp-only values such as variants, results, options, and errors at
  an external boundary; and
- define resource limits and permissions before untrusted code can read files,
  start processes, or call the network.

Therefore, JSON removes a *syntax reader/writer implementation*, not a language
implementation. For a full independent port, parser savings are meaningful but
usually smaller than evaluator, type system, runtime, diagnostics, import, test,
and release maintenance.

## Jisp versus miniMAL

| Concern | miniMAL | Jisp | Consequence |
| --- | --- | --- | --- |
| Canonical program representation | JSON arrays/objects | Canonical JSON plus Lisp and restricted YAML-like readers | Both are easy to transport; Jisp keeps human-friendly alternatives without changing Core IR. |
| Identifier versus literal string | Same JSON string namespace; quote creates a literal string | `["str", ...]` is an explicit literal string node | Jisp is clearer for schemas, data tooling, and host adapters. |
| Host integration | Direct access to JavaScript globals and methods | Native Rust subset plus deliberately deferred FFI | miniMAL is convenient but couples program behaviour to one host. Jisp should make host effects explicit. |
| Type contract | Dynamic | Statically oriented Core IR and inference | Jisp needs a conformance contract beyond JSON shape. |
| Portability goal | Tiny implementations and teaching | Consistent language, typed native codegen, and interoperable data | Jisp should optimize for one semantic reference, not code golf in every host. |

The important miniMAL lesson is positive: host programs can evaluate a JSON AST
without an S-expression parser. The important warning is that direct host-global
interop makes a program's meaning depend on JavaScript. That is unsuitable as
the default model for portable Jisp modules.

## Deployment models

| Model | Description | Strengths | Costs and limits | Recommendation |
| --- | --- | --- | --- | --- |
| Canonical process runner | A host starts `jisp serve --stdio` or `jisp run` and exchanges JSON. | One evaluator, type checker, diagnostics, and release artifact; works from nearly any MAL host with process and JSON support. | Process startup, serialization, deployment of the runner, no direct host object or closure sharing. | **First portable integration.** |
| Long-lived JSON-RPC worker | The host maintains a Jisp process and uses request/response messages over stdin/stdout or a socket. | Amortizes startup, supports concurrency/cancellation design, language-neutral protocol. | Must version protocol, handle worker lifecycle, timeouts, output framing, and untrusted input. | **Preferred API for Node ↔ Python/Jisp-style orchestration.** |
| Native extension | Bind the Rust runtime through N-API, Python extension APIs, JNI, .NET hosting, Ruby/PHP extension APIs, or a C ABI. | Low latency, direct memory-adjacent calls, natural host package experience. | Per-host build/distribution, ABI/ownership/panic rules, version matrix, and larger attack surface. | **Use selectively after a written FFI design.** |
| Independent host interpreter | Implement Jisp semantics in JavaScript, Python, Go, JVM, etc., reading the canonical JSON AST. | Offline use, no child process, host-native debugging and deployment. | The MAL maintenance problem returns: semantic drift, test parity, macro/type/runtime work per host. | **Only for a few proven high-value hosts.** |
| Host-target transpiler | Compile a typed Jisp subset to each host language. | Potentially fast and idiomatic integration. | A codegen backend and generated-diagnostic contract for every target; host semantic differences leak in. | **Not now.** |
| WebAssembly component | Package the canonical runtime as a Wasm component with a WIT interface. | Portable binary distribution and typed component boundaries for modern hosts. | Requires component-capable runtimes and adapters; does not cover all MAL languages. | **Evaluate later, alongside the process runner.** |

[JSON-RPC 2.0](https://www.jsonrpc.org/specification) supplies a small,
well-known request/response envelope with method names, structured parameters,
request identifiers, and error responses. It is suitable for the process-worker
model; transport framing should be JSON Lines for stdio or a documented
length-prefixed/socket transport. It is not an FFI and must not be treated as a
way to smuggle arbitrary host objects across the boundary.

The [WebAssembly Component Model](https://component-model.bytecodealliance.org/)
is an interoperable component architecture. Its WIT language defines interface
contracts, including lists, options, results, records, variants, and resources.
It is promising for JS, Python, Rust, Go, C#, and other modern component hosts,
but it should be an additional distribution target, not the compatibility plan
for every language represented by MAL.

## Recommended portable boundary

### Separate program, data, and wire values

Do not use one untagged JSON shape for all three concepts.

1. **Program AST** is Jisp source. It follows the canonical JSON syntax and
   carries a language version. Strings are explicit AST nodes such as
   `["str", "hello"]`.
2. **Data JSON** is ordinary user data validated against an application schema.
   It should remain ordinary JSON when that is sufficient, rather than being
   polluted with execution or host-object tags.
3. **Wire values** cross a process, FFI, or Wasm boundary. They need a separate,
   versioned codec because Jisp values are richer than JSON.

An illustrative wire-value convention is:

```json
{
  "protocol": "jisp-wire/1",
  "value": {
    "$jisp": "result",
    "tag": "ok",
    "value": {"$jisp": "bigint", "decimal": "9223372036854775808"}
  }
}
```

This is a design sketch, not a committed syntax. Before implementation, decide
how application objects escape a reserved tag, whether every integer is tagged
or only values beyond a safe range, and how floats such as `NaN` are represented.
Functions, closures, host handles, and mutable resources should not be serialised
at all. They belong to a local capability interface.

### Version the AST and schema

Expose a stable, named schema URI and a language version in every transport
request. The current Jisp core schema is useful for validating syntax shape, and
[JSON Schema](https://json-schema.org/draft/2020-12/json-schema-core) supports
reusable definitions, references, assertions, and annotations. It cannot prove
Jisp semantic validity. A successful schema validation must therefore be
followed by canonical Jisp lowering and type checking.

Versioning rules should include:

- no silent reinterpretation of existing AST nodes;
- a compatibility policy for unknown fields and future syntax;
- deterministic module and import resolution independent of host working
  directory where possible;
- a canonical serialization only where hashes, cache keys, or signatures need
  it; and
- a public conformance corpus containing valid programs, invalid programs,
  expected values, diagnostics, and wire-codec examples.

### Make effects capabilities, not globals

Portable modules should be pure by default. A host invocation supplies a small,
declared capability set rather than unrestricted access to Python, Node, the
filesystem, subprocesses, or network APIs. For example:

```json
{
  "jsonrpc": "2.0",
  "id": "42",
  "method": "jisp.eval",
  "params": {
    "languageVersion": "1",
    "module": ["export", "main", ["fn", [], 42]],
    "input": {},
    "capabilities": []
  }
}
```

A future capability may be `clock.now`, `log.write`, `fs.read` under a mounted
root, or a named host function with a declared input/output schema. Each needs
an explicit contract, timeout/cancellation policy, size limits, and error
mapping. Arbitrary `python.eval` or `node.eval` should remain outside portable
Jisp because it destroys reproducibility and sandboxing.

## Concrete use cases

### Node.js coordinates Python work

For orchestration, Node should not embed Python source in a Jisp program or
spawn Python for every expression. Start a long-lived Python worker, expose its
approved operations as versioned JSON-RPC methods, and let Jisp or Node call
those methods through an adapter. Inputs and outputs are Jisp wire values or
application-schema JSON; neither side receives a raw closure from the other.

This design provides process isolation, explicit failure/cancellation, and a
testable contract. Its downside is serialization and worker management, which
is acceptable for orchestration and batch work but not for fine-grained hot
loops. For the latter, write the operation in Rust/Jisp's native subset or add a
carefully designed native extension.

### Shared algorithms and data structures in JSON

This is a strong use case if the algorithm lies in a deliberately portable,
pure subset: deterministic values, no ambient globals, bounded resources,
versioned imports, and schemas for inputs/outputs. Examples include validation,
normalization, policy evaluation, routing, transformations, and business rules.

It is a poor fit for latency-critical kernels, GUI mutation, arbitrary host
reflection, open-ended streaming, or code that requires a host library's object
model. Keep those at the host boundary and pass data through schemas.

## Host support tiers

MAL's host list should inspire a compatibility gradient, not a promise of equal
feature parity everywhere.

| Tier | Contract | Candidate hosts | Required support |
| --- | --- | --- | --- |
| Data | Validate and exchange application JSON only. | Nearly every MAL language. | JSON parser plus a schema validator where available. |
| Runner client | Start or contact the canonical Rust runner. | Node/TS, Python, shell, Java/JVM, .NET, Go, Ruby, PHP, and many others. | Process/socket API, JSON codec, protocol client. |
| Embedded runtime | Call the Rust runtime directly. | Node/TS, Python, JVM, .NET, Ruby, PHP, possibly Go. | Stable FFI/binding, ownership and error ABI, host package distribution. |
| Independent runtime | Evaluate canonical JSON AST locally. | Only JS/TS and Python initially, if evidence demands it. | Full conformance suite, semantic/version policy, maintained release process. |
| Component runtime | Call a Wasm component through WIT. | Modern Wasm component hosts. | Component runtime and generated bindings. |

The data and runner tiers are the route to broad MAL-language reach. Native FFI
and independent runtimes are optimizations for selected ecosystems, not the
baseline portability promise.

## Consequences for current Jisp design

The current architecture already has useful foundations:

- three syntaxes normalize through one source-aware frontend;
- `jisp` exposes facade operations for check, evaluation, imports, and native
  Rust emission;
- Core JSON Schema already exists;
- the interpreter is intentionally broader than native codegen; and
- native codegen rejects unsupported values rather than introducing a universal
  dynamic `Value` ABI.

The next work should preserve those boundaries. Do not make the native backend
or proc macro own a second JSON reader, import resolver, or type checker. Do
not make host interop implicit in the standard library. Do not promise that a
JSON document is executable without checking it against the selected language
version and full Jisp semantics.

## Suggested staged plan

1. Write a short design for `jisp-wire/1`: supported values, exact integers,
   variants/results/options, errors, unsupported values, and size limits.
2. Define a `jisp serve --stdio` proof of concept with JSON Lines or JSON-RPC,
   a `check` and a `run-main` operation, structured diagnostics, and no ambient
   capabilities.
3. Add conformance fixtures shared by the CLI and any future host SDK. Include
   JSON number-boundary, Unicode, result/option, bigint, error, and import
   cases.
4. Build a thin Node/TypeScript client and a thin Python client that invoke the
   runner. Measure startup, steady-state latency, payload size, cancellation,
   and developer experience on the Node-to-Python use case.
5. Decide whether native Node/Python extensions are justified by those results.
   If they are, complete the FFI/ABI design before publishing bindings.
6. Revisit a Wasm component after the process protocol and wire values are
   stable. WIT should describe the resulting stable boundary, not drive the
   language's internal type design.

## Explicit non-goals

- Reimplement Jisp immediately in every MAL language.
- Treat JSON Schema as a replacement for lowering or type inference.
- Serialize functions, closures, arbitrary host objects, or raw pointers.
- Provide unrestricted `eval` in the host language from portable Jisp code.
- Introduce a dynamic universal value into the generated native Rust ABI merely
  to make FFI easier.

## Decision record

The resulting position is:

> JSON is the canonical interchange and source representation. The canonical
> Rust implementation owns complete semantics. A versioned process protocol is
> the universal host boundary; native FFI is a targeted optimization; and full
> host-language implementations are optional products that must earn their
> maintenance cost through a conformance suite and a concrete use case.

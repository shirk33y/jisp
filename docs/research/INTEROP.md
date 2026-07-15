# Interop architecture

## Decision

Jisp should offer a **layered interop ladder**, not one universal FFI and not a
native extension for every host language.

1. **Portable runner:** a long-lived Jisp process using a versioned wire codec
   and JSON-RPC over stdio or a socket. This is the default integration for a
   new host language.
2. **Stable local ABI:** a small C ABI for selected compiled exports. It is the
   common low-latency foundation for generated native adapters.
3. **Generated host adapters:** Python extension, Node-API addon, Java FFM
   binding, .NET `LibraryImport`, and similar packages generated over that ABI
   only after a host has a real use case.
4. **Wasm component:** a WIT-described component for hosts that support the
   Component Model and benefit from a portable binary interface.
5. **Direct codegen:** a separate product decision for a high-value runtime
   such as JS/ESM, the JVM, or BEAM. It is not a substitute for general
   embedding.

Regular Jisp `import` remains Jisp-only. Host libraries are reached through a
target-scoped typed binding or a declared capability; they are never silently
made portable by a foreign import.

## What each layer is for

| Layer | Best use | Contract | Do not use it for |
| --- | --- | --- | --- |
| Runner / JSON-RPC | Automation, polyglot services, prototypes, new languages | Versioned `jisp-wire` request/response values | Sharing host objects, closures, or per-element UI operations |
| C ABI | Low-latency local calls to stable, monomorphic exports | Versioned symbols, opaque context, owned buffers, explicit free/error functions | General Jisp values encoded as C structs or arbitrary callbacks |
| Generated adapter | Idiomatic package in one proven ecosystem | Adapter validates and converts host values around the C ABI | Becoming a second evaluator or owning language semantics |
| Wasm component | Portable binary distribution with WIT-capable hosts | WIT world, canonical ABI, declared imports/exports | A compatibility promise for hosts without Component Model support |
| Direct backend | Native deployment and deep ecosystem integration | Typed Core IR, target runtime, differential conformance suite | A shortcut around the interop or capability design |

The runner should be available first because it has one semantic implementation
and works wherever a process can be launched. JSON-RPC is transport agnostic
and defines request, response, and error envelopes; Jisp still owns framing,
authentication, limits, cancellation, and its richer value codec.

## Common contracts

### Values

Do not pass `jisp_eval::Value`, `serde_json::Value`, `Box<dyn Any>`, arbitrary
host objects, or raw language closures across a public boundary.

Define two representations instead:

- **`jisp-wire/1`** for runner and coarse component calls: versioned,
  serializable Jisp data. It tags values JSON cannot faithfully represent, such
  as bigint, variants/results, and non-finite floating-point values.
- **`jisp-abi/1`** for local native calls: opaque context and byte-buffer
  handles, explicit allocation/free functions, status/error codes, and a
  version query. It begins with validated encoded values, not exposed native
  container layouts. A later typed fast path may be generated only for a fixed,
  monomorphic export signature.

Functions, callbacks, resources, and host handles remain local. If a capability
needs one, it uses a named, versioned interface with an ownership, cancellation,
thread, and error contract; it is not smuggled through ordinary data.

### Bindings and capabilities

A future binding declaration must include the target, package/module, member,
Jisp signature, supported host versions, error mapping, and whether a callback
or resource can escape. The backend must reject it when the selected target has
no matching implementation.

Effects should normally be capabilities. A capability declares data it accepts
and returns; the host provides an implementation. This matches the current UI
rule that views remain pure and that hosts own execution of declared resources.

## Native adapters are optional packages

Native adapters are useful after the stable C ABI exists, but none is the Jisp
core or a new source-language import system.

| Host | Preferred adapter | Why | Boundary rule |
| --- | --- | --- | --- |
| Python | PyO3 extension package plus generated `.pyi` and `py.typed` | Natural `import` and IDE types | Map approved exports only; Python calls remain outside Jisp's portable core. |
| Node.js | Node-API addon | Stable ABI independent of V8; natural npm package | Use Node-API only, not raw V8 APIs. `node-gyp` is a build tool, not an interop model. |
| Java | FFM binding over the C ABI | Modern typed downcalls/upcalls and lifetime-managed foreign memory | Prefer FFM on Java 22+; JNI is compatibility-only. |
| .NET | Generated `LibraryImport` binding | Source-generated calls to a C ABI | Keep marshalling and ownership in generated code. |
| Other C-ABI hosts | Generated or maintained thin wrapper | Reuses one ABI rather than duplicating the evaluator | Add only after demand and conformance coverage. |

An adapter converts host input to one declared Jisp argument representation,
calls one concrete export, maps a result/error, and releases its allocations. It
does not expose an evaluator, imports foreign modules into Jisp, or let host
objects become structural Jisp values.

## Direct codegen is a different decision

Direct codegen is justified when Jisp should *run as a native member of a target
runtime*, not merely be called from it. It needs a target runtime library,
source maps, package integration, target-specific FFI bindings, and an
interpreter-versus-target conformance suite.

- **JS/ESM:** good candidate for Jisp UI and bundler integration. Generate
  ordinary ESM plus declarations and a small runtime; do not call a Rust addon
  for each UI operation.
- **JVM:** credible candidate for portable, monomorphic Jisp exports. Emit
  class files/JARs directly in the durable backend; use Java bindings only for
  explicitly declared APIs.
- **Python:** not an initial codegen target. The extension package gives Python
  users an idiomatic import while retaining the Rust compiler as the semantic
  implementation.
- **BEAM and other runtimes:** require their own target-semantics and runtime
  design before a backend is approved.

No direct backend may relax Jisp's concrete-layout rule merely because its host
offers a dynamic object model.

## Models to avoid as defaults

- One universal native module loaded by every runtime. It turns deployment,
  crashes, memory ownership, and host APIs into a cross-platform bottleneck.
- Embedding CPython, a JVM, or Node inside normal compiled Jisp programs. The
  host runtime's lifecycle, threads, package resolution, and error model would
  leak into every binary.
- Raw JNI, V8, or CPython API calls from generated user code. Keep unstable or
  unsafe host details in generated adapters.
- Automatic imports of arbitrary Python/JavaScript/Java libraries. Jisp cannot
  type-check their full dynamic/reflection/overload behavior and must not make
  their APIs look portable.
- A generic `extern` that accepts untyped values or arbitrary callbacks.

## Delivery order

1. Specify `jisp-wire/1`, including bigint, variants/results, float edge
   cases, limits, and error representation.
2. Ship the runner protocol and one maintained client example; make it the
   reference integration for new languages.
3. Specify `jisp-abi/1`: ABI versioning, context lifetime, panic containment,
   buffer allocation/freeing, errors, and thread/callback policy.
4. Generate one adapter for a validated use case—Python is the first likely
   candidate—and test its wheel/package plus type stubs.
5. Add WIT only when the Jisp value/capability model maps cleanly to a declared
   component world; maintain the existing WIT UI boundary separately from a
   general evaluator ABI.
6. Approve a direct target only with a written semantic mapping and
   differential tests for every accepted subset feature.

This keeps future languages additive: start with the runner, reuse the C ABI
when local latency matters, generate a native adapter only when its ergonomics
justify the distribution cost, and build a direct backend only when Jisp needs
to belong to that runtime.

## Primary sources

- [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification)
- [WebAssembly Component Model: WIT](https://component-model.bytecodealliance.org/design/wit.html)
- [WebAssembly Component Model: composition and canonical ABI](https://component-model.bytecodealliance.org/composing-and-distributing/composing.html)
- [Python: Extending and Embedding the Python Interpreter](https://docs.python.org/3.14/extending/)
- [PEP 561: distributing type information](https://peps.python.org/pep-0561/)
- [PyO3 guide](https://pyo3.rs/main/)
- [Node-API](https://nodejs.org/api/n-api.html)
- [JEP 454: Foreign Function & Memory API](https://openjdk.org/jeps/454)
- [.NET native interoperability ABI support](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/abi-support)
- [JVM Class-File API](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/classfile/package-summary.html)

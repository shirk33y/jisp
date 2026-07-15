# Python and Java targets

## Decision summary

Jisp should not treat Python or Java imports as ordinary portable `import`
forms. A regular Jisp import remains a Jisp module. Python and Java APIs are
target-specific implementations behind explicitly typed bindings or
capabilities.

The recommended order is:

1. **Python integration:** expose the existing native Rust subset as a Python
   extension module with generated `.pyi` type information. Keep calls from
   Jisp into arbitrary Python code behind a process/capability boundary.
2. **Java integration:** if a second direct compilation target is justified,
   add a JVM backend for the same portable, monomorphic subset. Emit class files
   and a JAR directly; do not route ordinary programs through JNI or a Rust
   library.
3. **Native boundary:** when a Java host needs the existing Rust-compiled Jisp
   library, offer a narrow C ABI consumed through Java's Foreign Function &
   Memory API (FFM), not a bespoke JNI layer.

This preserves the current invariant: the interpreter is the semantic oracle,
and compiled Jisp values have concrete target layouts rather than an implicit
universal `Value` ABI.

## Shared rule: source imports versus foreign bindings

```text
(import math "math.jisp")          portable Jisp source
(extern python ...)                 Python-only binding
(extern java ...)                   JVM-only binding
(capability storage.write ...)      portable effect interface; host provides it
```

An `extern` declaration must name its target, package/module, member, Jisp
signature, error conversion, and supported target version. It must fail at
compile time when selected for a target without an implementation. It may not
expose untyped reflection, a host object, or a callback with unspecified
lifetime as an ordinary Jisp value.

Generated code owns imports. Jisp does not parse arbitrary Python or Java code
to discover types, and it does not adopt either language's module-resolution
rules as its own package resolver.

## Python

### Recommendation: native extension first

The high-value Python deliverable is a normal importable Python package whose
implementation is generated Rust:

```text
typed Jisp module
  -> current Rust codegen subset
  -> generated PyO3 adapter
  -> platform wheel + .pyi stub
  -> import jisp_package
```

CPython officially supports dynamically loaded extension modules and embedding
through the Python/C API. PyO3 is the appropriate Rust adapter: it supports
both native Python extension modules and embedding Python in a Rust binary.
The adapter is deliberately outside the Jisp language core: it validates Python
arguments, calls a concrete Jisp export, converts a declared result, and maps a
Jisp `result` or native failure into a documented Python exception.

The package must ship generated `.pyi` stubs and a `py.typed` marker. PEP 561
defines the packaging and resolution convention that makes these signatures
visible to Python type checkers and IDEs.

Illustrative source and generated Python surface:

```lisp
(export slugify)
```

```python
# jisp_package/__init__.pyi
def slugify(value: str) -> str: ...
```

Only Jisp exports whose types have a declared Python representation are
eligible. Start with `null`, `bool`, checked `i64`, `float`, `str`, homogeneous
lists/maps, closed objects, concrete variants/results, and first-order
functions only where the callback/GIL contract is designed. `bigint` should be
represented explicitly rather than silently narrowed. Opaque handles, open
rows, arbitrary closures, and UI nodes are not initial Python values.

### Do not start with a Python codegen backend

Generating `.py` would make Jisp easy to inspect, but it would not make it
native or automatically portable. Python imports are extensible and runtime
driven, Python values are mutable by default, and Python's numeric and async
semantics differ from Jisp's checked `i64`, explicit bigint, immutable updates,
and future capability model. A complete backend would need its own runtime,
packaging, diagnostics, and differential conformance suite.

It is therefore a later interoperability target, not the first Python feature.
The extension approach reuses the existing Rust ABI work and gives Python users
an ordinary package installation and import experience without duplicating
Jisp's evaluator.

### Python called from Jisp

Do not embed CPython into ordinary compiled Jisp programs as the default. It
would impose interpreter lifecycle, distribution, thread/GIL, error, and host
object rules on every native build.

For approved Python operations, use a versioned capability or a long-lived
JSON-RPC worker with Jisp wire values. A future explicit `extern python`
binding can be considered only after the native FFI design defines ownership,
exceptions, callbacks, cancellation, and supported Python implementations.

## Java and the JVM

### Recommendation: direct JVM backend when a second codegen target is needed

The JVM is a plausible direct target because Java normally runs machine-
independent class files, and the runtime JIT-compiles and optimizes them. A
`jisp-codegen-jvm` crate should consume `TypedModule`, lower the portable
monomorphic subset, emit `.class` files, and package them in a JAR:

```text
typed Jisp module
  -> JVM lowering (closures, cases, layouts)
  -> class files
  -> JAR + generated Java signatures/source map
```

Start on the class path with an ordinary JAR. Adopt JPMS module descriptors
only once Jisp packages need strong encapsulation, service publication, or
reliable Java-module dependency metadata. Java module resolution is a real
compile- and runtime access boundary, not merely a naming convention.

Emit class files directly rather than generating Java source and invoking
`javac` in the durable backend. Direct emission gives one deterministic toolchain
and source-map contract; generated Java source is acceptable only as an
inspectable prototype. The JDK's Class-File API demonstrates that class-file
generation is a first-class platform operation, but it cannot be reused by the
Rust compiler itself; the Rust backend needs its own verified class-file writer.

### Java interop contract

Initial Jisp-to-Java bindings should be static and explicit:

```text
target: java
owner: com.example.Text
member: normalize
descriptor: (Ljava/lang/String;)Ljava/lang/String;
jisp type: (str) -> str
throws: java.lang.IllegalArgumentException -> result<str, java-error>
```

The backend emits a normal JVM invocation. It must not discover methods through
reflection or accept arbitrary overload resolution at runtime. Java overloads,
nullable references, mutation, checked/unchecked exceptions, generic erasure,
and Java collection ownership must be resolved in the binding declaration.

Suggested first mappings:

| Jisp | JVM representation | Boundary rule |
| --- | --- | --- |
| `null`, `bool`, `int`, `float`, `str` | `null`, `boolean`, `long`, `double`, `String` | Check Jisp `i64` and float error rules; do not use Java `int` for Jisp `int`. |
| `bigint` | `java.math.BigInteger` | Explicit conversion only. |
| `list<T>` | immutable/copying `List<T>` adapter | Do not leak a mutable Java list as a Jisp value. |
| `map<str, T>` | copying ordered-map adapter | Specify ordering and null policy. |
| Jisp variant/result | generated final/sealed representation | Map failures to explicit Jisp results, not implicit exceptions. |
| foreign object | opaque resource handle | No structural equality, serialization, or unbounded lifetime. |

Jisp functions and closures need an explicit Java functional-interface adapter
only after callback escaping, exceptions, and thread ownership have a written
contract. Do not represent every Jisp function as `Object` or `MethodHandle`.

### Java hosts calling native Jisp

This is separate from a JVM backend. For Java applications that want to call a
Rust-compiled Jisp library, expose the future narrow C ABI from
`docs/FFI_FUTURE.md` and bind it using FFM on Java 22 or later. FFM provides
typed downcalls/upcalls and managed foreign-memory lifetimes; it is the modern
Java route for native calls. Its restricted native-access operations still
require explicit deployment configuration, and an incorrect ABI descriptor can
remain unsafe, so generated bindings and a small ABI are mandatory.

JNI is a compatibility option only. It is not the default Jisp integration
model.

## Delivery gates

1. Write the target-binding schema and portable-value codec before adding any
   `extern` syntax.
2. Add interpreter-versus-generated conformance fixtures for every accepted
   type, numeric edge case, immutable update, case branch, and error mapping.
3. Ship Python as a generated extension plus `.pyi` stubs; verify it in an
   isolated virtual environment.
4. Prototype JVM lowering on closed, monomorphic, pure exports; verify the
   generated JAR on a declared JDK baseline.
5. Add Java foreign bindings and FFM only after the C ABI ownership and error
   contract are approved.

Neither target justifies weakening the one-Core-IR rule or adding a dynamic
language escape hatch to ordinary Jisp imports.

## Primary sources

- [Python: Extending and Embedding the Python Interpreter](https://docs.python.org/3.14/extending/)
- [Python/C API reference and stable-ABI material](https://docs.python.org/3.14/c-api/index.html)
- [Python import machinery](https://docs.python.org/3.14/library/modules.html)
- [PEP 561: distributing type information](https://peps.python.org/pep-0561/)
- [PyO3 guide](https://pyo3.rs/main/)
- [Java Language Specification: compilation and runtime](https://docs.oracle.com/javase/specs/jls/se26/html/jls-1.html)
- [JVM class-file API](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/classfile/package-summary.html)
- [Java modules and resolution](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/module/package-summary.html)
- [JEP 454: Foreign Function & Memory API](https://openjdk.org/jeps/454)

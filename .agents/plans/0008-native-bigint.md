# Native bigint support

## Goal

Lower the existing Jisp `bigint` type and supported bigint prelude operations
to concrete Rust without introducing `jisp_eval::Value` or an erased numeric
ABI. Native values must match interpreter semantics for arbitrary precision,
truncating division, and Euclidean division/modulo.

## Representation and public dependency

The generated Rust representation is `::num_bigint::BigInt`. This is a concrete
owned value that works in functions, closures, lists, closed objects, enum
payloads, and concrete `result`/`option` layouts already emitted by the backend.

Generated code refers to `::num_bigint` directly. A crate that uses
`jisp_macros::lisp_file!` for a module containing bigint values must therefore
declare a compatible direct dependency:

```toml
[dependencies]
jisp-macros = "..."
num-bigint = "0.4"
```

A proc-macro dependency cannot make an arbitrary transitive crate name available
to the consuming crate's generated Rust. Hiding bigint behind an evaluator value
or serialised string would violate the native ABI invariant; embedding a second
bigint implementation in every macro expansion would be a larger, less reliable
cost. The direct dependency is an intentional, visible native-codegen contract.

## In scope

- `(bigint "...")` construction through `BigInt::parse_bytes`.
- `+`, `-`, `*`, `/`, `//`, and `%` for bigint values.
- `=`, `<`, `>`, `<=`, and `>=` through native operators.
- `math.abs`, `math.min`, and `math.max` for bigint values.
- Bigints nested in typed native lists, closures, closed objects, enums, and
  concrete result/option layouts via ordinary recursive type emission.
- Generated-token tests, interpreter-vs-native differential tests, and a
  downstream proc-macro fixture that declares `num-bigint` explicitly.

## Out of scope

- Implicit coercion between `int`, `bigint`, and `float`.
- `math.pow` for bigint; the current language specification excludes it.
- A separate bigint type implemented in generated Rust.
- Open objects, dynamic field access, or a generic native `Value` fallback.

## Semantic lowering

`+`, `-`, `*`, equality, comparison, and `min`/`max` use the corresponding
`BigInt` operators or ordering implementation. `/` evaluates both operands once,
rejects zero, and uses BigInt's truncating quotient.

BigInt `//` and `%` cannot use Rust's raw `/` and `%` directly because their
remainder follows the dividend. Generated code mirrors the evaluator algorithm:

1. compute truncating quotient and remainder;
2. when the remainder is negative, adjust the quotient by `-1` for a positive
   divisor or `+1` for a negative divisor;
3. when the remainder is negative, add a positive divisor or subtract a
   negative divisor to obtain the Euclidean remainder.

`math.abs` compares with zero and negates only negative values, avoiding an
additional generated-code dependency on `num_traits::Signed`.

## Acceptance

- `emit-rust` emits `::num_bigint::BigInt` for a public bigint value and no
  `jisp_eval` or universal `Value` representation.
- The differential fixture covers construction, arithmetic beyond `i64`, signed
  Euclidean division/modulo, absolute value, and comparison.
- A temporary downstream macro crate with explicit `num-bigint` compiles and
  executes a generated bigint export.
- Existing unsupported-native compile-fail coverage moves to a genuinely
  unsupported operation rather than retaining an obsolete bigint failure.
- Full local formatting, Clippy, workspace tests, and macro tests pass.

## Status

Implemented on `master`: native type emission uses `::num_bigint::BigInt`; the
constructor, arithmetic, comparisons, `math.abs`, `math.min`, and `math.max`
all stay concrete. Differential coverage includes values beyond `i64`, signed
Euclidean division/modulo, comparison, and a captured bigint closure. The macro
suite runs a downstream crate with direct `num-bigint` dependency and retains a
separate compile-fail fixture for unsupported `ui.html` emission.

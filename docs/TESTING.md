# Testing strategy

Existing tests cover sources, parsers, lowering/evaluation, cross-syntax
normalisation, type instantiation, and unification foundations.

Add snapshot tests for diagnostics and schema. Add property tests ensuring
parse/normalise equivalence across syntax fixtures. Native codegen later needs
compile-pass/compile-fail fixtures and interpreter-vs-native differential tests.

This archive was written without installing or executing Rust. CI is provided
as the intended validation environment.

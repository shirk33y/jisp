# Testing strategy

Existing tests cover sources, parsers, lowering/evaluation, cross-syntax
normalisation, type instantiation, unification, module resolution, portable
language fixtures, and the native Rust subset. Proc-macro integration fixtures
compile and execute generated scalar, structural, and imported Rust programs.

Add broader snapshot tests for diagnostics and schema. Add property tests
ensuring parse/normalise equivalence across syntax fixtures. Native codegen
still needs systematic compile-fail fixtures and interpreter-vs-native
differential tests.

CI pins the Rust toolchain, checks formatting and Clippy with warnings denied,
runs the workspace suite, and runs `jisp-macros` separately so generated Rust
is compiled by the test harness.

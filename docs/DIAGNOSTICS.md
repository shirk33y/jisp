# Diagnostics

Every source AST node carries a byte span and source id. Parser, lowering,
module, macro, and type errors must be rendered against the original source.
The current renderer supports primary labels, secondary labels, notes,
cross-file labels, and multi-line spans. Macro expansion records generated span
origins in `ExpansionMap`; detailed facade errors render those origin chains as
secondary labels.

`check_detailed`, `run_main_detailed`, and `emit_rust_detailed` retain their
`SourceMap` on failure. Type inference records the narrowest failing expression
span and renders it as `JISP-TYPE`, including failures in imported modules.
Interpreter failures render as `JISP-RUNTIME`; their primary runtime span and
the collected evaluation frames are shown against the original Jisp sources.
Overloaded calls report every accepted function signature instead of leaking
the failure from an arbitrary overload candidate.

Stable proc macros cannot assign arbitrary spans into external Jisp files.
Native CLI builds should therefore optionally emit generated Rust plus a source
map, invoke Cargo with JSON diagnostics, and remap generated ranges back to
Jisp. The proc-macro-only experience can still print an embedded source excerpt
but rustc may additionally point at the macro invocation.

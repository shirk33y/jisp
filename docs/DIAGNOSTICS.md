# Diagnostics

Every source AST node carries a byte span and source id. Parser, lowering,
module, macro, and type errors must be rendered against the original source.
The current renderer supports primary labels, secondary labels, notes,
cross-file labels, and multi-line spans. Macro expansion should retain an
origin chain once macro expansion exists.

Stable proc macros cannot assign arbitrary spans into external Jisp files.
Native CLI builds should therefore optionally emit generated Rust plus a source
map, invoke Cargo with JSON diagnostics, and remap generated ranges back to
Jisp. The proc-macro-only experience can still print an embedded source excerpt
but rustc may additionally point at the macro invocation.

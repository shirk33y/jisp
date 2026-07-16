# Collection toolbox

`main.lisp` demonstrates immutable list and map updates and evaluates to `16`.
`unsupported.lisp` intentionally tries a dynamic key on a heterogeneous closed
object. It must fail before generated Rust because native Jisp has no dynamic
`Value` fallback. `open-row.lisp` is a generic row function: the interpreter
can run it, while native codegen rejects its polymorphic layout explicitly.

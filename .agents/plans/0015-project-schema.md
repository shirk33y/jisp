# Project-aware JSON Schema

## Separate two products

`jisp_core::core_schema()` is a schema for canonical JSON *source AST*. A
project-aware schema must not silently claim to validate source when it actually
describes evaluated/exported values. Expose two explicit APIs:

1. `source_schema`: the existing syntax schema, optionally enriched with the
   resolved module graph for editor tooling.
2. `export_schema`: a value schema for one selected public export, derived from
   its resolved Jisp `Scheme` after rejecting or requiring an instantiation for
   type variables and functions.

## Value mapping

Map `null`, `bool`, `int`, `float`, `str`, lists, and closed objects directly
to JSON Schema. Map named algebraic types to `oneOf` tagged-array schemas that
mirror canonical Jisp data. Bigints are decimal strings at the JSON boundary.
Functions, open object rows, and unresolved type variables are errors rather
than `any`, because a permissive schema would hide a missing contract.

## Project resolution

The facade already retains `ParsedModule.resolved_modules` and top-level
schemes. The new API should resolve imports exactly as `check_detailed`, return
module paths/dependencies alongside the schema for stale tracking, and let the
CLI select an export. Do not place module metadata inside a JSON Schema value
schema; return it as a separate envelope.

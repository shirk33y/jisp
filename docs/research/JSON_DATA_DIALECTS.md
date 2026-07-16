# JSON/YAML data dialects and raw object literals

Status: research and design direction, 2026-07-15. This document does not
enable raw `{}` syntax or change the language contract.

## Question

Jisp's canonical JSON intentionally treats ordinary JSON strings as symbols and
every JSON array as a form. That is compact for Lisp code but poor for data-heavy
source such as UI props, style tokens, fixtures, and API-shaped values. The
proposed experimental family reverses the atom default:

```json
["$def", "$profile",
  {"name": "Ada", "href": ["$str", "https://", ["$domain"]]}]
```

Here ordinary strings are strings, a `$` prefix makes a Jisp symbol, a
`$`-headed array is a form, and `{}` denotes a structural object. This report
examines whether that can preserve Jisp semantics and what has to be specified
before accepting it.

The goal is **AST and runtime-semantic equivalence with Jisp**, not byte-for-byte
round-tripping. Formatting a Lisp `(obj "name" "Ada")` as `{"name":"Ada"}`
is intentionally allowed; recovering the author's original surface spelling is
not.

## What the current implementation already provides

Raw JSON and YAML objects are deliberately rejected today: JSON emits
`JISP-J003` and YAML emits `JISP-Y003`. The parsers currently normalize into a
small source AST with `Form`, but no source-level `Map` node. See the
[JSON reader](../../crates/jisp-syntax-json/src/lib.rs) and
[core AST](../../crates/jisp-core/src/ast.rs).

This does **not** require adding a new Core IR value. The lowerer already treats
the heads `list` and `obj` as constructors and emits `ExprKind::List` and
`ExprKind::Object`, before ordinary call lowering. It also already rejects
repeated statically known object keys. See
[lowering of `list` and `obj`](../../crates/jisp-ir/src/lower.rs) and
[object lowering](../../crates/jisp-ir/src/lower.rs).

Therefore a data-dialect reader can normalize:

```text
[1, "$x"]       -> (list 1 x)
{"name": "$x"} -> (obj "name" x)
```

and still obey the one-source-AST, one-Core-IR architecture. Parsers only select
the existing `list`/`obj` forms; their language meaning remains in the shared
lowerer, type checker, runtime, evaluator, and code generator.

There are two important limits:

1. Canonical `(obj dynamic-key value)` is valid Jisp, but JSON/YAML `{}` keys
   are necessarily literal strings. A formatter may use `{}` only for an `obj`
   form whose keys are statically known strings.
2. The module-root array is a syntactic container, not an expression. A
   data-dialect reader must normalize it in a dedicated module context; applying
   the ordinary array rule to the outermost `[["$def", ...], ["$export", ...]]`
   would incorrectly introduce a `(list ...)`.

## Relevant external designs

| Source | Relevant observation | Consequence for Jisp |
| --- | --- | --- |
| [JSON-e](https://json-e.js.org/operators.html) and its [versioned multi-implementation repository](https://github.com/json-e/json-e) | Uses `$`-prefixed operators and `$$` to escape operator-like object properties. Its project treats an implementation disagreement as a language bug. | A sigil is viable, but escaping and conformance tests are part of the syntax contract, not parser trivia. |
| [JsonLogic](https://jsonlogic.com/operations.html) | Represents an operation as a single-key object such as `{"var": "a"}`. | This is an anti-pattern for Jisp: plain data objects become ambiguous with program forms. Jisp should keep `{}` as data and use arrays/forms for code. |
| [Jsonnet](https://jsonnet.org/ref/language.html) | Object fields may be computed, inherited, hidden, asserted, and lazily evaluated. | Do not import its object feature set. Dynamic keys, field visibility, inheritance, and spreads would turn a small literal into a separate language subsystem. |
| [CUE](https://cuelang.org/docs/reference/spec/) | Repeated fields participate in constraint unification. | Jisp objects are values, not constraints: duplicate literal keys must stay an error rather than acquiring merge or unification semantics. |
| [Clojure reader](https://clojure.org/reference/reader) | Map literals are a reader-level structural construct with dedicated rules. | The useful lesson is to give maps an explicit, separate syntactic category; Clojure's namespace/tag reader features are not needed here. |

The closest positive precedent is JSON-e's explicit escape convention. The
closest warning is JsonLogic: reserving ordinary object shapes as executable
forms makes an otherwise data-shaped language hard to reason about.

## Standards and research findings

### JSON objects are not a safe implicit language contract

[RFC 8259 section 4](https://www.rfc-editor.org/rfc/rfc8259#section-4) calls a
JSON object an unordered collection, requires string names, says names *should*
be unique, and explicitly says receiver behaviour with duplicates is
unpredictable. The empirical study
[The Behavioral Diversity of Java JSON Libraries](https://arxiv.org/abs/2104.14323)
found meaningful differences among 20 libraries around duplicate data and
numeric corner cases.

The dialect must consequently:

- parse object members as an ordered source sequence, never into a host map
  before validation;
- reject duplicate decoded keys with a diagnostic spanning both occurrences;
- preserve textual member order through normalization and formatting; and
- define its own integer/float rules through Jisp rather than accepting a host
  JSON library's numeric representation.

The order requirement matters even though ordinary JSON treats objects as
unordered. Jisp specifies left-to-right argument evaluation, and `{}` lowers to
an `obj` form with value expressions. A generic JSON rewriter that reorders
members must not be claimed to preserve an executable Jisp document.

### YAML needs a deliberately narrow subset

The [YAML 1.2.2 specification](https://yaml.org/spec/1.2.2/) models mappings as
unordered unique-key collections and additionally includes tags, anchors,
aliases, and graph-shaped data. The paper
[Laughter in the Wild](https://doi.org/10.1109/TrustCom/BigDataSE.2019.00053)
studied 14 YAML libraries across ten languages and reported seven previously
unknown DoS vulnerabilities.

`yaml-data-v1` should remain a *restricted flow-style reader*, not become a
general YAML implementation. In particular, reject rather than reinterpret:

- anchors and aliases (`&name`, `*name`), including cycles;
- tags, custom constructors, merge keys (`<<`), multi-document streams, and
  block scalars;
- complex, non-string, or computed mapping keys; and
- parser-defined implicit typing beyond Jisp's own null/bool/number rules.

This is both a portability decision and a resource/security boundary. It keeps
the JSON and YAML profiles as different scanners for the same semantic decoder.

### Web hosts need safe object materialisation

In JSON, `"__proto__"` is an ordinary property name. It becomes dangerous when
such data is later merged or assigned into ordinary JavaScript objects. MDN's
[prototype-pollution guidance](https://developer.mozilla.org/en-US/docs/Web/Security/Attacks/Prototype_pollution)
documents this exact path and recommends `Map` or null-prototype objects for
dynamically populated dictionaries.

The Jisp web renderer must retain object keys as data and avoid naive
`Object.assign`, `for...in`, or `target[key] = value` materialisation for
untrusted or program-generated objects. `__proto__`, `constructor`, and
`prototype` are valid Jisp string keys; banning them would make cross-host data
semantics less faithful. The defence belongs in the host adapter.

## The language traps

### 1. A first-element heuristic is contextual syntax, not JSON data

The compact rule is useful:

```json
["$f", 1]      // (f 1)
["a", "$x"]   // (list "a" x)
["$$f", 1]     // (list "$f" 1)
```

It means that moving a data item into first position can change a list into a
call. This is acceptable only when the dialect is explicitly code-bearing and
the playground makes the resulting classification visible. It is not a
lossless, data-only JSON configuration format.

The classifier must inspect the decoded *and unescaped* first value. `"$$f"`
is an escaped literal and therefore starts a list, not a call.

### 2. `$` needs an exact decoding and an escape hatch

For the ergonomic subset, decode JSON/YAML strings after their normal string
escapes as follows:

```text
"text"    -> String("text")
"$name"   -> Symbol("name")
"$$name"  -> String("$name")
"$$"      -> String("$")
"$"       -> error: empty sigil symbol
```

Thus JSON `"\u0024name"` means the same as `"$name"`; JSON
`"\u0024\u0024name"` means literal `"$name"`. The dialect escape is not a
JSON backslash escape.

This compact mapping is not complete by itself: Jisp's Lisp reader permits a
symbol whose spelling itself begins with `$`, and a symbol-headed form may have
a computed callee. A profile that promises AST-isomorphic conversion needs two
reserved, parser-only escape forms:

```json
["$sym", "$name"]
// a Symbol whose exact spelling is "$name"; the second element bypasses $ decoding

["$form", ["$if", "$enabled", "$f", "$g"], "$x"]
// ((if enabled f g) x)
```

`$form` normalizes directly to a `NodeKind::Form` whose first child is the
second input element; it is not a new Core special form. A direct call to a
function named `form` then uses the verbose but unambiguous
`["$form", "$form", ...]`. These reserved markers are the price of a total
AST codec. Without them the compact notation covers the common case but cannot
honestly claim 1:1 conversion from arbitrary Jisp.

### 3. `{}` must always be a literal structural object

The safe rule is intentionally narrow:

```json
{
  "$schema": "https://example.test/schema",
  "href": ["$str", "https://", ["$domain"]],
  "data-id": "$id"
}
```

normalizes to:

```lisp
(obj "$schema" "https://example.test/schema"
     "href" (str "https://" (domain))
     "data-id" id)
```

Object keys are always literal decoded strings. They never use `$` decoding,
symbol resolution, dot paths, namespace expansion, or computed-key semantics.
Object values use the normal data-dialect expression decoder.

Reject duplicate keys immediately, after JSON/YAML string decoding but without
Unicode normalization. `"a"` and `"\u0061"` are duplicates; visually similar
but different Unicode scalar sequences remain different keys, as they are for
ordinary Jisp strings.

Do not include spreads, implicit merge, dynamic keys, or a map/object
conversion in v1. Dynamic dictionaries remain explicit `map` values. Closed,
heterogeneous objects remain `obj` values, preserving the existing type and
native-ABI boundary.

### 4. Module-root and quote contexts need dedicated tests

The outer JSON/YAML sequence is already a module container in the current
syntax. In the data profile it must remain one; it is not an implicit `(list
...)`. A one-form module can keep the existing shorthand:

```json
["$export", "$main", ["$fn", [], 42]]
```

For a multi-form module, the outer sequence is module syntax and each child is
an expression/form:

```json
[
  ["$def", "$settings", {"theme": "dark"}],
  ["$export", "$settings"]
]
```

The normalizer must also be tested under `quote`, `quasiquote`, unquote, macros,
and pattern forms. It should produce the same nodes that the existing canonical
`list` and `obj` forms would have produced, so macro expansion does not acquire
format-specific semantics.

## Recommended experimental profile family

Do not alter `json`, `yaml`, `.json`, or `.yaml` in place. Their current
semantics are a documented interchange contract. Make this a named,
source-level, versioned profile selected by extension and/or manifest
allow-list, for example:

```text
json-canonical-v1       existing .json reader
yaml-canonical-v1       existing .yaml reader
json-data-v1            ordinary strings + $ symbols + [] heuristic + {}
yaml-data-v1            same normalizer, restricted YAML flow scanner
```

These are not Cargo feature flags. A Cargo flag would change the language
accepted by a build artifact and make editor, formatter, import, and playground
behaviour depend on compilation choices. Optional crates are appropriate for
renderers; source dialects need explicit per-file/per-project selection.

Internally, the implementation may model the profiles as independent axes:

```text
atom mode: canonical symbols | data strings with $
array mode: all forms      | $-headed forms, otherwise list
object mode: reject        | string-key object literal
```

The public contract should still expose only fixed, versioned profiles. This
allows the requested atom reversal and `{}` objects to coexist without an
unbounded matrix of user-defined booleans.

## Required conformance corpus before implementation

1. Cross-reader AST/IR equivalence for Lisp, `ws`, canonical JSON, canonical
   YAML, `json-data-v1`, and `yaml-data-v1`.
2. `$`, `$$`, `$$$`, empty `$`, JSON Unicode-dollar escapes, and exact-symbol
   escape examples.
3. Arrays whose first data item is `$`-like; empty lists; nested forms; and
   computed-callee `$form` examples.
4. Empty objects, nested objects, duplicate decoded keys, source ranges for
   both duplicate locations, and keys such as `__proto__` and `data-id`.
5. A formatter contract: parse/format/parse preserves normalized AST; source
   spelling, comments, and original choice of `(obj ...)` versus `{}` need not
   survive.
6. YAML rejection fixtures for anchors, aliases, tags, merge keys, complex
   keys, implicit ambiguous scalar types, excessive nesting, and oversized
   inputs.
7. Web-host tests proving safe materialisation of special JavaScript property
   names and no dependence on host object iteration order.

## Decision

Raw `{}` is viable and aligns naturally with Jisp's existing `obj` Core IR
literal. It should be introduced only as part of a versioned data-dialect
profile that also defines `$`, array classification, module-root handling,
escape hatches, diagnostics, and a restricted YAML surface. Treating braces as
a standalone parser toggle would leave the data/code boundary and cross-host
behaviour underspecified.

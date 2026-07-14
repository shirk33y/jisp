# Plan: Lisp-markup UI reader with AST-equivalent conversion

**Status:** Research-backed proposal; no implementation has started.
**Date:** 2026-07-14
**Depends on:** [0019-compiled-portable-ui-runtime.md](0019-compiled-portable-ui-runtime.md) and [0020-blitz-jispwind-renderer.md](0020-blitz-jispwind-renderer.md)
**Decision type:** New source reader and formatter, not a new UI runtime

## 1. Decision in one sentence

Add an optional Jisp Markup source syntax, provisionally using the .jmx
extension, in which HTML-shaped host elements are syntax sugar for the existing
Jisp UI forms. Complex dynamic values use their ordinary Jisp function-form
parentheses, while literals and single-symbol values use their ordinary Jisp
atom spelling.

~~~text
<div id=(. user "name")
     href=(str "https://" (get-domain) "/index.html")>
</div>
~~~

normalizes to the same source-aware Jisp AST shape as:

~~~lisp
(div
  (attr "id" (. user "name"))
  (attr "href" (str "https://" (get-domain) "/index.html")))
~~~

This is viable without a virtual DOM, JavaScript runtime, or renderer change.
It is a reader/formatter feature before macro expansion; it feeds the existing
shared AST, type checker, UI lowerer, JUIR compiler, static HTML renderer, DOM
host, and future native hosts unchanged.

## 2. What “1:1 conversion” can honestly mean

The supported guarantee is an AST isomorphism, not byte-for-byte source
round-tripping.

For any supported markup module M:

~~~text
parse_markup(M) == parse_lisp(format_lisp(parse_markup(M)))
~~~

For a valid Jisp UI module L, after its UI element/directive forms are selected
for markup printing:

~~~text
parse_markup(format_markup(parse_lisp(L))) == parse_lisp(L)
~~~

The equality compares normalized NodeKind trees and values, ignoring source
ranges. The markup formatter is canonical: it may change quoting, indentation,
empty-element spelling, directive grouping, and static class grouping.

The following do not survive a semantic-AST conversion and must not be
advertised as part of 1:1:

- comments, which the current shared AST already drops;
- original whitespace-only formatting runs;
- quote/entity spelling and attribute layout;
- source range positions, which necessarily change after printing; and
- an arbitrary HTML document's browser parsing behavior.

Conversely, this plan does not limit ordinary Jisp. A .jmx module remains a
complete Jisp module: definitions, macros, functions, conditionals, and
component calls remain parenthesized Jisp forms. The markup printer prints
valid UI host-element subtrees as tags and leaves all other forms as ordinary
Lisp. The only narrower claim is that not every Jisp expression can be printed
as a tag.

## 3. Why this is the right boundary for Jisp

The current Jisp UI language already has exactly the target structural
contract:

- a finite registry of host elements;
- explicit attr, prop, class, class-if, on, key, and for directives;
- text nodes as the explicit text form;
- child component calls, if blocks, and for blocks as ordinary Jisp
  expressions; and
- renderer-neutral nodes that become JUIR after typed UI lowering.

The new syntax must therefore normalize into those existing forms. It must not
introduce a MarkupNode into Core IR, let a parser validate renderer
capabilities, infer attributes from spelling, or teach a browser/native host to
parse a second source language.

This preserves the current architecture:

~~~text
.jmx reader
  -> shared Node AST
  -> macro expansion
  -> typed Core IR
  -> existing UI validation/lowering
  -> JUIR
  -> HTML / DOM patches / test tree / native host
~~~

JUIR remains an internal execution protocol, not the parsed form. In
particular, markup syntax must never cause the evaluator to allocate a generic
VDOM or cause a host to interpret Jisp source text.

## 4. Research findings

| Reference | Relevant finding | Consequence for Jisp |
| --- | --- | --- |
| Racket Scribble at-reader | An alternate concrete syntax can be an S-expression in disguise; its reader translates text and nested forms to ordinary S-expressions before evaluation. | Keep Markup a reader-level desugaring into Jisp Node values, not a runtime object or macro-only convention. |
| Racket XML X-expressions | XML can be represented as element name, ordered attributes, and children; reading/writing is structural rather than source-byte preserving. | Promise AST equivalence and canonical printing, not exact original text recovery. |
| SXML | An element maps naturally to a list beginning with its tag, followed by an attribute list and children. | Jisp already has an equivalent host-element plus directive shape; do not invent a competing generic XML data model. |
| TypeScript JSX | JSX is embeddable XML-like syntax whose transformation semantics are implementation-specific; it distinguishes intrinsic elements from value-based components. | Borrow only the ergonomic surface. Keep host tags explicit and distinguish them from Jisp component calls instead of inheriting React/JSX rules. |
| Babel JSX transform | JSX source is compiled into target calls; fragments, namespaces, spreads, and runtime options all add policy surface. | Do not pursue JSX compatibility, fragment syntax, namespaces, or prop spreads in the first version. Jisp has a smaller existing contract. |

The closest language-design precedent is Racket's reader model, not React:
concrete text is read as normal language data before evaluation. The closest
data precedent is SXML/X-expression conversion, which reinforces the distinction
between structural equivalence and preservation of source trivia.

## 5. Proposed syntax

### 5.1 File and reader mode

Introduce a new Syntax::Markup variant and recognize the provisional .jmx
extension. It is a hybrid Jisp reader:

- normal Jisp forms use the existing Lisp syntax;
- a less-than sign starting a well-formed tag begins a markup element;
- a parenthesized island is parsed with the ordinary Jisp reader;
- markup elements may occur anywhere one Jisp expression can occur;
- ordinary .lisp and .jisp files retain their current grammar and do not gain
  angle-bracket parsing.

The extension is deliberately not .html, .jsx, or .tsx:

- it is not directly parseable by a browser;
- it has Jisp expression semantics, not JavaScript semantics;
- it contains ordinary Jisp code at module scope; and
- it avoids promising JSX or HTML compatibility.

The .jmx spelling is provisional. Phase 0 must reserve it after checking the
package, editor, and documentation naming surface. No parser behavior depends
on the filename beyond Syntax selection.

### 5.2 Minimal grammar

The grammar below is descriptive. The normative grammar and error ranges must
be written before implementation.

~~~text
module       := jisp-form-or-markup*
markup       := open-tag attributes ("/>"
              | ">" child* close-tag)
child        := markup | jisp-form | text-run
attribute    := name "=" quoted-string
              | name "=" jisp-node
open-tag     := "<" tag-name
close-tag    := "</" same-tag-name ">"
jisp-form    := one complete existing Jisp form, beginning with "("
~~~

The reader switches lexing modes:

1. in a start tag it reads names, quoted strings, equals signs, and delimiters;
2. after equals it invokes the ordinary Jisp node reader for exactly one
   complete node, with whitespace, greater-than, and self-closing terminators
   recognized only at the outer markup level. A function call therefore keeps
   its normal parentheses, while true, false, null, 0, and a single symbol
   remain atoms;
3. in element content it recognizes nested tags, complete Jisp forms, or text;
4. it checks the lexical closing tag before any semantic UI validation.

The markup parser must reuse the Jisp node/form reader. It must not reimplement
a slightly different language for attribute values or children.

### 5.3 Example module

~~~jmx
(component profile-link (user)
  <a id=(. user "name")
     href=(str "https://" (get-domain) "/index.html")
     class="inline-flex gap-2"
     class-if:opacity-50=(. user "pending")
     on:click=(emit (ProfileOpened (. user "id")))>
    <span>(text (. user "name"))</span>
  </a>)
~~~

Its normalized Jisp representation is:

~~~lisp
(component profile-link (user)
  (a
    (attr "id" (. user "name"))
    (attr "href" (str "https://" (get-domain) "/index.html"))
    (class "inline-flex" "gap-2")
    (class-if "opacity-50" (. user "pending"))
    (on "click" (emit (ProfileOpened (. user "id"))))
    (span (text (. user "name")))))
~~~

The parenthesized form inside span is intentionally explicit text. A bare
child form continues to have the existing Jisp meaning: it must evaluate to a
UI child node or list of UI child nodes. This avoids an ambiguous and
type-dependent rule where a string expression is sometimes text and sometimes
a component result.

### 5.4 Host tags versus components

Only names in Jisp's current finite host-element registry may be written as
tags in v1:

~~~jmx
<div>...</div>
<input prop:value=(. state "draft") />
~~~

Jisp components remain ordinary child expressions:

~~~jmx
<ul>
  (for title titles
    <li>(todo-row title)</li>)
</ul>
~~~

This is intentional. Current Jisp component calls have positional parameters,
whereas JSX-style component attributes require a new named-prop contract,
prop-spread rules, component namespace syntax, and diagnostics. Adding
<UserCard name=(...)> would silently design all of that. It is deferred until
Jisp explicitly introduces named component inputs.

The parser does not itself consult the host-element registry. It emits the tag
name as a normal Jisp symbol; the existing UI lowerer reports an unknown or
invalid host element with a source span. This keeps parser crates limited to
normalization.

## 6. Exact directive mapping

Every existing UI directive family must have a spelling before the reader is
accepted. This table defines the first proposal.

| Markup spelling | Normalized Jisp form | Notes |
| --- | --- | --- |
| id="profile" | (attr "id" "profile") | Plain quoted attributes are strings. |
| disabled=true | (attr "disabled" true) | Boolean atoms keep their ordinary Jisp meaning. |
| href=(str "https://" domain) | (attr "href" (str "https://" domain)) | The parentheses are the normal Jisp call form. |
| attr:class="external" | (attr "class" "external") | Escapes a reserved markup directive name. |
| prop:value=(. state "draft") | (prop "value" (. state "draft")) | Property name follows prop:. |
| class="rounded px-2" | (class "rounded" "px-2") | Static utility tokens split only on ASCII whitespace. |
| class-if:opacity-50=(. state "pending") | (class-if "opacity-50" (. state "pending")) | Everything after the first colon is the exact class token, including Tailwind-like variants. |
| key=(. item "id") | (key (. item "id")) | No presence shorthand. |
| on:click=(emit Save) | (on "click" (emit Save)) | The value may also be an explicit handler form. |
| on:submit.prevent-default=(emit Save) | (on "submit" (prevent-default) (emit Save)) | Modifiers are a fixed, canonicalized set. |

Reserved leading names are class, class-if, prop, key, on, and attr. Any other
attribute name is an attr directive. The attr: prefix is the explicit escape
for a literal attribute whose name would otherwise be reserved.

The markup name grammar is intentionally narrower than arbitrary Jisp strings.
If a valid existing attr, prop, class, or event name cannot be represented
unambiguously by the table, the markup formatter leaves the entire host element
in canonical Lisp rather than inventing an escaping rule. Markup-to-Lisp stays
total for valid .jmx input; Lisp-to-Markup is total only for the representable
UI subset and otherwise produces a hybrid .jmx module.

In v1:

- quoted attributes are string literals; numbers, booleans, null, and symbols
  retain their Jisp atom spelling, for example tabindex=0, disabled=true, and
  id=user;
- calls and nested expressions retain their normal Jisp form syntax, for
  example href=(str "https://" domain);
- HTML-style boolean-presence shorthand is rejected;
- class=(some-runtime-string) is rejected; use static class plus class-if so
  Jispwind can extract and validate the full utility set;
- JSX prop spreads, XML namespaces, arbitrary style objects, and arbitrary
  inline on-event attributes are rejected; and
- duplicate directives are not resolved by the parser. Existing UI validation
  remains the single authority.

This maps directly to the current source contract and keeps Jispwind's static
class extraction sound. A later target-specific escape hatch must not weaken
portable-1 validation from plan 0020.

## 7. Text, whitespace, escaping, and comments

Markup is not HTML source. It uses tag-shaped syntax but does not inherit a
browser's parser, implicit element closing, entity behavior, case folding,
namespace behavior, or DOM property aliases.

The v1 rules are:

1. A non-whitespace text run becomes an explicit text node with that exact
   decoded source characters.
2. A whitespace-only run containing a line break is formatting indentation and
   is dropped.
3. A whitespace-only run on one line is a literal text node. This permits
   inline spaces between two inline children.
4. To guarantee a literal indentation/newline/less-than-sign/opening-parenthesis
   run, write the existing explicit form, for example (text " "), (text "<"),
   or (text "(").
5. Entity syntax is not interpreted in source. An ampersand is ordinary text;
   the static HTML renderer later escapes output as it already does.
6. Markup comments are not accepted in v1. Jisp comments remain available in
   surrounding Lisp positions and are not part of conversion guarantees.

These rules choose determinism over browser imitation. They prevent pretty
indentation from becoming accidental text while keeping every meaningful text
case expressible through the current explicit text form.

## 8. Parser and crate design

The current Lisp reader is compact but private to the jisp-syntax-lisp crate.
The safest implementation is to keep one reader implementation and add a mode,
not copy it into another crate.

Proposed shape:

~~~text
jisp-core
  Syntax::Markup and .jmx detection only

jisp-syntax-lisp
  LispParser: existing plain mode
  MarkupParser: hybrid mode using the same Reader
  Reader mode decides whether "<tag" is a markup primary expression

jisp
  selects MarkupParser in every parse path

jisp-cli
  recognizes .jmx for check/run/fmt/LSP and conversion output
~~~

The Reader must expose or internally factor one reusable operation for parsing
one complete Jisp expression from its current byte offset. The markup routines
call it after an attribute equals sign or for a child island. The operation
must use the exact existing string, prefix, comment, number, and symbol rules.

All synthetic Nodes use spans from the original .jmx source:

- tag symbols point to the opening tag name;
- an attr/prop/class/event directive spans its complete source attribute;
- the expression child retains its exact Jisp source span;
- literal text spans the text run; and
- an enclosing element form spans from opening less-than to closing greater-than.

No UI semantic validation belongs in this reader. In particular, it does not
know which attrs, props, events, or classes a particular renderer supports.
That preserves the parser/Core IR separation and makes errors flow through
existing lowering, type, JUIR, and host diagnostics.

## 9. Formatter and conversion commands

The existing formatter already proves a useful property for Lisp: parse,
canonical-format, and reparse yield an equivalent NodeKind tree. Markup needs
the same property but must be implemented separately from the generic Lisp
printer.

Add two explicit formatter modes after the reader is proven:

~~~text
jisp fmt --to lisp path.jmx
jisp fmt --to markup path.lisp
~~~

The normal .jmx formatter uses markup output. The --to markup form produces a
hybrid module:

- recognized host elements and their full directive set print as markup;
- components, for, if, text expressions, and arbitrary non-UI code remain
  ordinary Jisp forms wherever that is the exact existing meaning;
- a host element with an unrepresentable directive/name spelling remains an
  ordinary Jisp form in its entirety;
- a form that the printer cannot prove is an existing host element stays Lisp;
- quote, quasiquote, macro templates, and malformed/unlowerable UI stay Lisp
  rather than receiving a guessed tag rendering.

The command must document that it is a semantic conversion. It never promises
to preserve comments or original whitespace. A user wanting a byte-preserving
editor transformation needs a future concrete-syntax tree, which is explicitly
out of scope.

## 10. Delivery plan

### Phase 0: freeze the source contract

Before code:

1. approve the .jmx extension or rename it;
2. approve the v1 directive table, text rules, and explicit component policy;
3. write the normative grammar and a table of parse diagnostics;
4. add examples that parse to the proposed canonical Lisp forms; and
5. decide whether markup is allowed in quoted macro data in v1. The recommended
   answer is yes: it parses to ordinary Nodes before quote/macro expansion.

Do not add JSX compatibility, components-as-tags, or dynamic class strings to
unblock Phase 0.

### Phase 1: reader spike

Implement a private reader mode spike and only parser tests:

- top-level markup and markup nested in a function/component;
- nested markup elements and self-closing elements;
- attribute Jisp forms with nested strings, comments, prefixes, and lists;
- Jisp child forms containing nested markup;
- static text, literal-space text, and indentation-only text;
- every directive in section 6; and
- all malformed closing-tag, quote, delimiter, and embedded-form cases.

Gate A passes only if MarkupParser produces the same NodeKind shape as the
canonical Lisp fixtures and all diagnostics point to .jmx source ranges.

### Phase 2: facade, module, and equivalence wiring

After Gate A:

1. add Syntax::Markup, detection, source-file/import candidates, and facade
   parser dispatch;
2. run markup fixtures through expansion, type inference, interpreter, ui.html,
   and JUIR compilation;
3. add a four-way comparison where applicable: Lisp, JSON, YAML/WS canonical
   source, and Markup all lower to the same UI/JUIR behavior;
4. add LSP diagnostics and language-id/extension tests; and
5. document the source syntax and update CLI error messages.

The expected UI behavior must be exactly the current behavior. No change to
event payloads, effects, reconciliation, JUIR slots, or renderer capabilities
is authorized in this phase.

### Phase 3: canonical printers

Implement the Lisp and Markup conversion paths and property tests:

~~~text
Markup -> Node AST -> Lisp text -> Node AST
Lisp UI subset -> Markup text -> Node AST
Markup -> canonical Markup -> Node AST
~~~

Every arrow must retain NodeKind equality. Add golden fixtures for directives,
escaping, dynamic expressions, nested component calls, if, for, keys, and event
modifiers.

### Phase 4: Jispwind and documentation integration

Add .jmx UI examples to the Jispwind test corpus:

- static class tokens must match the equivalent Lisp program;
- both branches of class-if remain discoverable in the token manifest;
- incompatible portable profile tokens diagnose at the same source attribute;
- generated web CSS and native resolved style manifest remain independent of
  source syntax.

Update UI documentation to make clear that .jmx is an alternate source syntax,
not an HTML template engine and not a browser-run file.

## 11. Validation and acceptance criteria

| Area | Required proof |
| --- | --- |
| Reader equivalence | Every positive .jmx fixture has a canonical Lisp fixture with the same normalized Node AST. |
| Conversion | Both stated round-trip equations hold over a generated corpus and hand-written directive fixtures. |
| Existing semantics | The same markup and Lisp UI program produces equivalent structural UI output, static HTML, and JUIR template/slot/block plans. |
| Macros | Markup in normal code, quote, quasiquote, and imported macro templates has the same expansion and origin behavior as its canonical Lisp AST. |
| Diagnostics | Mismatched tags, unclosed tags, bad strings, missing attr values, bad Jisp islands, and existing duplicate/unknown UI directives point to meaningful .jmx ranges. |
| Whitespace | Fixtures cover indentation dropping, one-line literal spaces, text next to child elements, and explicit text escapes. |
| Styling | class and class-if normalize to existing directives, with no dynamic class-string loophole. |
| Dependencies | The feature adds no Node/npm/JavaScript toolchain and no renderer dependency. |
| Regression | The full allowed workspace test suite passes; existing .lisp, .json, .yaml, and .ws parsing remains unchanged. |

## 12. Risks, rejected alternatives, and review

### Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Readers diverge on embedded Jisp syntax | Use one Reader implementation with a mode; reject a copied Lisp parser. |
| HTML expectations leak into language semantics | State that .jmx is not HTML, define explicit whitespace/entities/booleans, and reject browser-only behavior. |
| JSX compatibility grows without bounds | Do not use JSX as a compatibility target; reject props spread, fragments, namespace rules, and component tags in v1. |
| Dynamic class expressions defeat Jispwind extraction | Keep static class and named class-if as the only portable class syntax. |
| Source-to-source conversion is called lossless | Document AST-level equivalence only; comments/trivia require a future CST. |
| Component attributes implicitly invent named props | Leave component calls as Jisp child forms until a named-component-input design exists. |
| Parser errors become hard to understand | Build errors/ranges before facade integration and preserve original source spans for synthesized directive forms. |

### Alternatives rejected for v1

| Alternative | Why it is rejected |
| --- | --- |
| A React/JSX-compatible parser | It imports JavaScript expression semantics, props/spread behavior, fragment rules, and component conventions that Jisp does not have. |
| A runtime macro that parses strings | It loses normal source ranges, editor support, and parser-level diagnostics; it also makes imports and formatting worse. |
| Browser HTML parsing as the source parser | Browser HTML parsing would impose HTML error recovery, implicit elements, case behavior, and JavaScript/tooling dependencies that do not belong to Jisp. |
| A new MarkupNode Core IR variant | It breaks the one shared AST rule and duplicates UI semantics in later stages. |
| Treat every parenthesized child as dynamic text | It breaks existing component and control-flow child expressions and introduces type-dependent parsing meaning. |
| JSX-style component tags | Existing Jisp components are positional calls, not props objects. |

### Critical review

The proposal is sound only because it is smaller than JSX:

- it normalizes to already-supported UI forms instead of introducing a second
  runtime representation;
- it makes dynamic code visibly Jisp and keeps it evaluated by Jisp;
- it does not pretend browser HTML is portable UI semantics; and
- it preserves the plan 0019 no-VDOM decision and the plan 0020 static
  Jispwind-token discipline.

The hardest part is not parsing angle brackets. It is making text/whitespace,
attribute/directive names, components, and diagnostics explicit enough that
round-trip claims are meaningful. The Phase 0 table and Gate A are mandatory;
if they cannot be agreed, the project should retain the current explicit
S-expression UI syntax rather than ship an ambiguous template language.

## 13. Research sources

- [Racket Scribble at-reader](https://docs.racket-lang.org/scribble/reader.html)
  — reader syntax that translates rich text and nested forms into ordinary
  S-expressions before evaluation.
- [Racket XML X-expressions](https://docs.racket-lang.org/xml/index.html) —
  structural XML representation and source-aware XML reader API; illustrates
  why structure is distinct from original source spelling.
- [SXML representation](https://docs.racket-lang.org/sxml/SXML.html) —
  element/tag/attribute/children mapping into S-expressions.
- [TypeScript JSX documentation](https://www.typescriptlang.org/docs/handbook/jsx)
  — JSX is an embeddable XML-like syntax with implementation-specific lowering;
  its intrinsic-versus-component split motivates an explicit Jisp distinction.
- [Babel JSX transform documentation](https://babel.dev/docs/babel-plugin-transform-react-jsx)
  — fragment, namespace, and spread/runtime policy surface deliberately omitted
  from the first Jisp Markup release.
- [Jisp declarative UI contract](../../docs/UI.md) — current host elements,
  directives, child semantics, renderer boundary, and comment behavior.
- [Jisp source syntax and parser contract](../../docs/SPEC.md) — one
  source-aware AST across syntax readers.
- [Jisp compiled portable UI runtime plan](0019-compiled-portable-ui-runtime.md)
  — JUIR boundary, source-map requirement, no-VDOM decision, and cross-reader
  validation approach.
- [Jispwind/Blitz plan](0020-blitz-jispwind-renderer.md) — static utility
  tokens, capability profiles, and renderer-neutral styling constraints.

# Plan: Optional Blitz renderer and Jispwind utility styles

**Status:** Research-backed proposal; no implementation has been started.
**Date:** 2026-07-14
**Depends on:** [0019-compiled-portable-ui-runtime.md](0019-compiled-portable-ui-runtime.md)
**Decision type:** Architecture and staged validation plan

## 1. Purpose and non-negotiable constraints

Jisp needs a portable UI path which:

1. preserves the renderer-neutral JUIR contract described in plan 0019;
2. does not introduce a generic Virtual DOM into the Jisp evaluator;
3. can render interactive web UI through ordinary HTML/CSS/DOM;
4. can optionally target a CSS-layout native renderer on desktop, then validate
   Android and iOS separately;
5. offers a small, Tailwind-shaped set of utility classes based on declared
   capabilities;
6. does not require Node, npm, a JavaScript configuration file, or the Dioxus
   runtime; and
7. keeps every non-web renderer dependency out of a web-only dependency graph.

This is deliberately **not** a promise of full Tailwind compatibility, exact
pixel identity between all operating systems, a web-browser implementation, or
automatic SwiftUI/GTK/KDE/WinUI translation. Those have different semantic and
accessibility models. The portable promise is a versioned, tested subset of UI
semantics and style utilities, with an explicit diagnostic for features outside
that subset.

Plan 0019 remains the source of truth for JUIR, template identity, patch
streams, event serialization, and the host-capability boundary. This plan adds
an optional rendering/style line; it does not revise the language contract.

## 2. Conclusions from the broader research

| Question | Evidence | Planning conclusion |
| --- | --- | --- |
| Can Blitz be used without Dioxus? | blitz-dom exposes a native Rust DOM/document API; Dioxus is one consumer rather than a prerequisite. | Yes. Build a Jisp-owned JUIR-to-Blitz writer directly on blitz-dom; do not include dioxus-core, dioxus-html, signals, hooks, or its runtime. |
| Is Blitz ready to be the universal renderer today? | Blitz identifies itself as alpha and not ready for production. Its public WPT CSS score is 47.50%, and its status page lists important gaps including partial filters/backdrop filters, touch-action, SVG styling, and transformed hit testing. | No. It is an experimental native backend behind a hard POC gate, not a default renderer or a compatibility claim. |
| Does Dioxus native DOM solve the JUIR writer? | It contains a useful mutation writer, but it maps Dioxus element identifiers and VDOM mutations into Blitz nodes. Its event module still contains many explicitly unimplemented event families. | Reimplement the small mapping for JUIR patches. Do not adopt the VDOM bridge or its event abstraction. |
| Does official Tailwind remove JavaScript from the toolchain? | The standalone CLI removes the need to install Node, but Tailwind's published standalone distribution packages its JavaScript implementation. Tailwind v4 is CSS-first, not a stable Rust utility-compiler API. | If “no Node/npm” is the requirement, the official binary remains an optional build tool. If the requirement is “no JavaScript implementation at all”, build a narrowly-scoped Rust Jispwind compiler instead. This plan chooses the latter for the portable core. |
| Is there a mature Rust Tailwind drop-in? | Existing Rust projects describe incomplete or non-identical Tailwind grammar and have limited production evidence. Lightning CSS is a capable Rust CSS parser/transformer, not a Tailwind utility generator. | Do not make an immature Tailwind-compatible crate a foundational dependency. Start with a Jisp-owned, versioned utility registry. Consider Lightning CSS only as a later optional build optimization. |
| Can web remain JavaScript-free? | Rust/Wasm DOM interaction uses generated wasm-bindgen glue. It can avoid handwritten application JavaScript, but an interactive Wasm DOM bundle still ships a small generated bridge. | Make this an explicit product choice: (A) no Node and no authored JS, allowing generated glue for interactive web, or (B) zero shipped JS, limiting web output to static/SSR HTML. Do not imply both are simultaneously available. |
| Can a capability model genuinely help portability? | Design Tokens Community Group format defines platform-agnostic token interchange. Cross-platform UI research consistently separates abstract UI from concrete/final UI and documents unavoidable target trade-offs. | Keep semantic UI in JUIR, compile tokens and utilities through target profiles, and reject unsupported capabilities before rendering. |

The key outcome is therefore:

> Use standard HTML/CSS/DOM for web. Validate a direct, optional Blitz backend
> for native desktop. Share JUIR, design tokens, and a deliberately limited
> utility specification, not a browser engine or a VDOM.

## 3. Target architecture

~~~text
Jisp UI source
  -> typed structural UI values
  -> JUIR templates + slots + keyed patch operations       (plan 0019)
  -> Jispwind utility resolution + target capability check
       |                                      |
       | web profile                          | blitz-1 profile
       v                                      v
static CSS + standard DOM patch host     JispBlitzWriter -> blitz-dom
Wasm/SSR application boundary                         -> blitz-paint / blitz-shell
~~~

The same JUIR patch stream drives both paths. The renderer owns retained nodes
and applies those patches directly:

- the browser host owns DOM node references;
- the Blitz host owns NodeId references from blitz-dom;
- neither path asks the evaluator to create, diff, or retain a generic VDOM;
- utility class parsing/validation occurs before a renderer sees the result;
- host effects such as storage, timers, and navigation remain in the existing
  WIT capability boundary. CSS, node patching, and per-frame UI updates must
  never be transferred through WIT.

“Same on every platform” means the same specified utility-to-property mapping,
layout semantics where both profiles support them, and the same diagnostics. It
does **not** mean identical font rasterization, input-method behavior, window
chrome, color-management behavior, or unsupported CSS behavior.

## 4. Dependency and Cargo boundary

Do not place Blitz behind an additive feature on a crate that all Jisp
applications use. Cargo features are additive across the resolution graph; a
single transitive feature user could otherwise bring WGPU/windowing/native
renderer dependencies into a web-only product.

The intended boundary is:

~~~text
jisp-ui                 renderer-neutral values, JUIR contract, no Blitz
jisp-wasm               existing web/DOM path, no Blitz dependency
jisp-jispwind           token schema, utility registry, CSS emitter, no shell
jisp-ui-blitz           optional native renderer adapter
  -> blitz-dom
  -> blitz-paint
  -> blitz-shell
  -> blitz-traits       only if its public traits are needed by the adapter
~~~

jisp-ui-blitz should be an opt-in workspace member/client dependency. A small
facade feature such as jisp-ui/blitz may be added only if it forwards to that
separate crate and remains disabled by default; it must not be the primary
isolation mechanism.

Initial dependency rules:

- no dioxus crate;
- no direct taffy dependency merely because Blitz uses it internally;
- no blitz-html or blitz-net unless a later, separately approved feature
  requires parsing external HTML or networking;
- select narrow Blitz features and disable defaults where supported;
- keep blitz-shell platform-specific and out of library/core crates;
- record the exact upstream Blitz revision and license review in dependency
  documentation when code is introduced.

Before accepting a backend, CI must prove:

~~~text
cargo tree -p jisp-wasm      contains no blitz, wgpu, winit, or dioxus
cargo tree -p jisp-ui        contains no blitz or dioxus
cargo tree -p jisp-ui-blitz  contains no dioxus
~~~

The specific command forms may change with package topology, but the graph
assertion is an acceptance criterion.

## 5. Jispwind: a narrow portable utility specification

### 5.1 Name and contract

Jispwind means “Tailwind-shaped utility classes for Jisp,” not “all current
Tailwind syntax and behavior.” Using a distinct name prevents false
compatibility expectations and gives the language a stable, reviewable
specification.

Inputs:

- DTCG-compatible JSON design tokens as the portable interchange format;
- a Jisp-owned profile configuration naming token sets and capability profile;
- literal class tokens and statically enumerable class-if branches from JUIR.

Outputs:

- a generated CSS file for the web profile;
- a compact resolved-style/property representation for native renderers;
- a manifest mapping each utility to required capabilities, token references,
  and generated CSS/native properties;
- build-time diagnostics for unknown, ambiguous, or profile-incompatible
  utilities.

The compiler should extract every static class token. Both arms of a class-if
must be known at build time. Arbitrary runtime-built class strings are outside
portable-profile v1 because they make CSS extraction and cross-backend
validation unbounded.

### 5.2 Initial capability profiles

Define profile data, versioned with Jispwind, rather than scattering
renderer-specific checks:

~~~text
web-1:
  normal HTML/CSS DOM profile

blitz-1:
  only behavior demonstrated in Jisp's own Blitz test suite

portable-1:
  intersection of web-1 and blitz-1; a class accepted here has the same
  specified mapping in both profiles
~~~

The portable v1 utility set should be intentionally modest:

- layout: display, flex direction/wrap, alignment, justified content, grid
  without subgrid, gap, normal flow and non-fixed positioning only after
  evidence;
- sizing and spacing: token-based width/height/min/max, margin, padding;
- visual basics: token colors, opacity, border width/style/color, radius,
  background color, overflow, and only demonstrated shadows;
- typography: token font family/size/weight/line height, alignment, wrapping;
- state variants only where the corresponding event/focus model is proven.

Initially exclude from portable-1:

- filters, backdrop filters, text shadows, advanced blend modes;
- SVG styling;
- 3D transforms and any transform needing pointer hit testing;
- subgrid, complex table layout, pseudo-element content, and browser-specific
  form control styling;
- touch-action/pointer-event utilities until native behavior is verified;
- animation/transition utilities until rendering, event, and reduced-motion
  behavior are tested.

A target-specific profile may eventually support more. It must never silently
downgrade a portable-1 claim; a utility either produces its declared behavior
or the compiler reports the requirement that is unavailable.

### 5.3 Exactness and tokens

Use named design tokens for spacing, type, colors, radii, and shadows. This
avoids platform-dependent ad-hoc values and lets one manifest document which
CSS property/native property each utility controls.

Pixel-perfect screenshots are not a portable conformance definition: different
text engines and font fallback make that impossible without tightly bundled
fonts and rendering settings. The conformance definition should instead be:

1. identical resolved property values expressed in the profile's canonical
   units;
2. equivalent semantic accessibility tree and focus order;
3. deterministic layout fixtures within declared tolerances; and
4. visual goldens per target, reviewed as target baselines rather than compared
   byte-for-byte across targets.

## 6. Direct Blitz adapter, without a VDOM

The optional adapter, tentatively named JispBlitzWriter, consumes JUIR template
instantiation and JUIR patch operations. It maintains only the renderer's
necessary identity map:

~~~text
JUIR template node / keyed block identity -> blitz-dom NodeId
~~~

For example, it must implement direct operations for creating elements/text,
setting attributes and resolved styles, inserting/removing/reordering children,
and changing text. It should then request style/layout/paint through the
document and selected shell/paint integration.

This is a retained renderer adapter, not a Virtual DOM:

- JUIR already supplies identity and change operations;
- no Jisp-side tree-diff pass is introduced;
- no Dioxus VirtualDom, ElementId, component runtime, hooks, signals, or
  mutation protocol is included;
- host events are translated into the minimal JUIR event snapshot specified by
  plan 0019, rather than adopting all Dioxus event types.

The Dioxus/Blitz mutation writer is useful prior art but is not a dependency or
implementation source. If a later change copies or closely adapts any part of
it, the change must include an exact pinned source permalink and license review,
as required by AGENTS.md, for example:

- [Dioxus native DOM mutation writer at Blitz revision
  1973351bc8f26310b4fcdcedcd15cf55ed5dc107](https://github.com/DioxusLabs/blitz/blob/1973351bc8f26310b4fcdcedcd15cf55ed5dc107/packages/dioxus-native-dom/src/mutation_writer.rs)

No such code is proposed for copying in this plan.

## 7. Staged delivery with hard gates

### Phase 0: freeze the product interpretation

Resolve this policy before implementation:

| Policy | Meaning | Consequence |
| --- | --- | --- |
| no-node-no-authored-js | No Node/npm and no handwritten JS application code. Generated Wasm browser glue is allowed. | Interactive web DOM is possible. |
| zero-shipped-js | No JavaScript artifact is shipped at all. | Web output is SSR/static HTML and CSS only; no client interactivity. |

The recommended default is no-node-no-authored-js, stated honestly in project
documentation. It matches Rust/Wasm practice while keeping the source and build
tooling JavaScript-free from the product author's perspective.

Also select the first supported native target: Linux desktop is the appropriate
POC target. Android and iOS must be explicit later validation targets, not
implied by winit or Blitz dependency metadata.

### Phase 1: isolated Blitz feasibility spike

Create an isolated, non-default jisp-ui-blitz spike with a fixed JUIR fixture
set. Do not alter the evaluator, language syntax, or current default web path.

The spike must demonstrate:

1. initial construction of container, text, button, text input, and keyed list;
2. JUIR-driven text/property/class update, insertion, removal, and keyed
   reorder without reconstructing unaffected nodes;
3. layout fixtures covering flex, basic grid, spacing, clipping, borders,
   radius, color, and the proposed first typography set;
4. pointer, keyboard, focus, controlled text-input, selection, and IME
   composition behavior relevant to Jisp's portable event model;
5. an inspectable AccessKit accessibility tree for label, button, text input,
   list, focus order, and disabled state;
6. a screenshot/golden harness plus structural tree assertions; and
7. an audited dependency graph with no Dioxus packages.

### Gate A: permission to build a native backend

Proceed only when all Phase 1 fixtures pass on the first target and the
following conditions are met:

- every proposed portable-1 utility has a tested CSS/native mapping;
- unsupported Blitz features never silently pass through as a portable utility;
- keyed patches preserve identity/focus where the JUIR contract requires it;
- controlled input and composition do not lose text or focus;
- the semantic accessibility fixture is exposed and correct;
- no core/web package gained Blitz, shell, WGPU, winit, or Dioxus dependencies;
- the dependency/license audit records exact versions and feature flags; and
- measured startup/binary-size/render-cost budgets are written before broadening
  scope.

Failing IME, accessibility, or keyed identity is a **no-go** for the native
interactive backend, not a reason to reduce the public portability claim.

The global WPT percentage is context, not the gate. The gate is Jisp's precise
subset, tested against the target runtime. Any regression in upstream Blitz
status requires rerunning the affected fixture set.

### Phase 2: implement the Jispwind compiler

After Gate A, implement an independent Rust compiler/library in this order:

1. DTCG token ingestion and schema validation;
2. a closed portable-1 utility registry with capability requirements;
3. static token extraction from JUIR metadata;
4. web CSS emission and deterministic manifest emission;
5. native resolved-style emission from the same registry;
6. profile diagnostics and a machine-readable support report; and
7. unit/contract tests that compare web and Blitz resolution for every
   portable utility.

Avoid a default dependency on a general CSS transform/minifier. Add Lightning
CSS only if a measured web build requirement justifies its API and alpha-version
risk. Do not attempt parser compatibility with arbitrary Tailwind plug-ins,
JavaScript configuration, or arbitrary-value grammar in this phase.

### Phase 3: integrate a supported native desktop backend

Wire JispBlitzWriter to the existing JUIR host interface behind a separate
native application package. Add:

- a documented capability profile version (blitz-1);
- runtime diagnostics naming the exact unsupported capability;
- visual/structural/a11y regression tests;
- feature and dependency-tree CI checks; and
- a reproducible, no-Node build command for native and web assets.

Keep the existing semantic in-memory native adapter. It remains useful for
fast tests and for native widget experiments, and it must not be replaced by
Blitz merely because Blitz exists.

### Phase 4: web pair

Retain the standard DOM renderer for the web target. Generate static CSS from
the same Jispwind registry and apply the same JUIR patches to DOM nodes.

For the no-node-no-authored-js policy, document the minimal generated
wasm-bindgen bootstrap as a build artifact. Audit it for determinism and do not
write a parallel handwritten JavaScript runtime. For the zero-shipped-js
policy, provide only SSR/static output and reject eventful JUIR programs at
build time.

Web must never depend on Blitz, winit, WGPU, or native shell code.

### Phase 5: mobile and broader renderer validation

Run dedicated Android and iOS POCs with the Phase 1 fixture matrix. Do not
advertise support based solely on winit's platform list. Accept each platform
only after its input, accessibility, lifecycle, graphics, and packaging tests
pass.

Separately investigate true native-widget targets (SwiftUI, Android Views or
Compose, GTK, KDE/Qt, WinUI). Those require a semantic-widget adapter and a
smaller common control vocabulary; they are not an extension of CSS rendering
and should not constrain the Blitz/web milestone.

## 8. Capability model and diagnostics

Keep two different capability layers:

| Layer | Carries | Does not carry |
| --- | --- | --- |
| Jisp host capabilities/WIT | coarse external effects such as storage, timer, navigation, dialogs, network policy | UI nodes, CSS, patches, renderer internals |
| Jispwind target profile | style/property/event prerequisites of a named renderer profile | permission grants or host-effect calls |

Each utility definition must record:

~~~text
utility name
-> design-token inputs
-> canonical property mapping
-> required capability identifiers
-> allowed profiles
-> fallback policy (normally: diagnostic, never silent omission)
~~~

A build report should distinguish:

- unknown utility (spelling/specification error);
- known but excluded from portable-1;
- supported only by an opted-in target profile;
- unsupported due to missing renderer capability; and
- an invalid token/value for the selected profile.

This makes capability drift reviewable and prevents “looks almost right” from
becoming the effective compatibility policy.

## 9. Test matrix and success criteria

The first end-to-end test matrix should contain:

| Area | Required evidence |
| --- | --- |
| JUIR patches | create, replace text, style/class update, keyed insert/remove/reorder, disposal |
| Layout | flex, basic grid, alignment, gap, sizing, overflow, borders/radius |
| Input | pointer, keyboard, tab focus, disabled controls, controlled input, selection, IME composition |
| Accessibility | role/name/value/state, labels, list semantics, focus order, disabled state via AccessKit/native tree and browser accessibility tree |
| Styling | token resolution, every portable-1 utility's CSS/native mapping, negative unsupported-class tests |
| Visuals | target-local golden images under stable fonts/settings, reviewed diffs |
| Dependencies | cargo-tree deny assertions for web/core, direct dependency manifest for native |
| Builds | no Node/npm command path, generated-artifact audit where interactive Wasm is selected |

The plan succeeds when a small documented program has one JUIR representation,
one token set, and the same accepted portable-1 utility set on web and the
first native target, with all differences either intentionally profile-scoped or
reported before runtime.

## 10. Risks and mitigations

| Risk | Impact | Mitigation / decision |
| --- | --- | --- |
| Blitz remains incomplete or regresses | Native renderer cannot meet the portable subset | Keep it optional; Gate A blocks integration; retain DOM web and semantic test adapter. |
| CSS coverage tempts scope creep | “Tailwind support” becomes a misleading claim | Call it Jispwind; publish the closed utility matrix and profile versions. |
| Text/IME/accessibility gaps | Serious usability and platform failure | Treat them as hard gates, not later polish. |
| Mobile support is inferred rather than proven | Unsupported platform promise | Separate Android/iOS POCs and accept independently. |
| Cargo feature leakage | Web download/build size and compile cost rise | Separate crates, default-off facade, and graph checks in CI. |
| “No JS” remains ambiguous | Unmet product/tooling expectations | Freeze one of the two Phase 0 policies and state generated glue explicitly. |
| Dioxus code is over-adopted | VDOM/runtime coupling and dependency growth | Depend on no Dioxus crate; own a small direct writer. |
| Cross-target visual mismatch | False claims of pixel equality | Specify semantic/property parity; use target-local visual baselines. |

## 11. Review of this plan

### What the plan gets right

- It preserves the central Jisp decision: JUIR patches are the retained update
  protocol, so a generic VDOM is unnecessary.
- It borrows only the difficult native rendering foundation from Blitz while
  explicitly avoiding Dioxus's component/runtime dependency cone.
- It keeps the web target on the browser's DOM/CSS engine, which is the mature
  accessibility and compatibility path.
- It turns the user-visible portability promise into a small capability matrix
  with diagnostics rather than an untestable “Tailwind everywhere” slogan.
- It prevents a native renderer feature from contaminating a web-only build.

### Hard objections that remain valid

- Blitz's present public compatibility evidence is too limited for a production
  cross-platform promise. The proposal must be rejected or deferred if Phase 1
  cannot prove Jisp's precise subset.
- A fresh Jispwind compiler is still new code. It is justified only by the
  strict no-JavaScript-implementation requirement; if that requirement relaxes,
  the official standalone Tailwind CLI may be cheaper for web-only asset
  generation.
- Interactive browser Wasm currently needs generated interop glue. Anyone
  requiring literally no JavaScript bytes must accept static/SSR-only web.
- SwiftUI, GTK/KDE, WinUI, and Android native widgets cannot be obtained merely
  by translating CSS classes. They need a separate semantic control backend.
- Accessibility and IME are likely the highest-risk areas; an attractive
  screenshot result is not enough.

### Alternatives rejected for the first milestone

| Alternative | Reason |
| --- | --- |
| Dioxus Desktop/Mobile as the backend | It brings a VDOM-oriented runtime and currently defaults to system WebView on those targets; it does not satisfy the minimal direct-renderer goal. |
| Dioxus native DOM package | It still depends on Dioxus core/HTML types and has incomplete event support. |
| Blitz as the web renderer | It would replace the mature browser platform with an incomplete engine and adds native graphics dependencies to a target that already has DOM/CSS. |
| Official Tailwind as the portable core | It is excellent for its intended web workflow, but its implementation/tooling does not meet strict zero-JS-core requirements and its web CSS cannot map automatically to native semantics. |
| One giant feature-gated UI crate | Cargo feature additivity makes dependency isolation unreliable. |
| Claim a universal native-widget backend now | The required semantic vocabulary and platform adapters are separate work; claiming them would hide the difficult parts. |

### Approval checklist

Before Phase 1 starts, confirm all of the following:

- [ ] The selected web policy is written as either no-node-no-authored-js or
  zero-shipped-js.
- [ ] Linux desktop is accepted as the sole first native POC target.
- [ ] The first Jispwind release is named/versioned as a closed portable-1
  profile, not advertised as full Tailwind compatibility.
- [ ] IME, focus preservation, and accessibility are hard Go/No-Go criteria.
- [ ] The native backend remains an opt-in package and jisp-wasm remains
  dependency-clean.
- [ ] Any later use of external implementation code includes a pinned source
  link and license review.

## 12. Research sources

### Primary implementation sources

- [Blitz repository](https://github.com/DioxusLabs/blitz) — modular renderer
  workspace, package boundaries, license metadata, and current revision
  context.
- [Blitz DOM crate documentation](https://docs.rs/crate/blitz-dom/latest) —
  direct Rust document/DOM APIs without Dioxus.
- [Blitz CSS status](https://blitz.is/status) — support gaps used to bound
  portable-1.
- [Blitz WPT status](https://blitz.is/status/wpt) — current 47.50% CSS
  compatibility context; not used as a product acceptance proxy.
- [Blitz project status](https://blitz.is/about) — explicit work-in-progress
  and production-readiness positioning.
- [Dioxus 0.7 renderer/project documentation](https://dioxuslabs.com/learn/0.7/beyond/project_structure/) —
  experimental status of the native Blitz renderer and WebView defaults.
- [Dioxus native DOM mutation writer, pinned upstream revision](https://github.com/DioxusLabs/blitz/blob/1973351bc8f26310b4fcdcedcd15cf55ed5dc107/packages/dioxus-native-dom/src/mutation_writer.rs) —
  prior art only; no code has been copied.

### Styling and web-build sources

- [Tailwind CLI documentation](https://tailwindcss.com/docs/installation/tailwind-cli?web=1) —
  standalone binary workflow without a Node installation.
- [Tailwind standalone CLI announcement](https://tailwindcss.com/blog/standalone-cli) —
  packaging of the JavaScript implementation inside the original standalone
  executable.
- [Tailwind CSS v4 announcement](https://tailwindcss.com/blog/tailwindcss-v4) —
  CSS-first configuration direction and compatibility context.
- [Lightning CSS Rust documentation](https://docs.rs/lightningcss/latest/lightningcss/index.html) —
  optional CSS transform/minification candidate, not a utility compiler.
- [wasm-bindgen DOM example](https://wasm-bindgen.github.io/wasm-bindgen/examples/dom.html) —
  Rust/Wasm DOM bridge and generated browser glue context.

### Portability and accessibility sources

- [Design Tokens Community Group format, 2025.10](https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/) —
  platform-agnostic token interchange basis.
- [AccessKit](https://accesskit.dev/) — cross-platform accessibility
  infrastructure used as a native acceptance target.
- [winit platform support](https://rust-windowing.github.io/winit/winit/) —
  platform reach is not treated as renderer readiness.
- [Cameleon: A multi-target end-user development environment](https://www.sciencedirect.com/science/article/abs/pii/S0953543803000109) —
  abstract/concrete/final UI separation as model-driven portability context.
- [Cross-platform development approaches survey](https://www.sciencedirect.com/science/article/pii/S2090447915001276) —
  trade-off context; no universal native-UI translation is assumed.

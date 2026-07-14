import { Decoration, EditorView, keymap, ViewPlugin } from "https://esm.sh/@codemirror/view@6.43.6";
import { Compartment, EditorState, RangeSetBuilder } from "https://esm.sh/@codemirror/state@6.7.1";
import { StreamLanguage, syntaxTree } from "https://esm.sh/@codemirror/language@6.12.4";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "https://esm.sh/@codemirror/commands@6.10.4";
import { clojure } from "https://esm.sh/@codemirror/legacy-modes@6.5.0/mode/clojure";
import { json } from "https://esm.sh/@codemirror/lang-json@6.0.2";
import { yaml } from "https://esm.sh/@codemirror/lang-yaml@6.1.2";
import { oneDark } from "https://esm.sh/@codemirror/theme-one-dark@6.1.2";

const assetVersion = new URL(import.meta.url).searchParams.get("v") || "dev";
const wasmModule = await import(`./pkg/jisp_wasm.js?v=${encodeURIComponent(assetVersion)}`);
const { default: init, convert_source, PlaygroundSession } = wasmModule;

const examples = [
  ["Todo updates", "examples/todos.lisp"],
  ["Product launch board", "examples/kanban.lisp"],
  ["Tiny rituals", "examples/habits.lisp"],
  ["Personal spend", "examples/finance.lisp"],
];

const app = document.getElementById("app");

app.innerHTML = `
  <header class="border-b border-slate-800 bg-slate-950">
    <div class="mx-auto flex max-w-[1600px] flex-col gap-4 px-5 py-5 md:flex-row md:items-center md:justify-between">
      <div>
        <p class="text-sm font-semibold uppercase tracking-[0.2em] text-cyan-400">Experimental</p>
        <h1 class="mt-1 text-2xl font-bold text-white">Jisp UI playground</h1>
      </div>
      <div class="flex items-center gap-4">
        <label class="flex items-center gap-3 text-sm font-medium text-slate-300">
          Example
          <select id="example" class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-white outline-none focus:border-cyan-400"></select>
        </label>
        <a href="https://github.com/shirk33y/jisp" target="_blank" rel="noreferrer" class="rounded-lg border border-slate-700 px-3 py-2 text-sm font-semibold text-slate-200 transition hover:border-cyan-400 hover:text-cyan-300">GitHub ↗</a>
      </div>
    </div>
  </header>
  <main class="mx-auto max-w-[1600px] px-5 py-6">
    <p class="mb-5 max-w-4xl text-sm leading-6 text-slate-400">
      Jisp compiles and evaluates in WebAssembly. Browser events become plain values, Jisp turns them
      into actions, and the update function returns the next immutable state. The preview only renders the
      structural tree and forwards events; it never evaluates Jisp or owns application state.
    </p>
    <div class="grid gap-6 lg:grid-cols-2">
      <section class="editor-shell overflow-hidden rounded-2xl border border-slate-800 bg-slate-900 shadow-2xl">
        <div class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
          <div>
            <h2 class="font-semibold text-white">Jisp UI</h2>
            <fieldset class="mt-2 flex gap-1" aria-label="Source syntax">
              <label class="cursor-pointer rounded-md px-2 py-1 text-xs font-semibold text-slate-300 has-[:checked]:bg-cyan-400 has-[:checked]:text-slate-950"><input class="sr-only" type="radio" name="syntax" value="lisp" checked> Lisp</label>
              <label class="cursor-pointer rounded-md px-2 py-1 text-xs font-semibold text-slate-300 has-[:checked]:bg-cyan-400 has-[:checked]:text-slate-950"><input class="sr-only" type="radio" name="syntax" value="json"> JSON</label>
              <label class="cursor-pointer rounded-md px-2 py-1 text-xs font-semibold text-slate-300 has-[:checked]:bg-cyan-400 has-[:checked]:text-slate-950"><input class="sr-only" type="radio" name="syntax" value="yaml"> YAML</label>
              <label class="cursor-pointer rounded-md px-2 py-1 text-xs font-semibold text-slate-300 has-[:checked]:bg-cyan-400 has-[:checked]:text-slate-950"><input class="sr-only" type="radio" name="syntax" value="ws"> WS</label>
            </fieldset>
          </div>
          <button id="reset" class="rounded-md px-2 py-1 text-xs font-semibold text-cyan-300 hover:bg-slate-800">Reset</button>
        </div>
        <div id="editor" aria-label="Jisp UI source"></div>
      </section>
      <section class="preview-shell overflow-hidden rounded-2xl border border-slate-800 bg-white shadow-2xl">
        <div class="flex items-center justify-between border-b border-slate-200 bg-white px-4 py-3">
          <h2 class="font-semibold text-slate-900">Rendered preview</h2>
          <span id="status" class="rounded-full bg-amber-100 px-2 py-1 text-xs font-semibold text-amber-700" aria-live="polite">Loading Wasm</span>
        </div>
        <pre id="error" class="m-4 hidden whitespace-pre-wrap rounded-xl border border-rose-200 bg-rose-50 p-4 font-mono text-sm text-rose-800"></pre>
        <iframe id="preview" title="Jisp UI preview" sandbox="allow-scripts" class="preview-surface block min-h-[34rem] w-full border-0"></iframe>
      </section>
    </div>
  </main>
`;

const select = document.getElementById("example");
const reset = document.getElementById("reset");
const preview = document.getElementById("preview");
const error = document.getElementById("error");
const status = document.getElementById("status");
const syntaxInputs = [...document.querySelectorAll('input[name="syntax"]')];
let editor;
let initialSource = "";
let syntax = "lisp";
let ready = false;
let renderTimer;
let latestTree = null;
let session = null;
const language = new Compartment();
const clojureLanguage = StreamLanguage.define(clojure);
const jsonLanguage = json();
const yamlLanguage = yaml();
const rainbowBracketClasses = [
  "cm-rainbow-bracket-0",
  "cm-rainbow-bracket-1",
  "cm-rainbow-bracket-2",
  "cm-rainbow-bracket-3",
  "cm-rainbow-bracket-4",
];
const rainbowBracketDecorations = rainbowBracketClasses.map((className) => Decoration.mark({ class: className }));
const rainbowBracketTheme = EditorView.baseTheme({
  ".cm-rainbow-bracket-0": { color: "#f472b6", fontWeight: "700" },
  ".cm-rainbow-bracket-1": { color: "#facc15", fontWeight: "700" },
  ".cm-rainbow-bracket-2": { color: "#4ade80", fontWeight: "700" },
  ".cm-rainbow-bracket-3": { color: "#38bdf8", fontWeight: "700" },
  ".cm-rainbow-bracket-4": { color: "#c084fc", fontWeight: "700" },
});
const rainbowBrackets = ViewPlugin.fromClass(class {
  constructor(view) {
    this.decorations = rainbowBracketRanges(view);
  }

  update(update) {
    this.decorations = rainbowBracketRanges(update.view);
  }
}, {
  decorations: (plugin) => plugin.decorations,
});

function rainbowBracketRanges(view) {
  const builder = new RangeSetBuilder();
  const stack = [];
  const pairs = { "(": ")", "[": "]", "{": "}" };
  const text = view.state.doc.toString();
  const tree = syntaxTree(view.state);
  for (let position = 0; position < text.length; position += 1) {
    const token = tree.resolveInner(position, 1).name;
    if (/String|Comment/.test(token)) continue;
    const character = text[position];
    if (pairs[character]) {
      const depth = stack.length;
      builder.add(position, position + 1, rainbowBracketDecorations[depth % rainbowBracketDecorations.length]);
      stack.push({ close: pairs[character], depth });
      continue;
    }
    const opening = stack.at(-1);
    if (opening?.close === character) {
      builder.add(position, position + 1, rainbowBracketDecorations[opening.depth % rainbowBracketDecorations.length]);
      stack.pop();
    }
  }
  return builder.finish();
}
const wsLanguage = StreamLanguage.define({
  startState() {
    return { head: false };
  },
  token(stream, state) {
    if (stream.sol()) {
      stream.eatSpace();
      state.head = true;
    }
    if (stream.eatSpace()) return null;
    if (stream.match(";")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/"(?:[^"\\]|\\.)*"/)) return "string";
    if (stream.match(/-?\d+(?:\.\d+)?/)) return "number";
    if (stream.match(/\.\.\./)) return "keyword";
    if (stream.match(/[^\s]+/)) {
      const token = stream.current();
      const style = state.head ? "keyword" : /^(true|false|null)$/.test(token) ? "bool" : "variableName";
      state.head = false;
      return style;
    }
    stream.next();
    return null;
  },
});

for (const [name, path] of examples) {
  const option = document.createElement("option");
  option.value = path;
  option.textContent = name;
  select.append(option);
}

function setStatus(kind, text) {
  status.className = `rounded-full px-2 py-1 text-xs font-semibold ${kind}`;
  status.textContent = text;
}

function setRuntimeStatus(label) {
  const metrics = JSON.parse(session.metrics());
  const execution = metrics.execution;
  const reused = execution
    ? execution.reusedSlots + execution.reusedBlocks + execution.reusedComponents
    : 0;
  const detail = metrics.lastRenderSkipped
    ? "render skipped"
    : reused > 0
      ? `${reused} JUIR value${reused === 1 ? "" : "s"} reused`
      : null;
  setStatus("bg-emerald-100 text-emerald-700", detail ? `${label} · ${detail}` : label);
  status.title = JSON.stringify(metrics, null, 2);
}

function previewDocument() {
  return `<!doctype html><html><head><meta charset="utf-8"><style>html { overflow-y: scroll; scrollbar-gutter: stable; }</style><script src="https://cdn.tailwindcss.com"><\/script></head><body class="min-h-screen bg-slate-50 p-4 md:p-8"><div id="root"></div><script>
const allowedTags = new Set(["a", "article", "aside", "button", "div", "footer", "form", "h1", "h2", "h3", "header", "img", "input", "label", "li", "main", "nav", "ol", "option", "p", "section", "select", "span", "strong", "textarea", "ul"]);
const allowedEvents = new Set(["blur", "change", "click", "focus", "input", "keydown", "keyup", "submit"]);
const root = document.getElementById("root");
let eventSequence = 0;

function safeAttribute(element, name, value) {
  if (name.startsWith("on") || value === null || value === false) return false;
  if ((name === "href" || name === "src") && typeof value === "string" && /^javascript:/i.test(value)) return false;
  if (value === true) element.setAttribute(name, "");
  else element.setAttribute(name, String(value));
  return true;
}

function browserEvent(event) {
  const target = event.target;
  return {
    type: event.type,
    value: target && "value" in target ? target.value : null,
    checked: target && "checked" in target ? target.checked : null,
    key: event.key || null,
  };
}

function treeKey(tree) {
  if (tree?.kind !== "element" || tree.key === null || tree.key === undefined) return null;
  return \`\${typeof tree.key}:\${JSON.stringify(tree.key)}\`;
}

function isElementTree(tree) {
  return tree?.kind === "element" && allowedTags.has(tree.tag);
}

function matchesTree(existing, tree) {
  if (tree?.kind === "text") return existing?.nodeType === Node.TEXT_NODE;
  return isElementTree(tree)
    && existing?.nodeType === Node.ELEMENT_NODE
    && existing.tagName.toLowerCase() === tree.tag;
}

function createNode(tree, path) {
  if (tree.kind === "text") return document.createTextNode(String(tree.value ?? ""));
  if (tree.kind !== "element" || !allowedTags.has(tree.tag)) return document.createComment("invalid Jisp UI node");
  const element = document.createElement(tree.tag);
  patchElement(element, tree, path);
  return element;
}

function patchNode(parent, existing, tree, path) {
  if (!matchesTree(existing, tree)) {
    const created = createNode(tree, path);
    if (existing?.parentNode === parent) parent.replaceChild(created, existing);
    else parent.append(created);
    return created;
  }
  if (tree.kind === "text") {
    const value = String(tree.value ?? "");
    if (existing.data !== value) existing.data = value;
  } else {
    patchElement(existing, tree, path);
  }
  return existing;
}

function patchElement(element, tree, path) {
  element.dataset.jispPath = path;
  syncAttributes(element, tree.attrs || {});
  syncProperties(element, tree.props || {});
  syncClasses(element, tree.classes || []);
  syncEvents(element, tree.events || {});
  reconcileChildren(element, tree.children || [], path);
  element.__jispKey = treeKey(tree);
}

function syncAttributes(element, attrs) {
  const previous = element.__jispAttrs || new Set();
  const next = new Set();
  for (const [name, value] of Object.entries(attrs)) {
    if (safeAttribute(element, name, value)) next.add(name);
  }
  for (const name of previous) {
    if (!next.has(name)) element.removeAttribute(name);
  }
  element.__jispAttrs = next;
}

function resetProperty(element, name) {
  if (typeof element[name] === "boolean") element[name] = false;
  else if (typeof element[name] === "string") element[name] = "";
  else element[name] = null;
}

function syncProperties(element, props, sequence = null) {
  const previous = element.__jispProps || new Map();
  const next = new Map();
  for (const [name, value] of Object.entries(props)) {
    if (name.startsWith("on") || !(name in element)) continue;
    next.set(name, value);
    if (name === "value"
      && element === document.activeElement
      && Number.isInteger(sequence)
      && element.__jispInputSequence > sequence) continue;
    if (!Object.is(previous.get(name), value) && !Object.is(element[name], value)) element[name] = value;
  }
  for (const name of previous.keys()) {
    if (!next.has(name)) resetProperty(element, name);
  }
  element.__jispProps = next;
}

function syncClasses(element, classes) {
  const previous = element.__jispClasses || new Set();
  const next = new Set(classes.filter((name) => typeof name === "string"));
  for (const name of previous) {
    if (!next.has(name)) element.classList.remove(name);
  }
  for (const name of next) {
    if (!previous.has(name)) element.classList.add(name);
  }
  element.__jispClasses = next;
}

function syncEvents(element, events) {
  const records = element.__jispEvents || new Map();
  for (const [name, record] of records) {
    const next = eventDescriptor(events[name]);
    if (!allowedEvents.has(name) || !next || next.policy.capture !== record.policy.capture) {
      element.removeEventListener(name, record.listener, record.policy.capture);
      records.delete(name);
    }
  }
  for (const [name, encoded] of Object.entries(events)) {
    const next = eventDescriptor(encoded);
    if (!allowedEvents.has(name) || !next) continue;
    let record = records.get(name);
    if (!record) {
      record = {
        handler: next.handler,
        policy: next.policy,
        listener(event) {
          if (record.policy.preventDefault) event.preventDefault();
          if (record.policy.stopPropagation) event.stopPropagation();
          const sequence = ++eventSequence;
          if (event.target && "value" in event.target) event.target.__jispInputSequence = sequence;
          parent.postMessage({ type: "jisp-event", handler: record.handler, event: browserEvent(event), sequence }, "*");
        },
      };
      records.set(name, record);
      element.addEventListener(name, record.listener, record.policy.capture);
    }
    record.handler = next.handler;
    record.policy = next.policy;
  }
  element.__jispEvents = records;
}

function eventDescriptor(value) {
  if (Number.isInteger(value)) {
    return {
      handler: value,
      policy: { preventDefault: false, stopPropagation: false, capture: false },
    };
  }
  if (!value || !Number.isInteger(value.handler) || typeof value.policy !== "object") return null;
  const policy = value.policy;
  if (typeof policy.preventDefault !== "boolean"
    || typeof policy.stopPropagation !== "boolean"
    || typeof policy.capture !== "boolean") return null;
  return { handler: value.handler, policy };
}

function reconcileChildren(parent, trees, path) {
  const existing = [...parent.childNodes];
  const keyed = new Map();
  const unkeyed = [];
  for (const child of existing) {
    if (child.__jispKey !== null && child.__jispKey !== undefined) keyed.set(child.__jispKey, child);
    else unkeyed.push(child);
  }

  const rendered = [];
  let unkeyedIndex = 0;
  for (const [index, tree] of trees.entries()) {
    const key = treeKey(tree);
    const current = key === null ? unkeyed[unkeyedIndex++] : keyed.get(key);
    const child = patchNode(parent, current, tree, path + "." + index);
    child.__jispKey = key;
    rendered.push(child);
  }

  for (const [index, child] of rendered.entries()) {
    const current = parent.childNodes[index];
    if (current !== child) parent.insertBefore(child, current || null);
  }
  const retained = new Set(rendered);
  for (const child of [...parent.childNodes]) {
    if (!retained.has(child)) child.remove();
  }
}

function focusedControl() {
  const element = document.activeElement;
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement)) return null;
  return {
    element,
    path: element.dataset.jispPath,
    start: element.selectionStart,
    end: element.selectionEnd,
  };
}

function restoreFocus(focus) {
  if (!focus?.path) return;
  const element = root.contains(focus.element)
    ? focus.element
    : root.querySelector('[data-jisp-path="' + focus.path + '"]');
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement)) return;
  element.focus({ preventScroll: true });
  if (Number.isInteger(focus.start) && Number.isInteger(focus.end)) element.setSelectionRange(focus.start, focus.end);
}

function nodeAt(path) {
  const parts = String(path).split(".");
  if (parts.shift() !== "0") return null;
  let node = root.firstChild;
  for (const part of parts) {
    if (!node || !/^\\d+$/.test(part)) return null;
    node = node.childNodes[Number(part)] || null;
  }
  return node;
}

function applyPatches(patches, sequence) {
  const focus = focusedControl();
  for (const patch of patches) {
    const node = nodeAt(patch.path);
    if (patch.op === "text") {
      if (node?.nodeType !== Node.TEXT_NODE) return false;
      node.data = String(patch.value ?? "");
      continue;
    }
    if (patch.op === "element") {
      if (node?.nodeType !== Node.ELEMENT_NODE) return false;
      if (Object.hasOwn(patch, "attrs")) syncAttributes(node, patch.attrs || {});
      if (Object.hasOwn(patch, "props")) syncProperties(node, patch.props || {}, sequence);
      if (Object.hasOwn(patch, "classes")) syncClasses(node, patch.classes || []);
      if (Object.hasOwn(patch, "events")) syncEvents(node, patch.events || {});
      continue;
    }
    if (patch.op === "children") {
      if (node?.nodeType !== Node.ELEMENT_NODE || !Array.isArray(patch.trees)) return false;
      reconcileChildren(node, patch.trees, patch.path);
      continue;
    }
    if (patch.op === "replace") {
      const parts = String(patch.path).split(".");
      const index = Number(parts.pop());
      const parent = parts.length ? nodeAt(parts.join(".")) : root;
      if (!parent || !Number.isInteger(index) || !patch.tree) return false;
      patchNode(parent, parent.childNodes[index], patch.tree, patch.path);
      continue;
    }
    return false;
  }
  restoreFocus(focus);
  return true;
}

addEventListener("message", (message) => {
  if (message.source !== parent) return;
  if (message.data?.type === "jisp-render") {
    const focus = focusedControl();
    const current = root.firstChild;
    const next = patchNode(root, current, message.data.tree, "0");
    for (const child of [...root.childNodes]) {
      if (child !== next) child.remove();
    }
    restoreFocus(focus);
    return;
  }
  if (message.data?.type === "jisp-patches" && !applyPatches(message.data.patches || [], message.data.sequence)) {
    parent.postMessage({ type: "jisp-recover" }, "*");
  }
});
<\/script></body></html>`;
}

function postTree(tree) {
  latestTree = tree;
  preview.contentWindow?.postMessage({ type: "jisp-render", tree }, "*");
}

function postPatches(patches, sequence) {
  latestTree = null;
  preview.contentWindow?.postMessage({ type: "jisp-patches", patches, sequence }, "*");
}

function sourceText() {
  return editor.state.doc.toString();
}

function renderPreview() {
  if (!ready) return;
  try {
    session = new PlaygroundSession();
    postTree(JSON.parse(session.load_syntax(sourceText(), syntax)));
    error.classList.add("hidden");
    setRuntimeStatus("Update ready");
  } catch (reason) {
    session = null;
    status.removeAttribute("title");
    postTree({ kind: "text", value: "" });
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Jisp error");
  }
}

function setSource(text) {
  editor.dispatch({ changes: { from: 0, to: editor.state.doc.length, insert: text } });
}

function setEditorLanguage() {
  const nextLanguage = {
    lisp: clojureLanguage,
    json: jsonLanguage,
    yaml: yamlLanguage,
    ws: wsLanguage,
  }[syntax];
  editor.dispatch({ effects: language.reconfigure(nextLanguage) });
}

async function loadExample(path) {
  setStatus("bg-amber-100 text-amber-700", "Loading example");
  try {
    const response = await fetch(`${path}?v=${encodeURIComponent(assetVersion)}`, { cache: "no-store" });
    if (!response.ok) throw new Error(`Could not load ${path}: ${response.status}`);
    initialSource = await response.text();
    syntax = "lisp";
    syntaxInputs.find((input) => input.value === syntax).checked = true;
    setEditorLanguage();
    setSource(initialSource);
    renderPreview();
  } catch (reason) {
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Load error");
  }
}

preview.addEventListener("load", () => {
  if (latestTree) postTree(latestTree);
  else if (session) postTree(JSON.parse(session.snapshot()));
});
preview.srcdoc = previewDocument();

window.addEventListener("message", (message) => {
  if (message.source !== preview.contentWindow || !session) return;
  if (message.data?.type === "jisp-recover") {
    postTree(JSON.parse(session.snapshot()));
    return;
  }
  if (message.data?.type !== "jisp-event") return;
  try {
    const update = JSON.parse(session.dispatch_patches(message.data.handler, JSON.stringify(message.data.event)));
    postPatches(update.patches, message.data.sequence);
    error.classList.add("hidden");
    setRuntimeStatus("State updated");
  } catch (reason) {
    status.removeAttribute("title");
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Update error");
  }
});

editor = new EditorView({
  state: EditorState.create({
    doc: "",
    extensions: [
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
      EditorView.lineWrapping,
      language.of(clojureLanguage),
      oneDark,
      rainbowBracketTheme,
      rainbowBrackets,
      EditorView.updateListener.of((update) => {
        if (!update.docChanged) return;
        clearTimeout(renderTimer);
        renderTimer = setTimeout(renderPreview, 220);
      }),
    ],
  }),
  parent: document.getElementById("editor"),
});

select.addEventListener("change", () => loadExample(select.value));
syntaxInputs.forEach((input) => input.addEventListener("change", () => {
  if (!input.checked || input.value === syntax) return;
  try {
    const converted = convert_source(sourceText(), syntax, input.value);
    syntax = input.value;
    setEditorLanguage();
    setSource(converted);
    renderPreview();
    setStatus("bg-emerald-100 text-emerald-700", `Converted to ${syntax.toUpperCase()}`);
  } catch (reason) {
    input.checked = false;
    syntaxInputs.find((current) => current.value === syntax).checked = true;
    error.textContent = `Cannot convert invalid ${syntax.toUpperCase()} source: ${reason}`;
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Conversion error");
  }
}));
reset.addEventListener("click", () => {
  try {
    setSource(convert_source(initialSource, "lisp", syntax));
    renderPreview();
  } catch (reason) {
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Reset error");
  }
});

try {
  await init({ module_or_path: new URL(`./pkg/jisp_wasm_bg.wasm?v=${encodeURIComponent(assetVersion)}`, import.meta.url) });
  ready = true;
  await loadExample(select.value);
} catch (reason) {
  error.textContent = `WebAssembly failed to load: ${reason}`;
  error.classList.remove("hidden");
  setStatus("bg-rose-100 text-rose-700", "Wasm unavailable");
}

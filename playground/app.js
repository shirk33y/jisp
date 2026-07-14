import { EditorView, keymap } from "https://esm.sh/@codemirror/view@6.43.6";
import { Compartment, EditorState } from "https://esm.sh/@codemirror/state@6.7.1";
import { StreamLanguage } from "https://esm.sh/@codemirror/language@6.12.4";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "https://esm.sh/@codemirror/commands@6.10.4";
import { clojure } from "https://esm.sh/@codemirror/legacy-modes@6.5.0/mode/clojure";
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
      <label class="flex items-center gap-3 text-sm font-medium text-slate-300">
        Example
        <select id="example" class="rounded-lg border border-slate-700 bg-slate-900 px-3 py-2 text-white outline-none focus:border-cyan-400"></select>
      </label>
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

function previewDocument() {
  return `<!doctype html><html><head><meta charset="utf-8"><style>html { overflow-y: scroll; scrollbar-gutter: stable; }</style><script src="https://cdn.tailwindcss.com"><\/script></head><body class="min-h-screen bg-slate-50 p-4 md:p-8"><div id="root"></div><script>
const allowedTags = new Set(["a", "article", "aside", "button", "div", "footer", "form", "h1", "h2", "h3", "header", "img", "input", "label", "li", "main", "nav", "ol", "option", "p", "section", "select", "span", "strong", "textarea", "ul"]);
const allowedEvents = new Set(["blur", "change", "click", "focus", "input", "keydown", "keyup", "submit"]);
const root = document.getElementById("root");

function safeAttribute(element, name, value) {
  if (name.startsWith("on") || value === null || value === false) return;
  if ((name === "href" || name === "src") && typeof value === "string" && /^javascript:/i.test(value)) return;
  if (value === true) element.setAttribute(name, "");
  else element.setAttribute(name, String(value));
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

function node(tree, path) {
  if (tree.kind === "text") return document.createTextNode(String(tree.value ?? ""));
  if (tree.kind !== "element" || !allowedTags.has(tree.tag)) return document.createComment("invalid Jisp UI node");
  const element = document.createElement(tree.tag);
  element.dataset.jispPath = path;
  for (const [name, value] of Object.entries(tree.attrs || {})) safeAttribute(element, name, value);
  for (const [name, value] of Object.entries(tree.props || {})) {
    if (!name.startsWith("on") && name in element) element[name] = value;
  }
  for (const name of tree.classes || []) element.classList.add(name);
  for (const [name, handler] of Object.entries(tree.events || {})) {
    if (!allowedEvents.has(name) || !Number.isInteger(handler)) continue;
    element.addEventListener(name, (event) => {
      if (name === "submit") event.preventDefault();
      parent.postMessage({ type: "jisp-event", handler, event: browserEvent(event) }, "*");
    });
  }
  for (const [index, child] of (tree.children || []).entries()) element.append(node(child, path + "." + index));
  return element;
}

function focusedControl() {
  const element = document.activeElement;
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement)) return null;
  return {
    path: element.dataset.jispPath,
    start: element.selectionStart,
    end: element.selectionEnd,
  };
}

function restoreFocus(focus) {
  if (!focus?.path) return;
  const element = root.querySelector('[data-jisp-path="' + focus.path + '"]');
  if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement)) return;
  element.focus({ preventScroll: true });
  if (Number.isInteger(focus.start) && Number.isInteger(focus.end)) element.setSelectionRange(focus.start, focus.end);
}

addEventListener("message", (message) => {
  if (message.source !== parent || message.data?.type !== "jisp-render") return;
  const focus = focusedControl();
  root.replaceChildren(node(message.data.tree, "0"));
  restoreFocus(focus);
});
<\/script></body></html>`;
}

function postTree(tree) {
  latestTree = tree;
  preview.contentWindow?.postMessage({ type: "jisp-render", tree }, "*");
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
    setStatus("bg-emerald-100 text-emerald-700", "Update ready");
  } catch (reason) {
    session = null;
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
  editor.dispatch({ effects: language.reconfigure(syntax === "ws" ? wsLanguage : clojureLanguage) });
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
});
preview.srcdoc = previewDocument();

window.addEventListener("message", (message) => {
  if (message.source !== preview.contentWindow || message.data?.type !== "jisp-event" || !session) return;
  try {
    postTree(JSON.parse(session.dispatch(message.data.handler, JSON.stringify(message.data.event))));
    error.classList.add("hidden");
    setStatus("bg-emerald-100 text-emerald-700", "State updated");
  } catch (reason) {
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

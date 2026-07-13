import init, { render_html } from "./pkg/jisp_wasm.js";

const examples = [
  ["Welcome card", "examples/welcome.lisp"],
  ["Todo list", "examples/todos.lisp"],
  ["Profile", "examples/profile.lisp"],
  ["Notifications", "examples/notifications.lisp"],
  ["Dashboard", "examples/dashboard.lisp"],
  ["Settings", "examples/settings.lisp"],
  ["Product card", "examples/product.lisp"],
  ["Navigation", "examples/navigation.lisp"],
  ["Empty state", "examples/empty-state.lisp"],
  ["Project board", "examples/projects.lisp"],
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
      This page runs the real Jisp interpreter compiled to WebAssembly. JavaScript only loads the module,
      edits source, and places the escaped HTML result in an isolated preview frame. Tailwind styles the
      preview; it is not part of Jisp semantics.
    </p>
    <div class="grid gap-6 lg:grid-cols-2">
      <section class="editor-shell overflow-hidden rounded-2xl border border-slate-800 bg-slate-900 shadow-2xl">
        <div class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
          <h2 class="font-semibold text-white">Jisp UI</h2>
          <button id="reset" class="rounded-md px-2 py-1 text-xs font-semibold text-cyan-300 hover:bg-slate-800">Reset</button>
        </div>
        <textarea id="source" class="block w-full bg-slate-900 p-5 font-mono text-sm leading-6 text-slate-100 outline-none" spellcheck="false" aria-label="Jisp UI source"></textarea>
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
const source = document.getElementById("source");
const reset = document.getElementById("reset");
const preview = document.getElementById("preview");
const error = document.getElementById("error");
const status = document.getElementById("status");
let initialSource = "";
let ready = false;
let renderTimer;

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

function previewDocument(html) {
  return `<!doctype html><html><head><meta charset="utf-8"><script src="https://cdn.tailwindcss.com"><\/script></head><body class="min-h-screen bg-slate-50 p-4 md:p-8">${html}</body></html>`;
}

function renderPreview() {
  if (!ready) return;
  try {
    preview.srcdoc = previewDocument(render_html(source.value));
    error.classList.add("hidden");
    setStatus("bg-emerald-100 text-emerald-700", "Rendered by Wasm");
  } catch (reason) {
    preview.srcdoc = "";
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Jisp error");
  }
}

async function loadExample(path) {
  setStatus("bg-amber-100 text-amber-700", "Loading example");
  try {
    const response = await fetch(path);
    if (!response.ok) throw new Error(`Could not load ${path}: ${response.status}`);
    initialSource = await response.text();
    source.value = initialSource;
    renderPreview();
  } catch (reason) {
    error.textContent = String(reason);
    error.classList.remove("hidden");
    setStatus("bg-rose-100 text-rose-700", "Load error");
  }
}

select.addEventListener("change", () => loadExample(select.value));
reset.addEventListener("click", () => {
  source.value = initialSource;
  renderPreview();
});
source.addEventListener("input", () => {
  clearTimeout(renderTimer);
  renderTimer = setTimeout(renderPreview, 180);
});

try {
  await init();
  ready = true;
  await loadExample(select.value);
} catch (reason) {
  error.textContent = `WebAssembly failed to load: ${reason}`;
  error.classList.remove("hidden");
  setStatus("bg-rose-100 text-rose-700", "Wasm unavailable");
}

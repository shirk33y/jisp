#!/usr/bin/env bash
# Exercise the deployed browser-host contract with a real Wasm build. The
# browser test is deliberately black-box: it drives the sandboxed preview via
# its accessibility tree and reads only the host's diagnostic probe.

set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
playground="$repository_root/playground"
if [[ ! -f "$playground/pkg/jisp_wasm.js" || ! -f "$playground/pkg/jisp_wasm_bg.wasm" ]]; then
  echo "The playground Wasm package is missing. Build it first with:" >&2
  echo "  wasm-pack build crates/jisp-wasm --target web --out-dir ../../playground/pkg --out-name jisp_wasm" >&2
  exit 1
fi

for command in agent-browser python3; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Required command is unavailable: $command" >&2
    exit 1
  fi
done

port=${JISP_PLAYGROUND_PORT:-$(python3 - <<'PY'
import socket

with socket.socket() as socket_:
    socket_.bind(("127.0.0.1", 0))
    print(socket_.getsockname()[1])
PY
)}
session="jisp-playground-regression-$$"
server_log=$(mktemp)

cleanup() {
  agent-browser --session "$session" close >/dev/null 2>&1 || true
  if [[ -n "${server_pid:-}" ]]; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -f "$server_log"
}
trap cleanup EXIT

python3 -m http.server "$port" --bind 127.0.0.1 --directory "$playground" >"$server_log" 2>&1 &
server_pid=$!

agent-browser --session "$session" open "http://127.0.0.1:$port"
agent-browser --session "$session" wait --load networkidle
agent-browser --session "$session" wait --fn 'document.querySelector("#status")?.textContent.includes("Update ready")'

snapshot=$(agent-browser --session "$session" snapshot -i -c)
input_ref=$(printf '%s\n' "$snapshot" | sed -n 's/.*textbox "What needs doing?" \[ref=\([^]]*\)\].*/@\1/p' | head -1)
if [[ -z "$input_ref" ]]; then
  echo "The Todo controlled input did not render." >&2
  printf '%s\n' "$snapshot" >&2
  exit 1
fi

request_probe() {
  agent-browser --session "$session" eval --stdin <<'JS'
(() => {
const preview = document.querySelector("#preview");
if (!preview?.contentWindow) throw new Error("Preview frame is unavailable");
delete preview.dataset.jispHostProbe;
preview.contentWindow.postMessage({ type: "jisp-host-probe" }, "*");
})();
JS
  agent-browser --session "$session" wait --fn 'Boolean(document.querySelector("#preview")?.dataset.jispHostProbe)'
}

agent-browser --session "$session" focus "$input_ref"
request_probe
agent-browser --session "$session" eval --stdin <<'JS'
(() => {
const preview = document.querySelector("#preview");
const probe = JSON.parse(preview.dataset.jispHostProbe);
if (probe.active?.tag !== "INPUT" || probe.active.identity !== probe.firstControl?.identity) {
  throw new Error("The controlled Todo input did not receive browser focus");
}
if (probe.viewport.overflowY !== "scroll" || probe.viewport.scrollbarGutter !== "stable") {
  throw new Error(`The preview must reserve its scrollbar gutter, got ${JSON.stringify(probe.viewport)}`);
}
preview.dataset.jispExpectedControlIdentity = String(probe.firstControl.identity);
preview.dataset.jispExpectedClientWidth = String(probe.viewport.clientWidth);
})();
JS

agent-browser --session "$session" fill "$input_ref" "focus survives incremental update"
agent-browser --session "$session" wait --fn 'document.querySelector("#status")?.textContent.includes("State updated")'
request_probe
agent-browser --session "$session" eval --stdin <<'JS'
(() => {
const preview = document.querySelector("#preview");
const probe = JSON.parse(preview.dataset.jispHostProbe);
const expectedIdentity = preview.dataset.jispExpectedControlIdentity;
if (String(probe.firstControl?.identity) !== expectedIdentity || probe.active?.identity !== probe.firstControl?.identity) {
  throw new Error("The controlled input was recreated or lost focus after its reducer update");
}
if (probe.firstControl.value !== "focus survives incremental update") {
  throw new Error(`Controlled input value was lost: ${probe.firstControl.value}`);
}
})();
JS

snapshot=$(agent-browser --session "$session" snapshot -i -c)
input_ref=$(printf '%s\n' "$snapshot" | sed -n 's/.*textbox "What needs doing?" \[ref=\([^]]*\)\].*/@\1/p' | head -1)
if [[ -z "$input_ref" ]]; then
  echo "The controlled input disappeared after its update." >&2
  exit 1
fi
agent-browser --session "$session" focus "$input_ref"
agent-browser --session "$session" press Control+a
request_probe
agent-browser --session "$session" eval --stdin <<'JS'
(() => {
const preview = document.querySelector("#preview");
const probe = JSON.parse(preview.dataset.jispHostProbe);
const control = probe.firstControl;
if (probe.active?.identity !== control?.identity
  || control.selectionStart !== 0
  || control.selectionEnd !== control.value.length) {
  throw new Error(`The focused input selection was not retained: ${JSON.stringify(probe)}`);
}
})();
JS

agent-browser --session "$session" click "#hydrate-ssr"
agent-browser --session "$session" wait --fn 'document.querySelector("#status")?.textContent.includes("SSR hydrated")'
request_probe
agent-browser --session "$session" eval --stdin <<'JS'
(() => {
const preview = document.querySelector("#preview");
const probe = JSON.parse(preview.dataset.jispHostProbe);
const control = probe.firstControl;
if (String(control?.identity) !== preview.dataset.jispExpectedControlIdentity
  || control.value !== "focus survives incremental update"
  || control.selectionStart !== 0
  || control.selectionEnd !== control.value.length) {
  throw new Error(`SSR hydration replaced or reset the controlled input: ${JSON.stringify(probe)}`);
}
if (probe.viewport.clientWidth !== Number(preview.dataset.jispExpectedClientWidth)) {
  throw new Error(`Preview width changed across an update: ${JSON.stringify(probe.viewport)}`);
}
})();
JS

errors=$(agent-browser --session "$session" errors)
if [[ -n "$errors" ]]; then
  echo "The playground emitted browser errors:" >&2
  printf '%s\n' "$errors" >&2
  exit 1
fi

echo "verified playground focus, selection, hydration, and scrollbar stability"

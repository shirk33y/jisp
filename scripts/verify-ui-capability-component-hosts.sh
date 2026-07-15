#!/usr/bin/env bash
# Verify the deterministic WIT component through two independent Component
# Model hosts. The component itself remains the only ABI source; this script
# deliberately consumes its compiled wasm output rather than Rust internals.

set -euo pipefail

component=${1:-target/wasm32-wasip2/release/jisp_ui_capability_component.wasm}
wasmtime_bin=${WASMTIME_BIN:-wasmtime}
jco_version=${JCO_VERSION:-1.25.2}
preview2_shim_version=${PREVIEW2_SHIM_VERSION:-0.17.9}

if [[ ! -f "$component" ]]; then
  echo "Component artifact does not exist: $component" >&2
  echo "Build it with: cargo build -p jisp-ui-capability-component --target wasm32-wasip2 --release" >&2
  exit 1
fi

if ! command -v "$wasmtime_bin" >/dev/null 2>&1; then
  echo "Wasmtime executable is unavailable: $wasmtime_bin" >&2
  exit 1
fi

for command in node npx npm; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "Required command is unavailable: $command" >&2
    exit 1
  fi
done

assert_contains() {
  local actual=$1
  local expected=$2
  local context=$3
  if [[ "$actual" != *"$expected"* ]]; then
    echo "$context did not contain expected text: $expected" >&2
    echo "actual: $actual" >&2
    exit 1
  fi
}

invoke_wasmtime() {
  "$wasmtime_bin" run --invoke "$1" "$component"
}

supported=$(invoke_wasmtime 'supported()')
assert_contains "$supported" 'storage.write' 'Wasmtime supported()'
assert_contains "$supported" 'timer.sleep' 'Wasmtime supported()'

assert_contains \
  "$(invoke_wasmtime 'storage-write({key: "draft:1", value-json: "{\"title\":\"Plan\"}"})')" \
  'ok' \
  'Wasmtime storage-write valid request'
assert_contains \
  "$(invoke_wasmtime 'storage-write({key: "", value-json: "null"})')" \
  'storage.write@1 requires a nonempty key' \
  'Wasmtime storage-write invalid request'
assert_contains \
  "$(invoke_wasmtime 'timer-sleep({milliseconds: 0})')" \
  'timer.sleep@1 requires milliseconds greater than zero' \
  'Wasmtime timer-sleep invalid request'
assert_contains \
  "$(invoke_wasmtime 'navigate({target: "/settings", replace: false})')" \
  'navigate@1 is not enabled by the deterministic fixture host' \
  'Wasmtime navigate unsupported request'

temporary_directory=$(mktemp -d)
cleanup() {
  rm -rf "$temporary_directory"
}
trap cleanup EXIT

npx --yes "@bytecodealliance/jco@$jco_version" transpile "$component" -o "$temporary_directory" >/dev/null
(
  cd "$temporary_directory"
  npm init -y >/dev/null
  npm pkg set type=module
  npm install --no-save "@bytecodealliance/preview2-shim@$preview2_shim_version" >/dev/null
)

node --input-type=module - "$temporary_directory" <<'NODE'
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

const directory = process.argv[2];
const module = await import(
  pathToFileURL(`${directory}/jisp_ui_capability_component.js`).href,
);
const { capabilities } = module;

function outcome(operation) {
  try {
    operation();
    return { ok: true };
  } catch (error) {
    return { error: error.payload ?? { message: error.message } };
  }
}

assert.deepEqual(capabilities.supported(), [
  { name: 'storage.write', version: 1 },
  { name: 'timer.sleep', version: 1 },
]);
assert.deepEqual(
  outcome(() =>
    capabilities.storageWrite({
      key: 'draft:1',
      valueJson: JSON.stringify({ title: 'Plan' }),
    }),
  ),
  { ok: true },
);
assert.deepEqual(
  outcome(() => capabilities.storageWrite({ key: '', valueJson: 'null' })),
  {
    error: {
      code: 'invalid-request',
      message: 'storage.write@1 requires a nonempty key',
    },
  },
);
assert.deepEqual(
  outcome(() => capabilities.timerSleep({ milliseconds: 0n })),
  {
    error: {
      code: 'invalid-request',
      message: 'timer.sleep@1 requires milliseconds greater than zero',
    },
  },
);
assert.deepEqual(
  outcome(() => capabilities.timerSleep({ milliseconds: 1n })),
  { ok: true },
);
assert.deepEqual(
  outcome(() => capabilities.navigate({ target: '/settings', replace: false })),
  {
    error: {
      code: 'unsupported-capability',
      message: 'navigate@1 is not enabled by the deterministic fixture host',
    },
  },
);
NODE

echo "verified jisp-ui-capability-component with Wasmtime and JCO/Node"

# Plan: LSP incremental sync and protocol conformance (3 hours)

**Status:** proposed. **Estimate:** 3 hours. This hardens the experimental
editor endpoint; it changes neither Jisp source semantics nor the compiler
pipeline.

## Why this is a real 3-hour slice

The current endpoint advertises full document sync and has useful direct helper
tests, but the actual stdio protocol loop is untested. It retains only text,
uses the last `contentChanges` item, and does not model document versions. A
correct incremental-sync boundary needs a small state model, UTF-16 range
application, protocol-frame tests, and explicit stale/malformed-edit policy.
Those are distinct correctness boundaries, not one cosmetic LSP tweak.

## Goal

Make `jisp lsp` a deterministic incremental-sync server: ordered changes apply
against the currently opened document, requests observe the newest valid text,
and the public JSON-RPC stream proves initialization, diagnostics, hover,
definition, and close semantics.

## Timeboxed work

### 0:00–0:25 — freeze the protocol contract and find the seam

1. Trace `lsp`, framing, document storage, diagnostics, hover, and definition
   in `crates/jisp-cli`.
2. Write the local contract before implementation:
   - advertise `TextDocumentSyncKind::Incremental` (`2`);
   - `didOpen` establishes URI, version, and complete text;
   - `didChange` applies its changes in listed order to that exact version;
   - an older version is ignored without replacing good text;
   - invalid ranges/UTF-16 positions do not panic or partially mutate a
     document; and
   - `didClose` clears state and publishes empty diagnostics.
3. Keep document contents in memory only. Do not add filesystem watches,
   workspace indexing, or background workers.

### 0:25–1:20 — extract a testable server and document state

1. Move the protocol loop out of the oversized CLI entrypoint into a focused
   `lsp` module. Keep the CLI command as a thin stdin/stdout adapter.
2. Introduce a private document record: `text` plus accepted integer version.
   Make state transitions explicit instead of storing bare strings in a map.
3. Implement full replacement changes and ranged incremental changes. Convert
   LSP `(line, character)` positions from UTF-16 code units to byte offsets
   without splitting a scalar or accepting a position beyond a line.
4. Apply multiple changes sequentially; on any invalid change, preserve the
   previous document as a whole. Publish diagnostics only for committed text.
5. Reuse the existing frontend diagnostics, hover, and definition helpers;
   do not duplicate parsing/typechecking in the protocol module.

### 1:20–2:20 — protocol-level regression suite

Test the extracted loop with framed JSON-RPC bytes via in-memory reader/writer,
not just direct helper calls. Add named cases for:

1. `initialize` advertises incremental sync and existing capabilities;
2. `didOpen` publishes a parser/type diagnostic with Jisp code and UTF-16
   range;
3. a ranged edit after an emoji repairs the document and clears diagnostics;
4. two ordered ranged edits in one notification produce the expected final
   document, not only the final replacement;
5. a full replacement remains accepted for conservative clients;
6. an old version cannot overwrite a newer valid document;
7. an invalid UTF-16 boundary/range leaves the prior document usable by hover
   or definition and does not crash the server;
8. hover and definition after a valid change resolve from the updated text;
9. `didClose` publishes an empty diagnostic set and later requests return
   `null` rather than stale results.

Keep existing direct range/definition tests; the new suite proves wire and
lifecycle integration.

### 2:20–2:45 — diagnostics and documentation alignment

1. Add only the documentation necessary to state incremental sync, in-memory
   document lifetime, UTF-16 positions, and stale-edit behavior.
2. Update `docs/TESTING.md` or `docs/DIAGNOSTICS.md` with the protocol-level
   coverage boundary so future changes do not downgrade it to helper tests.
3. If refactoring reveals a frontend diagnostic range defect, repair it at the
   shared source/diagnostic seam and add the smallest regression. Do not turn
   LSP into a separate diagnostic renderer.

### 2:45–3:00 — full gate and handoff

Run:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
cargo test -p jisp-macros --quiet
```

Commit one conventional patch. Record the accepted document/version rules and
the exact protocol scenarios proven.

## Done when

- The server advertises and correctly applies incremental sync with UTF-16
  positions and ordered multi-change notifications.
- Invalid or stale edits never corrupt the last valid open document.
- Stdio JSON-RPC framing and the full open/change/request/close lifecycle have
  regression coverage.
- Hover, definition, and diagnostics read one shared current document state.
- No workspace indexing, file watching, async runtime, or language feature is
  introduced.

## Cut line

Do not add completion ranking, rename, references, semantic tokens, code
actions, file watching, multi-root workspaces, package resolution over LSP, or
native-code diagnostics. Those need separate protocol and performance
contracts.

# Plan: offline package determinism (about 3 hours)

**Status:** proposed. **Estimate:** 2.5–3.5 hours.  This is a resolver and CLI
hardening slice; it adds no remote registry, package format, or language
feature.

## Why this is a real 3-hour slice

The package contract crosses four independently testable boundaries: manifest
parsing, local-index-to-cache locking, lockfile-to-resolver loading, and CLI
behaviour.  The work below targets 10–12 cases in two crates, including
temporary package projects and failure diagnostics.  Any implementation fix
must preserve the no-network invariant.

## Goal

Make a package locked from a local registry deterministic, offline, and
tamper-evident from `jisp lock` through `jisp run`/facade resolution.  Record
every verified boundary in tests and keep `docs/PACKAGES.md` exact.

## Timeboxed work

### 0:00–0:30 — contract and gap ledger

Trace `jisp lock`, local index loading, lock rendering/parsing, cache lookup,
and facade resolution.  Make a checked-in ledger with each condition below,
its current test, and its final proof.  Reproduce any suspected gap before
editing production code.

### 0:30–1:35 — lock creation and cache tests

Add CLI-level tests for these independent outcomes:

1. lock output is byte-identical on a second run;
2. registry entries are sorted independently of manifest order;
3. an unused pre-existing registry lock entry is removed or preserved exactly
   as the documented policy requires;
4. local index source and manifest checksum disagreement fails before cache
   write;
5. malformed checksum, missing index checksum, and missing index source fail
   with actionable diagnostics;
6. cache filename generation cannot collide for distinct source extensions or
   unsafe package/version spelling.

Use temporary directories and assert filesystem effects: a failed lock must not
leave a trusted new cache entry or a partially rewritten lockfile.

### 1:35–2:25 — locked resolution and no-network tests

Add facade/module-resolution tests for:

7. a valid cache resolves after the local registry directory is removed;
8. changed cache bytes fail the recorded SHA-256 check;
9. missing cache, version mismatch, and manifest-vs-lock checksum mismatch
   remain distinct errors;
10. a locked source path escaping the project cache is rejected or is handled
    exactly as the existing documented trust boundary specifies;
11. `http://` and `https://` registry declarations fail locally, without an
    attempted fetch;
12. local path dependencies keep their documented precedence over same-named
    package dependencies.

Do not mock a network client or add one.  A test proving no network code path
is selected is sufficient.

### 2:25–2:50 — one end-to-end CLI proof and repair

Build one compact package project with a local registry, run `jisp lock`,
remove the registry, then run the locked package from its cache.  If any
earlier test reveals a bug, repair it in the owning resolver/CLI seam and add a
regression assertion at the level that exposed it.  Do not paper over a facade
bug in a CLI-only test.

### 2:50–3:10 — docs, full gate, handoff

Update `docs/PACKAGES.md` and `docs/TESTING.md` only for verified behaviours.
Run:

```text
cargo fmt --all -- --check
cargo test --workspace --exclude jisp-macros --quiet
cargo test -p jisp-macros --quiet
```

Commit one conventional patch.  Finish with the ledger, including any
deliberately untested remote-registry behaviour.

## Done when

- The 10–12 cases above have explicit proof or an evidence-backed documented
  exclusion.
- The end-to-end project runs from a verified cache after its local registry is
  unavailable.
- No failing path writes a trusted cache/lock artifact.
- Remote URL input remains a local rejection; no network capability is added.
- Documentation, tests, and resolver behaviour agree.

## Execution ledger

| Boundary | Proof |
| --- | --- |
| Stable sorted lock | a local two-package registry locks byte-identically twice and orders registry rows by name |
| Registry removal | the same project runs from its verified cache after the registry directory is deleted |
| Failed lock transaction | a frontend error restores the prior lock and removes new cache artifacts |
| Index validation | bad checksum, missing `source`, missing `checksum`, and manifest/index disagreement leave no lock/cache |
| Cache names | colliding sanitized package/version spelling produces distinct deterministic cache files |
| Cache integrity | existing facade tests cover cache-byte, lock-version, and manifest/lock checksum mismatches |
| Lock source trust | `..` source escapes are rejected before read/checksum use |
| Local/no-network policy | existing tests cover local-path precedence and both remote URL schemes as local rejections |

## Cut line

No semver solver, archive format, registry downloads, signatures, credentials,
or remote metadata lookup.  If a trust-boundary design change is needed rather
than a bounded bug fix, record it and stop that branch instead of guessing.

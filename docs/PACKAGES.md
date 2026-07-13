# Packages and registry policy

Jisp packages are deliberately minimal today. A package directory contains a
`jisp.toml` manifest, an entry source file, and optional local dependencies.

```toml
[package]
name = "app"
version = "0.1.0"
entry = "main.lisp"

[dependencies]
math = { path = "../math" }
```

`jisp run` without an explicit path reads `[package].entry` from the local
manifest. `jisp lock` resolves that entry, all transitive local path imports,
and any already-locked registry cache entries used by the import graph, then
writes the deterministic `jisp.lock` used by the current offline workflow.

## Manifest schema

Supported now:

- `[package].name`: package name for humans and future registry identity.
- `[package].version`: package version string. It is written by `jisp init` but
  not yet used for local path resolution.
- `[package].entry`: source file to run or lock when no explicit file is given.
- `[dependencies].name = "../path"`: shorthand local path dependency.
- `[dependencies].name = { path = "../path" }`: explicit local path dependency.

Supported only through an existing offline lock/cache entry:

```toml
[dependencies]
math = {
  registry = "jisp",
  package = "math",
  version = "1.2.3",
  checksum = "sha256:<hex-encoded digest>"
}
```

The parser recognizes `version`-based registry dependency specs, but resolution
does not fetch from the network. A registry dependency resolves only when
`jisp.lock` contains a matching `[registry.<name>]` entry with a local cached
source path and a SHA-256 checksum:

```toml
version = 1

[registry.math]
registry = "jisp"
package = "math"
version = "1.2.3"
source = "cache/math.lisp"
checksum = "sha256:<hex-encoded digest>"
```

The resolver requires lockfile `version` to match the manifest requirement,
reads `source`, verifies `checksum`, and then treats the cached file as the
imported module. Missing lock entries, version mismatches, and checksum
mismatches are hard errors. `registry` and `package` are recorded for
auditability and future index/fetch tooling; current resolution is intentionally
driven by the locked source and checksum.

`jisp lock` preserves and normalizes registry entries that are already present,
valid, and used by the import graph. It does not create a new registry entry
from the manifest alone, because no registry index or fetch/cache population
layer exists yet.

## Source and index decision

The planned registry model is source-first:

1. A registry index maps `(registry, package, version)` to immutable source
   archive metadata.
2. The lockfile records the selected version, source URL or index object ID,
   cached source path, and checksum.
3. Builds and tests consume only the lockfile plus a local cache.

The index is metadata, not executable code. The source archive is the authority
for package contents, and the checksum is the authority for cache integrity.

## Checksum policy

Registry dependencies must be content-addressed before they can be accepted by
the resolver:

- every locked registry package records a SHA-256 checksum;
- cache hits must verify the checksum before use;
- checksum mismatch is a hard error;
- no network fallback is allowed during ordinary `check`, `run`, `emit-rust`,
  `native-check`, or proc-macro expansion.

The local cache-hit and lockfile preservation path above exists now. Registry
index lookup, archive download, and cache population remain deferred.

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
manifest. `jisp lock` resolves that entry and all transitive local path imports,
then writes the deterministic `jisp.lock` used by the current offline workflow.

## Manifest schema

Supported now:

- `[package].name`: package name for humans and future registry identity.
- `[package].version`: package version string. It is written by `jisp init` but
  not yet used for local path resolution.
- `[package].entry`: source file to run or lock when no explicit file is given.
- `[dependencies].name = "../path"`: shorthand local path dependency.
- `[dependencies].name = { path = "../path" }`: explicit local path dependency.

Reserved for the future registry resolver:

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
fails offline with a clear unsupported-registry error. This keeps the manifest
shape testable without silently inventing fetch semantics.

## Source and index decision

The planned registry model is source-first:

1. A registry index maps `(registry, package, version)` to immutable source
   archive metadata.
2. The lockfile records the selected version, source URL or index object ID, and
   checksum.
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

Until the fetch/cache layer exists, registry specs remain intentionally
unsupported at resolution time.

---
slug: rdocx-opc-zip-features-break-wasm
severity: low
status: workaround-shipped
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-opc 0.1.2)
affects-versions:
  - rdocx-opc 0.1.2 (declares `zip = "8.1"` with default-features = true)
  - zip 8.x default features include `zstd` and `bzip2` (both C-backed)
discovered-on: 2026-05-20
project-impact: |
  Without the workaround, `cargo build --target wasm32-unknown-unknown` fails
  because zstd-sys and bzip2-sys try to compile C with a clang that doesn't
  have wasm32 support. With the workaround (a local [patch.crates-io] for
  rdocx-opc that disables zip's default features), the build succeeds, the
  WASM is smaller, and DOCX (which uses DEFLATE only) is unaffected.
workaround-in-this-repo: |
  approach-c-rdocx/converter/patches/rdocx-opc/Cargo.toml — overrides
  zip with default-features = false, features = ["deflate"].
repro-available: yes
repro-fixture: "n/a — Cargo build failure"
---

# rdocx-opc declares zip with default features, pulling C-backed zstd + bzip2 that don't cross-compile to wasm32-unknown-unknown

## Summary
`rdocx-opc 0.1.2`'s `Cargo.toml` declares:
```toml
[dependencies.zip]
version = "8.1"
```
The `zip` crate's default features include `zstd` and `bzip2`. Both depend on C libraries (`zstd-sys`, `bzip2-sys`) that don't cross-compile to `wasm32-unknown-unknown` without a wasi-sdk or similar. The build fails with:
```
error: unable to create target: 'No available targets are compatible with triple "wasm32-unknown-unknown"'
```
during `zstd-sys`'s `cc-rs`-invoked clang.

## Why this matters
- Cargo's "additive features" rule means downstream consumers cannot subtract `zstd` / `bzip2` from `zip` — once `rdocx-opc` enabled them, every consumer of `rdocx-opc` has them on.
- DOCX (and every other OOXML format) uses **DEFLATE** for its zip container. zstd and bzip2 are never relevant. The default-features inclusion is over-broad.

## Suggested upstream fix
Change `rdocx-opc`'s declaration to:
```toml
[dependencies.zip]
version = "8.1"
default-features = false
features = ["deflate"]
```
This removes the C deps, shrinks the build, and makes `rdocx-opc` cleanly cross-compilable to `wasm32-unknown-unknown` out of the box.

## Workaround we currently ship
`approach-c-rdocx/converter/patches/rdocx-opc/Cargo.toml` is a vendored copy of `rdocx-opc 0.1.2`'s manifest with the change above applied. Activated via:
```toml
[patch.crates-io]
rdocx-opc = { path = "patches/rdocx-opc" }
```
Has worked reliably across all opt-N variants.

## Repro artifacts in this repo
- `approach-c-rdocx/converter/patches/rdocx-opc/Cargo.toml` — the workaround
- The build error is reproducible by removing the `[patch.crates-io]` line and running `cargo build --target wasm32-unknown-unknown`.

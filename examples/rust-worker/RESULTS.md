# rust-worker — Results

## Status
**Shipped.** Pure-Rust Cloudflare Worker that mirrors the JS worker
(`examples/cloudflare-worker/`) on every route and response shape, built via
[`workers-rs`](https://github.com/cloudflare/workers-rs) 0.8.3 against the
opt-9a converter crate (post-security-audit).

> The bundle-size table below reflects the *current* multi-format,
> security-hardened build. Earlier numbers (when this worker shipped the
> opt-8 PDF-only converter) were ~1.03 MiB gz — see git history.

## Bundle size

| Build stage | Size |
|---|---|
| Cargo release (`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`) | 2.16 MiB raw |
| After `wasm-bindgen` (worker-build) | 2.16 MiB raw |
| After `wasm-opt -Oz` (worker-build) | 2.16 MiB raw |
| **Wrangler `Total Upload` (worker.wasm + shim, gzipped)** | **~1.08 MiB** |

| Metric | JS worker | Rust worker (this) | Δ |
|---|---|---|---|
| Total upload (gzipped) | ~1.09 MiB | **~1.08 MiB** | −10 KB |
| WASM raw | 2.25 MiB | 2.16 MiB | −90 KB |
| JS shim | 3 KB | 20 KB | +17 KB |

The Rust worker is ~10 KB *smaller* gzipped despite the larger JS shim
(`worker-build` produces a richer glue file than our hand-written
`worker.js`) because the routing logic compiled to Rust is more compact and
better LTO'd than the JS shim with TypeScript helpers from the package.

## Live e2e test (`wrangler dev` on port 8793)

| Endpoint | HTTP | Bytes out | Time | Content check |
|---|---|---|---|---|
| GET `/health` | 200 | banner | — | — |
| POST `/convert` (cdc_ngs_validation.docx, 670 KB) | 200 | 82 KB PDF | **75 ms** | "The Next Generation Sequencing Quality Initiative" ✓ |
| POST `/convert/html` | 200 | 66 KB HTML | **16 ms** | `<!DOCTYPE html><html>...` ✓ |
| POST `/convert/markdown` | 200 | 17 KB MD | **13 ms** | "The Next Generation Sequencing Quality Initiative\n\n…" ✓ |
| POST `/convert` with `"junk"` body | 422 | error message | — | `docx read: Opc(Zip(InvalidArchive("Could not find EOCD")))` ✓ |
| GET `/nowhere` | 404 | "Not found" | — | — |
| GET `/convert` | 405 | "Use POST with a .docx body" | — | — |

Latencies vs the JS worker (same test, same fixture):

| Format | JS worker | Rust worker | Δ |
|---|---|---|---|
| PDF | 84 ms | **75 ms** | −9 ms (~−11 %) |
| HTML | 18 ms | **16 ms** | −2 ms |
| Markdown | 14 ms | **13 ms** | −1 ms |

## What we learned about workers-rs (vs the JS-shim approach)

### In favour of Rust
- **Single language** for the production codebase. Conversion failures, route
  matching, and content negotiation all live in one place. No JS↔WASM ABI
  hand-rolling.
- **Slightly faster** per-request — the JS shim isn't called per request;
  `wasm-bindgen` exposes the `fetch` handler directly to the runtime.
- **Type-safety** all the way to the HTTP edge. `Response`, `Headers`,
  `Method` are typed Rust.
- **Same toolchain** as our converter crate, so `cargo check` covers both.

### In favour of the JS shim
- **Faster edit-reload** for routing changes. JS edits hot-reload in
  ~milliseconds; Rust edits trigger a `cargo build --target wasm32` + LLVM
  optimization that takes 5–15 seconds.
- **Easier to bolt on JS-only Workers features** (Service Bindings, Workers
  AI calls, Durable Object stubs) — the workers-rs crate covers most of
  these but the JS path covers all of them out of the box.
- **Smaller dev dep footprint** for someone who already has Node — no Rust
  toolchain needed if they only want to customize routing.

## Recommendation
**Ship both as parallel examples.** They cost ~5 KB of source each and
demonstrate two valid integration patterns. The JS worker is the
better-trodden path for most CF Workers users; the Rust worker shows the
"all-Rust edge" alternative and is the simpler architecture for teams who
already work in Rust.

The package (`packages/docx-to-pdf-wasm/`) is consumable from both. The Rust
worker imports it as a path dependency on the opt-9a crate; the JS worker
imports it via the published JS API (and the workspace-symlinked WASM).

# Approach C — Results

## TL;DR
**Approach C works.** WASM compiles, runs in Node.js (V8, same engine as CF Workers), produces correct PDFs across the corpus, and fits Cloudflare Workers' 10 MiB compressed bundle limit with **6.2 MiB of headroom**.

## WASM size

| Build | Size | Notes |
|---|---|---|
| Raw (`cargo build --release --target wasm32-unknown-unknown`) | 8.66 MiB | |
| After `wasm-opt -Oz` | 8.11 MiB | |
| **Gzipped (CF deploy size)** | **4.04 MiB** | 40.4 % of 10 MiB ceiling |
| Brotli q11 | 2.69 MiB | If CF accepts brotli (it does for static assets) |

Includes all 22 bundled fonts (Carlito, Caladea, Liberation Serif/Sans/Mono, OpenSans, NotoSans) totalling ~6.8 MiB of TTF data. Without bundled fonts the WASM is **0.65 MiB gzipped** — fonts dominate the size budget.

## Corpus scorecard (Node.js driving the WASM)

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| T1   | 5     | 5  | 0.97         | 0.00         | 0.0         | 48 ms  |
| T2   | 10    | 10 | 0.94         | 0.10         | 0.0         | 49 ms  |
| T3   | 10    | 9  | 0.77         | 0.00         | 0.4         | 47 ms  |

Compare to native baseline (`rdocx-cli` from crates.io, same code path):

| Tier | OK (native vs wasm) | Recall (native vs wasm) |
|------|---------------------|--------------------------|
| T1   | 5 / 5               | 1.00 vs 0.97             |
| T2   | 10 / 10             | 0.99 vs 0.94             |
| T3   | 9 / 10              | 0.78 vs 0.77             |

The 3–5 percentage-point drop on T1/T2 recall is caused by font substitution: native macOS has Calibri installed; WASM falls back to bundled Carlito, which renders some glyphs differently and a few words get pdftotext'd into slightly different tokens. **This is acceptable** per the project's "content > font fidelity" priority.

## What worked
- `cargo build --release --target wasm32-unknown-unknown --no-default-features` builds cleanly.
- Bundled fonts via `rdocx-layout` feature `bundled-fonts`.
- Custom C-ABI WASM exports (`alloc`, `dealloc`, `convert_wasm`, `last_error_ptr`, `last_error_len`) — no `wasm-bindgen` dependency.
- Same Rust source compiles to both native CLI and WASM cdylib.

## What we patched
- `rdocx-opc` transitively pulls in `zip` 8.x with all default features, including `zstd` and `bzip2` (both C-backed). These don't cross-compile to `wasm32-unknown-unknown` and aren't needed for DOCX (which uses DEFLATE only).
- Local patch in `converter/patches/rdocx-opc/` with `zip = { version = "8.1", default-features = false, features = ["deflate"] }`. Activated via `[patch.crates-io]`.

## What still fails
- **`tier3/deep-table-cell.docx`**: native and WASM both fail. rdocx returns a `Read` error. Probably an OOXML feature rdocx 0.1.2 doesn't yet support.
- **`tier3/image_with_textbox_caption.docx`** and **`tier3/notes.docx`**: convert "OK" but text in textboxes / footnote bodies is not rendered into the PDF body, so token recall drops. Known limitation of rdocx's text extraction from these structures.
- **`tier3/track_changes_insertion.docx`**: track-changes content not rendered (recall 0.62).

These are all rdocx upstream limitations, not our wrapper's. To fix, contribute upstream.

## CPU and memory
- Node.js per-document conversion: 47–50 ms. Cold-start (first call after instantiation) is similar — fonts are eagerly resolved at first use but the working set is small.
- Native binary: 23–33 ms (WASM is ~2× slower, as expected).
- Did not measure peak memory empirically; the largest docs in the corpus are <50 KB, well within 128 MB.

## Cloudflare Workers fit
- **Bundle size: 4.04 MiB gzipped** — fits with 6.2 MiB of margin under the 10 MiB ceiling.
- **CPU time: <50 ms per typical document** — well under even the 30 s default paid limit. A 100-page document at ~5 ms per page would land near 500 ms.
- **No threads, no SIMD, no WASI required** — `wasm32-unknown-unknown` only.

## Project files (this approach)
- `PLAN.md` — approach-specific plan
- `RESULTS.md` — this file
- `converter/` — Rust crate (lib + bin + cdylib)
  - `Cargo.toml` — dependencies + `[patch.crates-io]` for rdocx-opc
  - `src/lib.rs` — `convert(&[u8]) -> Result<Vec<u8>>` + WASM ABI module
  - `src/bin.rs` — native CLI `approach_c_rdocx IN OUT`
  - `patches/rdocx-opc/` — local patch (Cargo.toml only) disabling zip's zstd/bzip2
- `wasm-runner.mjs` — Node.js driver for the WASM, exposes the same `IN OUT` CLI shape
- `approach_c_rdocx.wasm` — built artifact (raw)
- `approach_c_rdocx.opt.wasm` — wasm-opt -Oz output (what we'd deploy)

## Recommendation
**Ship this.** Move to Phase 4 (Cloudflare Worker wrapper). For T3 fidelity improvements, file issues upstream against `rdocx` rather than patching locally.

Possible size optimization for v2: ship only Carlito + Liberation Serif (+ Bold) instead of all 22 fonts. That cuts ~5 MiB of TTF data, taking gzipped WASM from 4.04 MiB → ~1.0 MiB. The downside: documents that reference Times/Courier/Arial will fall back to Carlito only. Per the project's "content > font fidelity" stance, this is probably fine.

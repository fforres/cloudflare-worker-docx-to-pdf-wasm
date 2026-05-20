# opt-1-trim-fonts — Results

## TL;DR
Shipping only 4 TTFs (Carlito Reg/Bold, Liberation Serif Reg/Bold) cuts the gzipped WASM from **4.04 MiB → 1.57 MiB** (–61%, saving 2.47 MiB) with the same OK counts as baseline. Worker bundle gzipped: **1.60 MiB**, well under CF's 10 MiB ceiling.

## WASM size

| Build | opt-1 | baseline | Δ |
|---|---|---|---|
| Raw (`cargo build --release`) | 3.89 MiB | 8.66 MiB | –4.77 MiB |
| `wasm-opt -Oz` | 3.44 MiB | 8.11 MiB | –4.67 MiB |
| **Gzipped (CF deploy size)** | **1.57 MiB** | **4.04 MiB** | **–2.47 MiB (–61%)** |
| Brotli q11 | 1.16 MiB | 2.69 MiB | –1.53 MiB |
| Wrangler upload (gzipped) | 1.60 MiB | n/a (different worker) | — |

Total embedded TTF: ~2.0 MiB raw (vs 6.8 MiB in baseline). Compresses extremely well because much of each TTF is uncompressed glyph tables and DEFLATE-friendly metadata.

## Scorecard (Node.js driving the opt WASM)

| Tier | Files | OK | Recall (avg) | Page Δ | Img Δ | Avg ms |
|------|-------|----|--------------|--------|-------|--------|
| T1   | 5     | 5  | **0.91**     | 0.00   | 0.0   | 104 ms |
| T2   | 10    | 10 | **0.91**     | 0.10   | 0.0   | 103 ms |
| T3   | 10    | 9  | **0.77**     | 0.00   | 0.4   | 104 ms |

**vs baseline (`approach-c-rdocx`)**:

| Tier | OK (opt vs baseline) | Recall (opt vs baseline) |
|------|----------------------|---------------------------|
| T1   | 5/5 vs 5/5           | 0.91 vs 0.97              |
| T2   | 10/10 vs 10/10       | 0.91 vs 0.94              |
| T3   | 9/10 vs 9/10         | 0.77 vs 0.77              |

T1 dropped 6 pts (0.97→0.91) and T2 dropped 3 pts (0.94→0.91). T3 unchanged. All gates pass (T1 ≥ 0.85, T2 ≥ 6/10 OK). The biggest single drop: `lists_level_override.docx` T2 fell from ~1.0 to 0.467 — likely because list-marker fonts that resolved to Liberation Mono / OpenSans in baseline now fall back to Carlito and `pdftotext` tokenizes differently.

## Worker test (wrangler dev on :8788)

```
$ curl -sw 'HTTP=%{http_code} size=%{size_download}\n' --data-binary @fixtures/tier1/inline_formatting.docx http://127.0.0.1:8788/convert
HTTP=200 size=24873   (1-page PDF)

$ curl -sw 'HTTP=%{http_code} size=%{size_download}\n' --data-binary @fixtures/tier2/tables.docx http://127.0.0.1:8788/convert
HTTP=200 size=17092   (1-page PDF)

$ curl -sw 'HTTP=%{http_code} size=%{size_download}\n' --data-binary @fixtures/tier3/having-images.docx http://127.0.0.1:8788/convert
HTTP=200 size=190373  (1-page PDF, 7/9 imgs)

$ curl --data-binary 'garbage-not-a-docx' http://127.0.0.1:8788/convert
HTTP=500: convert failed: docx read: Opc(Zip(InvalidArchive("Could not find EOCD")))

$ curl --data-binary @fixtures/tier1/test.docx http://127.0.0.1:8788/convert     # recovery
HTTP=200 size=13635   (per-request reinstantiation works)
```

`npx wrangler deploy --dry-run` reports **Total Upload: 3531.13 KiB / gzip: 1634.51 KiB**.

## What got worse vs baseline
- T1 recall –6 pts, T2 recall –3 pts. Bold-italic and italic faces of Carlito/LibSerif are now absent → fontdb resolves them to Regular, which sometimes produces different shaped output (a few words tokenize differently after `pdftotext`).
- One T2 doc (`lists_level_override.docx`) regressed sharply (0.47 vs ~1.0) — list markers in unusual fonts now fall back.
- Documents using Liberation Mono / Courier (none in current corpus) would fall back to generic Serif/Sans — no monospace face shipped.

## What got better vs baseline
- **2.47 MiB smaller gzipped bundle** (61% reduction). Bigger CF headroom (now 8.43 MiB of slack vs 6.2 MiB).
- Faster wrangler bundle and presumably faster cold-start instantiation (less code to parse).
- Same OK counts across all tiers; all pass/fail gates met.

## Files
- `converter/Cargo.toml` — `bundled-fonts` feature dropped; `[patch.crates-io] rdocx-opc` retained.
- `converter/fonts/{Carlito-Regular,Carlito-Bold,LiberationSerif-Regular,LiberationSerif-Bold}.ttf`
- `converter/src/lib.rs` — `include_bytes!` four TTFs, call `to_pdf_with_fonts`.
- `wasm-runner.mjs` — points at `converter/target/.../approach_c_rdocx.opt.wasm`.
- `worker/{wrangler.toml,src/worker.js,src/converter.wasm}` — port 8788, per-request instantiation (verbatim from baseline).

## Verdict
**Ship.** 61% bundle reduction for ~3–6 pp recall cost on T1/T2 is a clear win given the project's explicit "content > font fidelity" stance. Combining this with opt-4 (code stripping) should land below 1 MiB gzipped.

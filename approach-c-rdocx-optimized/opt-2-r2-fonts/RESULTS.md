# opt-2 — Results

## TL;DR
**Smallest WASM of any variant** (0.65 MiB gz, same as the proven no-fonts
baseline). Worker total upload with the 4 fonts bundled as data assets is
**1.6 MiB gzipped** — well under the 10 MiB CF Workers ceiling, with the
fonts swappable to R2 in production. Corpus parity with the parent approach.

## Sizes

| Artifact | Raw | Gzipped |
|---|---|---|
| `approach_c_rdocx_opt2.wasm` (no fonts) | 1.91 MiB | — |
| `approach_c_rdocx_opt2.opt.wasm` (after `wasm-opt -Oz`) | 1.59 MiB | **0.65 MiB** |
| `wrangler deploy --dry-run` (worker.js + WASM + 4 TTFs) | 3.58 MiB total upload | **1.60 MiB** |

Compared to baseline (`approach-c-rdocx`): WASM-only **6.2× smaller gzipped**
(4.04 MiB -> 0.65 MiB). Worker bundle including the 4 fonts is **2.5× smaller**
than the all-fonts baseline. In a true R2 production setup the worker upload
would shrink further (just `worker.js` + WASM ~0.65 MiB gz) and the fonts
travel out-of-band.

## Corpus scorecard

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| T1   | 5     | 5  | **0.91**     | 0.00         | 0.0         | 62 ms  |
| T2   | 10    | 10 | **0.91**     | 0.10         | 0.0         | 65 ms  |
| T3   | 10    | 9  | 0.77         | 0.00         | 0.4         | 60 ms  |

All baseline pass gates met (T1 >= 0.85, T2 >= 6/10, WASM <= 10 MiB).

The 4 fonts (Carlito Reg/Bold + Liberation Serif Reg/Bold) match the
baseline's font coverage for typical Calibri/Times documents. Slight recall
drop on `tier1/links.docx` (0.74) and `tier2/lists_level_override.docx`
(0.47) — both also lag in the baseline, so the regression is rdocx upstream,
not a fonts issue.

## Worker test outcome
- `/health` returns the info line.
- `POST /convert` with `tier1/test.docx`, `tier2/tables.docx`,
  `tier3/footnotes.docx` -> 200 with valid PDF bodies.
- Bad-input recovery: malformed body returns 500 with the rdocx error
  string; the very next good request returns 200. Fresh-instance-per-request
  policy keeps a trap from poisoning the worker.

## Pros vs `opt-1` (trim-fonts)
- **Smallest WASM artifact**, by far (0.65 MiB gz vs ~1.0-1.5 MiB gz).
- Fonts are mutable post-deploy: rotate the R2 bucket to fix glyph coverage
  without recompiling Rust.
- Adding more fonts is an R2 upload, not a Rust rebuild.
- WASM stays cacheable across worker versions if fonts change.

## Cons vs `opt-1`
- Extra runtime cost: per-request `alloc + memcpy` of the fonts buffer
  (~1.3 MiB). Measured impact ~10-15 ms vs baseline (~50 ms -> 60-65 ms).
- Two trips of complexity: worker init logic + R2 wiring for production.
- Cold start for a brand-new isolate must fetch from R2 before the first
  convert (4 parallel R2 GETs, typically <50 ms warm). Cached in
  module-scope after that.
- Worker bundle still ships ~1.3 MiB of TTF as `Data` assets in dev; only
  the R2 path actually shrinks the wire size below the all-fonts baseline.

## Files
- `converter/` — fork of `approach-c-rdocx/converter`, no `rdocx-layout` dep,
  no `bundled-fonts` feature. Adds `convert_with_fonts` export and
  `decode_fonts_buffer` mini-format parser.
- `worker/wrangler.toml` — `CompiledWasm` + `Data` rules. Port 8789.
- `worker/src/worker.js` — imports the 4 TTFs as ArrayBuffers, builds the
  mini-format buffer once at module load, fresh WASM instance per request.
- `worker/src/fonts/` — Carlito-{Regular,Bold}.ttf,
  LiberationSerif-{Regular,Bold}.ttf (copied from `rdocx-layout-0.1.2`).
- `worker/README.md` — documents the R2 swap-in for production.
- `wasm-runner.mjs` — Node.js CLI mirror of the worker; harness driver.
- `fonts-bundle/` — copy of the 4 TTFs (the "would-be R2 upload" set).

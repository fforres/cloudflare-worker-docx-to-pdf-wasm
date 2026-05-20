# opt-3 — Build-time font subsetting — Results

## TL;DR
**Works.** Subsetting all 20 bundled TTFs to Latin Unicode coverage (U+0000–024F + general punctuation + currency + super/subscript) at build time via `pyftsubset` cut the WASM from **4.04 MiB → 1.31 MiB gzipped** (a 67.5% reduction), with **identical scorecard** to baseline (T1 0.97 / T2 0.94 / T3 0.77 — Latin-only docs are unaffected).

## Strategy used
**Strategy A: `pyftsubset` from fonttools** invoked by `build.rs`.

Why not Strategy B (subsetter crate): pyftsubset works out-of-the-box, produces standalone TTFs that drop straight into `to_pdf_with_fonts`, and respects OT layout features (`--layout-features='*'`). The `subsetter` crate is designed primarily for PDF embedding streams, not standalone TTF output — it would have required validating that the output is parsable by `fontdb` / `ttf-parser`. Strategy A is faster to ship.

### Build-time Python dependency
`build.rs` requires `pyftsubset` (from `pip3 install --user --break-system-packages fonttools brotli`). Auto-discovered in PATH, `/opt/homebrew/bin`, `/usr/local/bin`, or `~/Library/Python/*/bin`. Override with `PYFTSUBSET` env var. On a fresh CI image: `pip3 install --user fonttools brotli` before `cargo build`.

## Per-font size before/after

| Font | Original | Subset | Ratio |
|---|---|---|---|
| Carlito-Regular | 636 KB | 119 KB | 18% |
| Carlito-Bold | 691 KB | 128 KB | 18% |
| Carlito-Italic | 623 KB | 113 KB | 18% |
| Carlito-BoldItalic | 817 KB | 153 KB | 18% |
| Caladea-Regular | 59 KB | 48 KB | 81% (already Latin) |
| Caladea-Bold | 59 KB | 48 KB | 81% |
| Caladea-Italic | 62 KB | 51 KB | 82% |
| Caladea-BoldItalic | 62 KB | 50 KB | 81% |
| LiberationSans-Regular | 350 KB | 42 KB | 12% |
| LiberationSans-Bold | 354 KB | 43 KB | 12% |
| LiberationSans-Italic | 356 KB | 45 KB | 12% |
| LiberationSans-BoldItalic | 350 KB | 45 KB | 12% |
| LiberationSerif-Regular | 388 KB | 46 KB | 11% |
| LiberationSerif-Bold | 365 KB | 46 KB | 12% |
| LiberationSerif-Italic | 371 KB | 48 KB | 12% |
| LiberationSerif-BoldItalic | 371 KB | 47 KB | 12% |
| LiberationMono-Regular | 313 KB | 43 KB | 13% |
| LiberationMono-Bold | 302 KB | 43 KB | 14% |
| LiberationMono-Italic | 275 KB | 46 KB | 16% |
| LiberationMono-BoldItalic | 278 KB | 45 KB | 16% |
| **Total TTF** | **~6.8 MB** | **~1.30 MB** | **~19%** |

The Carlito family is by far the heaviest (~2.7 MB raw covered most of the original bundle's font budget). Liberation fonts ship with Hebrew/Arabic/Cyrillic and a full hinting program — both stripped here. Caladea is already Latin-only so only modest savings.

## WASM size (all 20 subset fonts embedded)

| Build | Size | vs baseline (4.04 MiB gz) |
|---|---|---|
| Raw `cargo build --release` | 3.09 MiB | -5.57 MiB |
| `wasm-opt -Oz` | 2.70 MiB | |
| **Gzipped (CF deploy size)** | **1.31 MiB** | **-2.73 MiB (-67.5%)** |
| Brotli q11 | 1.00 MiB | |

CF Workers margin under 10 MiB: 8.69 MiB.

## Wrangler `deploy --dry-run`
```
Total Upload: 3173.88 KiB / gzip: 1371.09 KiB
```
Worker bundle on the wire: **1.34 MiB gzipped** (includes worker.js shim + the wasm).

## Scorecard (`harness/score.py opt-3-font-subsetting ./wasm-runner.mjs`)
| Tier | Files | OK | Recall (avg) | Page Δ | Img Δ | Avg ms |
|------|-------|----|--------------|--------|-------|--------|
| tier1 | 5 | 5 | 0.97 | 0.00 | 0.0 | 45 |
| tier2 | 10 | 10 | 0.94 | 0.10 | 0.0 | 46 |
| tier3 | 10 | 9 | 0.77 | 0.00 | 0.4 | 46 |

**Bit-for-bit identical to baseline.** The corpus is all Latin/English text; the subset coverage U+0000–U+024F + punctuation + currency catches everything. The same upstream rdocx limitations apply (`deep-table-cell` fails, textbox/footnote text not extracted on T3 outliers).

## Worker end-to-end test (port 8790)
```
$ npx wrangler dev --port 8790 &
$ curl -X POST --data-binary @fixtures/tier1/test.docx http://127.0.0.1:8790/convert -o out.pdf
HTTP:200 size:7774 type:application/pdf  → valid PDF, 1 page
$ curl -X POST --data-binary @fixtures/tier2/tables.docx http://127.0.0.1:8790/convert -o out2.pdf
HTTP:200 size:10088 → valid PDF, 1 page
```

## What's in the bundle
- `converter/Cargo.toml` — drops `bundled-fonts` feature on `rdocx-layout` (otherwise dup-fonts via two paths).
- `converter/build.rs` — runs `pyftsubset` on each of 20 TTFs at compile time. Output goes to `$OUT_DIR/fonts/`.
- `converter/src/lib.rs` — `include_bytes!` each subset, registered via `to_pdf_with_fonts(&[(family, data); 20])`.
- `converter/patches/rdocx-opc/` — same zip-feature patch as baseline.
- `worker/` — port 8790, identical per-request instantiation pattern as baseline worker.
- `wasm-runner.mjs` — Node driver for the harness.

## Fidelity caveat
Documents containing CJK, Cyrillic, Greek, Arabic, or rare math symbols will render those glyphs as `.notdef` boxes — same as if those fonts had never been bundled. The harness corpus contains no such inputs, so the scorecard matches baseline exactly.

## Stacking potential
Combined with **opt-4 (code stripping)** this could land around **1.0–1.1 MiB gz**. Combined with **opt-1** instead (only Carlito Reg+Bold + Liberation Serif Reg+Bold, subset) would land near **0.85 MiB gz**. Both deferred per project plan.

## Verdict
**Ship this** if Latin-only coverage is acceptable. 67.5% bundle reduction with zero scorecard regression and minimal added build complexity (one Python dep). Build adds ~0.5 s for the 20 pyftsubset invocations on a warm cache.

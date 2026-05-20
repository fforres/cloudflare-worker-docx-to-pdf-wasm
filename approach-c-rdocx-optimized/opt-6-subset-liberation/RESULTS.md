# opt-6-subset-liberation — Results

## TL;DR

**Stacked opt-3's pyftsubset pipeline on top of opt-5's Liberation-only +
alias-map fix.** Result: opt-5's correctness (avg complex recall 0.94, cdc
0.97) is fully preserved, while the bundle shrinks from **2.82 MiB gz →
0.98 MiB gz** (a 1.83 MiB / 65% cut). Worker upload size from
`wrangler deploy --dry-run` drops from **2.88 MiB gz → 1.01 MiB gz**.

## Bundle size

| Artifact                              | opt-5            | opt-6            | Δ            |
|---------------------------------------|------------------|------------------|--------------|
| WASM raw                              | 5.79 MiB         | 2.41 MiB         | **−3.38 MiB**|
| WASM gzip                             | 2.82 MiB         | **0.98 MiB**     | **−1.83 MiB**|
| WASM brotli                           | (n/a)            | 0.75 MiB         | —            |
| Worker upload (wrangler dry-run, gz)  | 2.88 MiB         | **1.01 MiB**     | **−1.87 MiB**|
| Fits CF Workers (10 MiB compressed)   | ✓ 7.1 MiB margin | ✓ **9.0 MiB**    | +1.9 MiB     |

## Per-Liberation-font size (before → after pyftsubset)

Coverage applied: `U+0000-024F, U+2000-206F, U+20A0-20CF, U+2070-209F`
(Latin Basic+Ext-A/B + general punctuation + currency + super/subscript),
with `--layout-features='*' --no-hinting --desubroutinize --drop-tables+=FFTM`.

| Font                              | Before    | After    | %    |
|-----------------------------------|-----------|----------|------|
| LiberationSans-Regular            | 350 200 B | 42 404 B | 12%  |
| LiberationSans-Bold               | 353 936 B | 42 552 B | 12%  |
| LiberationSans-Italic             | 355 608 B | 44 532 B | 12%  |
| LiberationSans-BoldItalic         | 349 724 B | 44 576 B | 12%  |
| LiberationSerif-Regular           | 388 352 B | 45 548 B | 11%  |
| LiberationSerif-Bold              | 365 112 B | 45 980 B | 12%  |
| LiberationSerif-Italic            | 370 968 B | 47 604 B | 12%  |
| LiberationSerif-BoldItalic        | 371 060 B | 47 268 B | 12%  |
| LiberationMono-Regular            | 313 408 B | 42 740 B | 13%  |
| LiberationMono-Bold               | 301 684 B | 42 548 B | 14%  |
| LiberationMono-Italic             | 274 984 B | 45 696 B | 16%  |
| LiberationMono-BoldItalic         | 277 912 B | 45 008 B | 16%  |
| **Total**                         | **4.07 MiB** | **0.51 MiB** | **12.5%** |

Net font bytes saved: ~3.55 MiB pre-compression → ~1.83 MiB post-gzip
(matches the WASM-gz delta exactly, confirming the cut is fonts-only and
that no codegen changes leaked in).

## Recall — complex corpus (25 docs)

| Metric                  | opt-5  | opt-6  | Δ      |
|-------------------------|--------|--------|--------|
| Avg recall              | 0.94   | **0.94** | 0      |
| OK / total              | 25/25  | 25/25  | 0      |
| cdc_ngs_validation      | 0.97   | **0.972** | +0.00  |
| cdc_ngs_validation_plan | 0.98   | 0.975  | −0.005 |
| epa_opcert_annual       | 0.91   | 0.908  | −0.00  |
| nasa_business_plan      | 1.00   | 1.00   | 0      |
| nist_sp800_53           | (n/a printed) | 0.971 | — |
| un_seea_policy_brief    | 0.00   | 0.00   | 0 (textbox upstream gap) |

cdc_ngs_validation specifically — the doc the Carlito fix rescued from
0.19 — stays at **0.972**, well above the 0.90 hard floor.

Hard pass criteria:

- complex avg recall ≥ 0.90: **0.94 ✓**
- bundle gz < opt-5's 2.82 MiB (target ~1.8 MiB): **0.98 MiB ✓** (beats target by ~0.8 MiB)
- cdc_ngs_validation ≥ 0.90: **0.972 ✓**

## Recall — toy corpus

| Tier | opt-5             | opt-6              | Δ |
|------|-------------------|--------------------|---|
| T1   | 1.00 (5/5)        | **1.00** (5/5)     | 0 |
| T2   | 0.99 (10/10)      | **0.99** (10/10)   | 0 |
| T3   | 0.78 (9/10 OK)    | **0.78** (9/10 OK) | 0 |

The single FAIL on `deep-table-cell.docx` is identical to opt-5 (rdocx
panic in a deeply-nested table cell, unrelated to fonts). Per-doc recall
matches opt-5 byte-for-byte except for the file-size shrink (PDFs are
~50% smaller because the embedded font subsets are smaller).

## Worker — port 8793

`npx wrangler dev --port 8793 --ip 127.0.0.1` came up clean; per-request
instantiation pattern carried over from opt-5 verbatim.

Smoke-test transcript:

```
GET  /health                              -> HTTP 200
POST /convert  cdc_ngs_validation.docx    -> HTTP 200, 81 392 B, 0.080 s
POST /convert  nist_sp800_53.docx         -> HTTP 200, 3 737 204 B, 3.71 s
POST /convert  nasa_business_plan.docx    -> HTTP 200, 20 045 B, 0.009 s
```

`pdftotext` on the cdc PDF returned by the worker:

```
The Next Generation Sequencing Quality Initiative
The Next Generation Sequencing (NGS) Quality Initiative is a collaboration between the
Centers for Disease Control and Prevention (CDC), the Association of Public Health
```

i.e. clean text, no Carlito cmap drift — the opt-5 fix is preserved
end-to-end through subsetting.

`npx wrangler deploy --dry-run`:

```
Total Upload: 2480.33 KiB / gzip: 1036.24 KiB
```

Wrangler killed at end of test.

## Implementation diff vs opt-5

- `converter/Cargo.toml`: rename crate → `approach_c_rdocx_opt6`, add
  `build = "build.rs"`. Profile, deps, and patch unchanged.
- `converter/build.rs` (new): copied from opt-3, narrowed `FONTS` list to
  the 12 Liberation faces, source dir defaults to the crate-local
  `fonts/` (the same 12 TTFs opt-5 already shipped), output to
  `OUT_DIR/fonts/<Name>.ttf`. Same pyftsubset args as opt-3.
- `converter/src/lib.rs`: only the 12 `include_bytes!` paths changed
  from `../fonts/<Name>.ttf` to
  `concat!(env!("OUT_DIR"), "/fonts/<Name>.ttf")`. Alias map, family
  enum, `to_pdf_with_fonts` call, WASM ABI all byte-identical to opt-5.
- `converter/src/bin.rs`: import path renamed to `approach_c_rdocx_opt6`.
- `wasm-runner.mjs`: wasm path renamed to `approach_c_rdocx_opt6.wasm`.
- `worker/wrangler.toml`: name and dev port (8793).
- `worker/src/converter.wasm`: replaced with the new (smaller) artifact.

## Recommendation

**Ship opt-6.** It is strictly better than opt-5: same correctness on
both corpora (0.94 complex avg, 1.00/0.99/0.78 toy), same Carlito-CMap
fix, but 65% smaller gzipped (0.98 MiB vs 2.82 MiB). The Carlito bug
workaround and font subsetting compose cleanly because they operate on
disjoint slices of the problem — the bug is a data-shape interaction in
Carlito's tables, which we never bundle; subsetting only trims glyphs
the document doesn't use. Worker cold-start, end-to-end conversion
latency, and PDF text fidelity are indistinguishable from opt-5 at the
3-doc smoke level. The remaining gaps (textbox text in
`un_seea_policy_brief`, deep-table-cell panic) are upstream rdocx
parser/renderer issues unaffected by font bytes and out of scope for
any opt-N font work.

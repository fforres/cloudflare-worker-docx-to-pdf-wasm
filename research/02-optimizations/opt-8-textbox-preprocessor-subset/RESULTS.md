# opt-8 — Results

## TL;DR
**Best variant shipped.** Stacks three independent learnings:
- opt-5's Liberation alias map (fixes the Carlito ToUnicode CMap bug → +0.23 complex-corpus recall)
- opt-6's pyftsubset build pipeline (–1.8 MiB gz)
- opt-7's textbox preprocessor (rescues 1 doc from 0.00 → 0.93 recall, smaller wins on 3 others)

Combined: **1.03 MiB gz**, **0.98 complex-corpus recall**, **0.89 T3 toy recall**, zero scorecard regressions vs opt-6.

## Bundle size

| Build | Size | Notes |
|---|---|---|
| Raw | 2.57 MiB | |
| wasm-opt -Oz | 2.23 MiB | |
| **gzip -9** | **1.03 MiB** | what the worker ships |
| brotli q11 | 0.79 MiB | |

Compared to opt-6's 0.98 MiB gz: **+50 KB** (the preprocessor + `zip` + `quick-xml` overhead). Compared to opt-5's 2.82 MiB gz: **–63 %**. Compared to the original C-baseline's 4.04 MiB: **–75 %**.

## Complex corpus (25 real-world docs)

| Variant | Avg recall | un_seea_policy_brief |
|---|---|---|
| opt-3 (cmap-drift bug, unfixed) | 0.71 | 0.00 |
| opt-5 (Carlito fix) | 0.94 | 0.00 |
| opt-6 (+ subset) | 0.94 | 0.00 |
| **opt-8 (+ textbox preprocessor)** | **0.98** | **0.93** |

All 25 docs OK. Highlights:
- `un_seea_policy_brief.docx` — 0.000 → **0.929** (textbox lift)
- `cdc_ngs_validation.docx` — 0.97 → **0.98**
- `cdc_ngs_validation_plan.docx` — 0.97 → **0.98**
- 22 other docs: unchanged (preprocessor was a no-op)

## Toy corpus regression check

| Tier | opt-6 | **opt-8** |
|---|---|---|
| T1 | 1.00 | **1.00** |
| T2 | 0.99 | **0.99** |
| T3 | 0.78 | **0.89** |

T3 jump from 0.78 → 0.89 is the textbox preprocessor catching `textbox_image.docx` and similar fixtures that exercise the same `<w:txbxContent>` path. No regressions anywhere.

## Worker end-to-end test (port 8787, top-level worker)

| Doc | HTTP | Out KB | Time | First line of pdftotext |
|---|---|---|---|---|
| un_seea_policy_brief.docx (textbox) | 200 | 12 | 40 ms | "Executive Summary / TITLE – Short Policy Brief" |
| cdc_ngs_validation.docx (Carlito) | 200 | 80 | 58 ms | "The Next Generation Sequencing Quality Initiative" |
| nist_sp800_53.docx (468 pages) | 200 | 3675 | 3.68 s | (correct security-controls preamble) |

The Carlito fix from opt-5 is intact; the textbox lift from opt-7 is working in WASM. Worker stays healthy across mixed good/bad inputs (per-request instantiation guards traps).

## Recommendation
**Shipped as the top-level worker** (`worker/src/converter.wasm`). Strictly better than every previous variant on every metric we score. The +50 KB gz over opt-6 buys a +0.04 absolute recall gain on the complex corpus and +0.11 on T3 toy — best $/byte spent in the project.

## Files
- `converter/Cargo.toml` — adds `zip` (deflate-flate2) and `quick-xml` deps on top of opt-6
- `converter/src/lib.rs` — opt-6 + `mod preprocess;` + one `preprocess::preprocess_textboxes(...)` call before `Document::from_bytes`
- `converter/src/preprocess.rs` — vendored from opt-7 unchanged (~480 lines, defensive byte-scanner — no XML tree builds; returns original bytes on any error)
- `converter/build.rs` — vendored from opt-6 unchanged
- `wasm-runner.mjs` — Node driver for the harness
- `../../worker/src/converter.wasm` — this variant's `wasm-opt -Oz` output, currently live

# Research notebook

This folder documents how we got from "is converting DOCX to PDF inside a Cloudflare Worker even possible?" to a shippable 1 MiB-gzipped WASM that handles real-world documents at ~50 ms per typical file.

It's a working log, not a tutorial. Numbers, dead ends, and the bug we found upstream are all here. The README at each level summarizes what happened; the `RESULTS.md` files inside each variant contain the raw measurements.

## The story in one paragraph

We evaluated three different Rust stacks in parallel (custom layout from scratch, Typst-based, and the `rdocx` library), picked `rdocx` for its size + speed, optimized its bundle four ways (font subsetting, R2-hosted fonts, font trimming, code stripping), tested all of that against 25 real-world public DOCX files, discovered a real bug in `rdocx-pdf` that scrambled text on ~32% of those documents, fixed it by bypassing the buggy code path, then layered a DOCX-XML preprocessor to recover content from sidebar/textbox layouts. The final build (opt-8) is what `packages/docx-to-pdf-wasm` ships.

## Folder tour

| Folder | What's in it |
|---|---|
| [`01-approaches/`](01-approaches/) | Approach A (`docx-rs` + `krilla`, hand-rolled layout), B (`docx-rs` → Typst → PDF), C (the `rdocx` library). Built in parallel, scored side-by-side against the toy corpus. |
| [`02-optimizations/`](02-optimizations/) | Optimizations on top of approach C. opt-1 through opt-8. opt-3 was the first "good enough" size win; opt-8 is what we ship. |
| [`03-real-world-testing/`](03-real-world-testing/) | Stress-test against 25 real-world public DOCX files (NIST, NASA, EPA, CDC, UN, WHO, university theses). This is where the upstream `rdocx-pdf` Carlito bug showed up and forced the opt-5 → opt-8 redesign. |
| [`04-found-issues/`](04-found-issues/) | Outstanding upstream issues we noticed but didn't (or couldn't) fix in this project. Written as if ready to file — pick whichever you want to push upstream. |
| [`fixtures/`](fixtures/) | The DOCX test corpus, organised by tier (T1 simple, T2 business, T3 complex, plus a `complex/` tier with the 25 real-world files). |
| [`reference-pdfs/`](reference-pdfs/) | LibreOffice-headless-generated reference PDFs. Treated as ground truth. |
| [`harness/`](harness/) | Python scorer (`score.py`, `score_complex.py`) — runs the converter under test against the corpus and writes per-doc + aggregate scorecards. |
| [`results/`](results/) | Per-variant scorecards. One folder per `<approach-or-opt-name>`. |
| [`PLAN.md`](PLAN.md), [`TEST_PLAN.md`](TEST_PLAN.md) | Original top-level planning docs. The pass/fail gates and the test methodology that everything else references. |

## Reading order

If you're skimming:
1. [`PLAN.md`](PLAN.md) — what we set out to do
2. [`01-approaches/`](01-approaches/) — what we tried
3. [`02-optimizations/RESULTS.md`](02-optimizations/RESULTS.md) — opt-1…opt-4 scorecard
4. [`03-real-world-testing/COMPARISON.md`](03-real-world-testing/COMPARISON.md) — the plot twist when real docs revealed a different bug
5. [`02-optimizations/opt-8-textbox-preprocessor-subset/RESULTS.md`](02-optimizations/opt-8-textbox-preprocessor-subset/RESULTS.md) — the final build
6. [`04-found-issues/`](04-found-issues/) — what's still broken upstream

If you're going deep, read everything bottom-up in chronological order:
- [`01-approaches/approach-a-custom/RESULTS.md`](01-approaches/approach-a-custom/RESULTS.md)
- [`01-approaches/approach-b-typst/RESULTS.md`](01-approaches/approach-b-typst/RESULTS.md)
- [`01-approaches/approach-c-rdocx/RESULTS.md`](01-approaches/approach-c-rdocx/RESULTS.md)
- [`results/comparison.md`](results/comparison.md) — first cross-approach synthesis (toy corpus only)
- [`02-optimizations/opt-1-trim-fonts/RESULTS.md`](02-optimizations/opt-1-trim-fonts/RESULTS.md) through `opt-4` — size opts
- [`02-optimizations/RESULTS.md`](02-optimizations/RESULTS.md) — first opt scorecard
- [`03-real-world-testing/RESULTS.md`](03-real-world-testing/RESULTS.md) — the stress test that broke our model
- [`03-real-world-testing/COMPARISON.md`](03-real-world-testing/COMPARISON.md) — comparison across A, B, C-baseline, opt-3 on real docs
- [`02-optimizations/opt-5-complex-corpus/RESULTS.md`](02-optimizations/opt-5-complex-corpus/RESULTS.md) — Carlito bug isolated + fix
- [`02-optimizations/opt-6-subset-liberation/RESULTS.md`](02-optimizations/opt-6-subset-liberation/RESULTS.md) — fix + size opt stacked
- [`02-optimizations/opt-7-textbox-preprocessor/RESULTS.md`](02-optimizations/opt-7-textbox-preprocessor/RESULTS.md) — textbox content recovery
- [`02-optimizations/opt-8-textbox-preprocessor-subset/RESULTS.md`](02-optimizations/opt-8-textbox-preprocessor-subset/RESULTS.md) — final, what we ship

## Headline metrics across the project

| Variant | Bundle (gz) | Toy T1/T2/T3 recall | Real-world recall | Conversion latency |
|---|---|---|---|---|
| Approach A (native) | 0.89 MiB | 1.00 / 0.79 / 0.62 | 0.82 | ~50 ms |
| Approach B (Typst, native — doesn't fit Workers) | 14.06 MiB ✗ | 1.00 / 0.99 / 0.91 | **0.98** | ~480 ms |
| Approach C — baseline WASM | 4.04 MiB | 0.97 / 0.94 / 0.77 | 0.71 (broken on real docs) | ~50 ms |
| opt-3 — font-subsetting | 1.31 MiB | 0.97 / 0.94 / 0.77 | 0.71 (same upstream bug) | ~50 ms |
| opt-5 — Carlito fix | 2.82 MiB | 1.00 / 0.99 / 0.78 | 0.94 | ~50 ms |
| opt-6 — Carlito fix + subset | 0.98 MiB | 1.00 / 0.99 / 0.78 | 0.94 | ~50 ms |
| **opt-8 — opt-6 + textbox preprocessor** | **1.03 MiB** | **1.00 / 0.99 / 0.89** | **0.98** | ~50 ms |

Reference: LibreOffice headless PDF = 1.00 recall (ground truth).

## How to reproduce the scorecards

```bash
# From the repo root:
python3 research/harness/score.py            <name>  <cli-or-runner-path>
python3 research/harness/score_complex.py    <name>  <cli-or-runner-path>
# Outputs scorecards under research/results/<name>/.
```

Each variant's folder has a `wasm-runner.mjs` (Node driver) and/or a native CLI binary in `target/release/`. The harness invokes either via the same `cli INPUT.docx OUTPUT.pdf` shape.

## Tools used

- **Rust 1.95+** with `wasm32-unknown-unknown` target
- **LibreOffice headless** (`soffice --convert-to pdf`) for ground-truth references
- **fontTools / pyftsubset** for build-time Latin font subsetting
- **`wasm-opt`** (binaryen) for `-Oz` size optimization
- **`pdftotext`, `pdfinfo`, `pdfimages`** (poppler) for scorecard metrics
- **Python 3** for the scoring harness

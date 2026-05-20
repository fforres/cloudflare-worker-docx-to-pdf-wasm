# Complex corpus — A / B / C-baseline / opt-3 head-to-head

Same 25 real-world DOCX files. Reference PDFs from LibreOffice headless. Scoring via `harness/score_complex.py` (text token recall, page-count delta, image count, conversion time).

## Headline scorecard

| Approach | OK | **Avg recall** | Avg page Δ | Avg ms | Bundle size (gz) | Fits CF Workers? |
|---|---|---|---|---|---|---|
| A — custom (docx-rs + krilla, native) | 25/25 | 0.82 | 0.46 | ~50 | 0.89 MiB | ✓ |
| **B — Typst (native)** | 25/25 | **0.98** | 0.23 | 694 | 14.06 MiB | ✗ |
| C-baseline native (22 full fonts, native) | 25/25 | **0.94** | 0.13 | 221 | n/a | n/a |
| C-baseline WASM (22 full fonts) | 25/25 | **0.71** | 0.13 | 405 | 4.04 MiB | ✓ |
| opt-3 WASM (20 subset fonts) | 25/25 | **0.71** | 0.13 | 337 | 1.31 MiB | ✓ |

## The big finding

**Native rdocx works (0.94 recall). WASM rdocx is broken (0.71 recall).** Same code, same fonts, same documents — the only variable is the compile target.

Subsetting is **not** the bug. Full-font C-baseline WASM produces byte-identical cmap-drift output to opt-3-subset WASM. Both fail in the same way, on the same docs, with the same scrambled text. Bundling 6.8 MB of full TTFs in the WASM gets you **zero** fidelity improvement.

The native build of the **same crate, same rdocx version, same code** renders the same docs at 0.94 avg. **This is a wasm32-unknown-unknown codegen issue in rdocx-layout or rdocx-pdf.**

## What this means in plain terms

The PDFs we generate from real-world DOCX look visually plausible but have **scrambled text layers**. Example from cdc_ngs_validation.docx:
- Reference (LibreOffice): *"The Next Generation Sequencing utility is a collaboration between..."*
- Approach B (Typst, native): identical to reference
- C-baseline native (rdocx): identical to reference
- **C-baseline WASM (rdocx, what ships)**: *"The Next eeeeatioe Sequeecieg uutiit neiitiie i t coiitloatioe letbeee..."*

`pdftotext`, full-text search, screen readers, and any AI content-extraction pipeline downstream will see gibberish on ~32% of real documents.

## Cmap-drift docs — head-to-head

Every doc where opt-3 had bad recall:

| Doc | LibreOffice (ref) | A | B | C-native | C-WASM | opt-3 WASM |
|---|---|---|---|---|---|---|
| cdc_ngs_validation | 1.00 | 0.66 | **0.97** | **0.97** | 0.19 | 0.19 |
| cdc_ngs_validation_plan | 1.00 | 0.72 | **0.97** | **0.98** | 0.21 | 0.21 |
| epa_site_inspection | 1.00 | 0.93 | **1.00** | **1.00** | 0.27 | 0.27 |
| epa_inception_workplan | 1.00 | 0.88 | **0.99** | **0.99** | 0.30 | 0.30 |
| epa_opcert_annual | 1.00 | 0.47 | **0.94** | **0.91** | 0.49 | 0.49 |
| un_ungegn_strategic | 1.00 | 0.76 | **1.00** | **0.97** | 0.18 | 0.18 |
| nasa_business_plan | 1.00 | 0.51 | **0.99** | **0.97** | 0.53 | 0.53 |
| cdc_hepatitis_eval | 1.00 | 0.46 | **0.99** | **0.99** | 0.55 | 0.55 |
| nist_ucd_report | 1.00 | 0.60 | **1.00** | **1.00** | 0.79 | 0.79 |
| **un_seea_policy_brief** (textbox) | 1.00 | 0.00 | **0.89** | ? | 0.00 | 0.00 |

C-WASM and opt-3-WASM track **byte-identically**. The cmap drift is the WASM build, not the fonts.

## What each approach does well — extractable learnings

| Approach | What it does *better* than C | Can we port that to C? |
|---|---|---|
| **B (Typst)** | Correct ToUnicode CMap writing on cmap-drift docs. Textbox content extracted. | **Yes — by replacing rdocx-pdf with Typst as the renderer.** But this means going back to approach B's stack (and 14 MiB bundle). Or extracting *just* Typst's PDF writer (typst-pdf is a separable crate). |
| **A (custom, krilla)** | Correct ToUnicode CMap writing in WASM. Smaller binary. No crashes on the complex corpus. | **Yes — by replacing rdocx-pdf with krilla.** krilla is pure-Rust, lightweight, used by A already. Combine with rdocx for parsing/layout. |
| **C native** | Already correct; we just can't ship native to Workers. | **No** — Workers is the target. |

A and B have the right behavior in WASM because they use different PDF writers (krilla and typst-pdf respectively). rdocx-pdf has a bug that only manifests in `wasm32-unknown-unknown`.

## The opt-5 plan

**Name**: `approach-c-rdocx-optimized-for-complex-corpus`
**Location**: `approach-c-rdocx-optimized/opt-5-complex-corpus/`
**Strategy**: Keep rdocx-opc + rdocx-oxml + rdocx-layout (the parsing + layout pipeline, which the C-native data shows works correctly). **Replace rdocx-pdf with a custom PDF emitter** using `krilla` or `pdf-writer` directly — the same writers that A and B use without the bug.

This is *the* learning to extract: **the bug is in rdocx-pdf's WASM glyph-emission code**, and we can sidestep it by writing PDF ourselves from rdocx-layout's output.

### Two implementation paths to try, in order

1. **Path A (low-risk)**: Try compile-flag variations first (`opt-level=s`, `lto=false`, `panic=abort`, different `wasm-opt` flags). If any of these change the output, we've narrowed it to LTO/codegen and may be able to fix with a flag. ~30 min budget.

2. **Path B (real fix)**: Replace rdocx-pdf with a custom emitter. Read rdocx-layout's `LayoutResult` (paragraphs, runs, positioned glyphs, fonts) and emit PDF using `krilla` (preferred — pure Rust, font subsetting, PDF/A support, A used it successfully) OR `pdf-writer` (lower-level, used by Typst). ~2-3 hours.

If Path A succeeds, opt-5 is a one-line build-config change. If Path B is needed, opt-5 is a meaningful refactor but well-bounded — we're not rewriting layout, just the final PDF write step.

### Pre-Path-A diagnostic (cheap)

Before either: diff the actual PDF byte streams of `cdc_ngs_validation.pdf` between native and WASM. `qpdf --qdf` decompresses streams; `diff` will point at the exact divergent code path. May immediately reveal which rdocx-pdf code is at fault.

## Out-of-scope for opt-5

- The textbox/sidebar extraction gap (`un_seea_policy_brief` 0.00). That's an upstream rdocx parser limitation. B handles it because Typst's docx-rs reader can. Not addressable without parser changes.
- Approach B as a deployed target. 14 MiB gz doesn't fit Workers. The only way to get B's quality into Workers is to port its writer behavior, not its codebase.

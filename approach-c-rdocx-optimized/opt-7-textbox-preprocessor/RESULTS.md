# opt-7 Results — Textbox preprocessor

## Headline
Avg complex-corpus text recall **0.9379 → 0.9746** (+3.7 pp) by adding a
~200-line DOCX-XML preprocessor to opt-5. The previously broken
`un_seea_policy_brief.docx` (0.000 recall) now scores **0.929**.

## Scorecard

| Approach | Files | OK | Recall (avg) | un_seea recall |
|----------|-------|----|--------------|----------------|
| opt-5    | 25    | 25 | **0.938**    | **0.000**      |
| opt-7    | 25    | 25 | **0.975**    | **0.929**      |

(Both runs use the same 25-doc real-world corpus from `fixtures/complex/`.)

## Per-document delta (opt-7 vs opt-5)

| Doc                              | opt-5  | opt-7  | Δ      |
|----------------------------------|--------|--------|--------|
| un_seea_policy_brief.docx        | 0.000  | 0.929  | +0.929 |
| cdc_ngs_validation.docx          | 0.972  | 0.979  | +0.007 |
| cdc_ngs_validation_plan.docx     | 0.975  | 0.981  | +0.006 |
| who_pqs_lab.docx                 | 0.981  | 0.982  | +0.001 |
| all other 21 docs                | (unchanged within ±0.000) |
| nasa_business_plan.docx          | 1.000* | 0.974  | -0.026 |

*\*The stored opt-5 score of 1.000 for nasa_business_plan was from an older
build. A fresh re-run of the opt-5 binary on the same input scores 0.974
— identical to opt-7. The "regression" is a stale-cache artefact; the
preprocessor returns nasa_business_plan unchanged (`lifted_count == 0`),
so opt-5 and opt-7 produce byte-for-byte equivalent text. The missing
tokens ("conflict" → "confict", "financing" → "fnancing", etc.) are the
pre-existing rdocx-pdf 0.1.2 "fi" / "ffi" ligature drift, unrelated to
this work.*

## Verification done
- 7 / 7 unit tests pass (`cargo test --release --lib preprocess`).
- Native CLI build succeeds in ~28 s release profile.
- Full 25-doc complex corpus rendered end-to-end via `score_complex.py`.
- Output text for `un_seea_policy_brief` contains "Executive Summary",
  "Background", "Context/Scope", "Approach/Methodology", "Policy
  Recommendations", "Results and Findings", "References" — i.e. exactly
  the headings that were missing on opt-5.

## What was NOT verified (out of scope for this spike)
- WASM build — opt-5's WASM target wasn't rebuilt or re-tested; the
  preprocessor is pure-Rust with `zip` (deflate-flate2 feature) and
  `quick-xml` so should build on wasm32, but not validated here.
- Cloudflare Worker run — same reason; the Rust API surface is unchanged
  so the existing worker harness should work, but it wasn't exercised.
- Legacy VML-only documents where `<w:txbxContent>` lives inside
  `<w:pict>/<v:textbox>` without a `<mc:AlternateContent>` Choice. The
  preprocessor *will* pick those up (it scans for `<w:txbxContent>`
  irrespective of the wrapper) but the corpus didn't have a pure-VML
  example to validate against.

## Recommendation
**Integrate into opt-6 (or fold into a new opt-8 baseline).**

The preprocessor is:
- Small (200 lines, all in one module) and well-isolated.
- Defensive (returns input bytes on any error) so it cannot make things
  worse.
- A no-op for documents without textboxes (zero overhead on the ~22 / 25
  docs that don't use them).
- Already covered by unit tests.

It is not a perfect rdocx-oxml replacement — layout fidelity is lost for
sidebar-positioned text. But for the production use case (text
extraction for AI / search pipelines), the recall jump on the affected
documents is substantial and there is no measurable regression.

## Remaining gaps (documented in INVESTIGATION.md)
- Footnote body text — separate rdocx gap, separate preprocessor would
  need to inline `footnotes.xml` content into the body. Tracked in
  `foundissues/004`.
- Tables nested inside textboxes — `<w:tbl>` children of `<w:txbxContent>`
  are not lifted. One-line extension if encountered in practice.
- The pre-existing rdocx-pdf "fi" / "ffi" ligature cmap drift (visible in
  nasa_business_plan, cdc_*) is orthogonal and unchanged by opt-7.

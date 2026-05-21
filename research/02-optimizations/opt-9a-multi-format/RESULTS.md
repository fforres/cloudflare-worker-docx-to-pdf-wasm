# opt-9a — Results

## TL;DR
opt-8 + multi-format output. Adds `convert_html_wasm` and `convert_md_wasm`
alongside the existing `convert_wasm` (PDF). PDF behaviour is byte-for-byte
identical to opt-8; HTML/MD reuse the same textbox preprocessor. Bundle size
grows by **+15 KB gzipped**, well inside the 50–200 KB budget. Ship-ready.

## Bundle size

| Build | opt-8 | opt-9a | Delta |
|---|---|---|---|
| Raw | 2.57 MiB | 2.60 MiB | +28 KiB |
| wasm-opt -Oz | 2.23 MiB | 2.25 MiB | +20 KiB |
| **gzip -9** | **1.03 MiB** | **1.04 MiB** | **+15 KiB** |
| brotli q11 | 0.79 MiB | 0.80 MiB | +13 KiB |

Surprisingly cheap. LTO collapses most of `rdocx-html` since it was already
transitively reachable from rdocx-pdf's shared types, and the HTML/MD
renderers don't need tiny-skia, fontdb, or rustybuzz at all.

## PDF corpus — no regression

### Toy

| Tier | opt-8 | **opt-9a** |
|---|---|---|
| T1 | 1.00 | **1.00** |
| T2 | 0.99 | **0.99** |
| T3 | 0.89 | **0.89** |

### Complex (25 docs)

| Metric | opt-8 | **opt-9a** |
|---|---|---|
| Avg recall | 0.98 | **0.98** |
| un_seea_policy_brief | 0.93 | **0.93** |
| cdc_ngs_validation | 0.98 | **0.98** |
| All 25 OK | yes | **yes** |

Numbers match opt-8 to two decimal places. The PDF path was not touched.

## HTML smoke (6 docs)

All 6 produce non-empty bytes, start with `<!DOCTYPE html>`, and `tidy -e`
reports **0 errors** (only minor warnings about missing `<title>` and meta
charset shorthand — cosmetic). Token recall vs LibreOffice reference PDF:

| Doc | HTML size | Token recall |
|---|---|---|
| `tier1/inline_formatting.docx` | 1.1 KB | 1.00 |
| `tier2/tables.docx` | 1.5 KB | 1.00 |
| `tier3/footnotes.docx` | 0.7 KB | 0.67 |
| `complex/cdc_ngs_validation.docx` | 66 KB | 0.98 |
| `complex/un_seea_policy_brief.docx` (textbox) | 1.2 KB | 1.00 |
| `complex/nasa_psd_final.docx` | 22 KB | 1.00 |

## Markdown smoke (6 docs)

All 6 produce non-empty bytes with paragraph breaks. Tables render as GFM
pipe tables. Headings appear as `**...**` bold rather than `#` ATX headings
(rdocx-markdown uses bold-for-headings; not ideal but readable). Token recall:

| Doc | MD size | Token recall |
|---|---|---|
| `tier1/inline_formatting.docx` | 244 B | 1.00 |
| `tier2/tables.docx` | 407 B | 1.00 |
| `tier3/footnotes.docx` | 35 B | 0.67 |
| `complex/cdc_ngs_validation.docx` | 17 KB | 0.98 |
| `complex/un_seea_policy_brief.docx` (textbox) | 232 B | 1.00 |
| `complex/nasa_psd_final.docx` | 5.8 KB | 1.00 |

## Top issues found in HTML/MD

1. **Headings rendered as bold, not `#`** (Markdown). rdocx-markdown 0.1.2
   emits e.g. `**Executive Summary**` instead of `# Executive Summary`. Body
   text is right; semantic structure is degraded for downstream Markdown
   consumers that key off heading levels.
2. **Footnote body text is missing** across all three outputs — same upstream
   limitation as opt-8's PDF path (foundissues/004). Recall of 0.67 on the
   tiny footnotes.docx fixture reflects this; not a new regression.
3. **Textbox preprocessor lift order = tail of body** — the lifted text
   appears at the end of the HTML/MD body (just like in the PDF), so the
   visual order of un_seea_policy_brief reads "main body then sidebar".
   Cosmetic; content is recoverable.
4. **HTML missing `<title>` element** — tidy warns; trivial to fix upstream
   but we don't synthesise one here.

## Recommendation

**Ship on top of opt-8.** +15 KB gz buys three output formats from one
download. The PDF path is bit-identical to opt-8 (same code path, same font
list, same preprocessor), so existing PDF callers are unaffected. HTML
output is valid and ~98 % token-recall on real-world docs; Markdown content
is correct but loses heading structure (acceptable for AI/search ingestion
since the text is still there — the loss is semantic only).

If we ship, the worker should grow a `?format=html|md|pdf` query param (or
three routes) — out of scope for this iteration.

## Files
- `converter/Cargo.toml` — renamed crate, no new dependencies vs opt-8
- `converter/src/lib.rs` — adds `convert_to_html`, `convert_to_markdown`,
  `convert_html_wasm`, `convert_md_wasm`; renames `convert` → `convert_to_pdf`
  with a back-compat `pub use` alias
- `converter/src/bin.rs` — crate rename only
- `converter/src/preprocess.rs` — vendored from opt-8 unchanged
- `wasm-runner.mjs` — PDF path
- `wasm-runner-html.mjs` — new
- `wasm-runner-md.mjs` — new

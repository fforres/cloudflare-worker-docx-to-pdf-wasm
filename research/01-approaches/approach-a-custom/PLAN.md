# Approach A — Custom stack (docx-rs + krilla)

## Goal
The smallest possible WASM that converts DOCX → PDF and passes **Tier 1** text-recall
(≥ 0.85 average). Optimize for size, not feature parity.

## Stack
- **`docx-rs` v0.4.20** — OOXML parser. Pure Rust, no_std-ish, parses paragraphs,
  runs, bold/italic, lists.
- **`krilla` v0.7.0** — PDF emission with built-in font subsetting.
- **`ttf-parser`** — read glyph advance widths from the bundled font. (krilla pulls
  this in transitively, we reuse it.)
- **Liberation Serif Regular** (OFL) — single bundled font, embedded via
  `include_bytes!`. Source: https://github.com/liberationfonts/liberation-fonts
  Release 2.1.5 tarball.
- No `cosmic-text`, no `parley`, no `rustybuzz`. No text shaping. Naive
  per-glyph advance widths. Acceptable for Latin Tier 1 fixtures.

## In scope (v1)
- Paragraph text content (concatenated runs).
- Bold / italic run flags → switch font family. (Skip — single font face to save
  size. Just emit all text in regular.) **Cut bold/italic font swap to save ~600
  KB per face.** Bold/italic markers from docx are read but rendered as regular.
- Naive word wrap at a fixed text-area width (US Letter, 1" margins).
- Page break when y position exceeds page bottom.
- List items: prefix with "• " for bullets, "N. " for numbered (count locally,
  ignore numId/abstractNumId).
- Headings: same font, slightly larger size (12pt body, 16pt heading).
- Hyperlinks: render anchor text (no annotation).

## Explicit non-goals
- Tables (no rendering — skip the entire `w:tbl` element)
- Images (skip `w:drawing`)
- Headers / footers / page numbers (skip)
- SectPr, columns, sections
- Footnotes / endnotes
- Track changes
- Text shaping / kerning / ligatures
- Bidi / RTL
- Font fallback / language coverage outside Latin-1
- Exact OOXML numbering (`w:numPr` / `numbering.xml`)
- Bold/italic visual distinction (read but flatten)

## WASM size budget
Target: **≤ 3 MiB gzip**. Hard ceiling: 10 MiB gzip.

Anticipated cost breakdown (rough):
- Liberation Serif Regular TTF: ~163 KB uncompressed, ~85 KB after krilla
  subsets to the document's used glyphs.
- `docx-rs` + `quick-xml` + `zip`: ~500 KB-1.2 MiB of WASM code.
- `krilla` core: ~800 KB-1.5 MiB.
- `ttf-parser`, `pdf-writer`, `flate2`: ~300-500 KB.

If `docx-rs` blows past 1.5 MiB on its own, fall back to hand-rolled XML
extraction via `quick-xml` + `zip`.

## Failure modes
- `krilla` may pull `harfbuzz_rs` / `image` / `resvg` — those would push us over.
  If so, audit Cargo.lock and disable features.
- `docx-rs` is known to have a lot of error types — `serde` derives can bloat.
  Likely OK because we're not using the writer.
- wasm32-unknown-unknown has no `time`, `fs`, `getrandom`. We need
  `getrandom` with the "custom" feature pointing at a stub.

## Plan
1. Scaffold Cargo project (lib + bin + cdylib).
2. Fetch Liberation Serif Regular OFL.
3. Implement DOCX→text-blocks extraction with `docx-rs`.
4. Implement minimal layout (font metrics from ttf-parser, naive wrap).
5. Emit PDF with `krilla`.
6. Native CLI works on Tier 1 fixtures.
7. WASM build. Measure with `harness/wasm-size.sh`.
8. Run harness, record results.

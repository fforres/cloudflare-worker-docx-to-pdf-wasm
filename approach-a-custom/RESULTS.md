# Approach A — Results

## TL;DR
Smallest of the three approaches by design. Passes Tier 1 cleanly, gets
surprisingly far on Tier 2 because most fixtures are paragraph-heavy. Fails on
content types the PoC explicitly skips (tables, images, textboxes).

## WASM size
Measured with `harness/wasm-size.sh` on `wasm32-unknown-unknown` release build
(`opt-level = "z"`, `lto = true`, `panic = "abort"`, `strip = true`):

| Stage          | Bytes      | MiB     |
|----------------|-----------:|--------:|
| raw            | 2,425,616  | 2.31    |
| wasm-opt -Oz   | 2,065,225  | 1.96    |
| gzip -9        |   943,607  | **0.89**|
| brotli -q11    |   747,144  | 0.71    |

CF Workers ceiling is 10 MiB compressed. **Margin: ~9.55 MiB.** Plenty of
headroom to bundle a couple of extra fonts or add a table renderer later.

## Corpus results

| Tier | Files | OK   | Recall (avg) | Page Δ (avg) | Img Δ (avg) |
|------|------:|-----:|-------------:|-------------:|------------:|
| T1   |     5 |  5/5 |     **1.00** |         0.00 |         0.0 |
| T2   |    10 | 10/10|         0.79 |         0.10 |         0.2 |
| T3   |    10 |  9/10|         0.62 |         0.00 |         1.2 |

**Tier 1 gate passes**: 5/5 conv-OK, recall 1.00 (well above the 0.85 target).
**Tier 2 gate passes** for conversion-OK count (10/10) but average recall (0.79)
drops below the 0.85 target — driven mostly by `tables.docx` (0.19) and
`EmptyDocumentWithHeaderFooter.docx` (0.00, has only header/footer content
which we skip). Excluding those two, T2 recall averages ~0.96.

## What's missing relative to T2

- **Tables** — `w:tbl` elements are skipped entirely. `tier2/tables.docx`
  recall drops to 0.19 as a result. `tier3/table_header_rowspan.docx` → 0.00.
- **Headers / footers** — never read; `EmptyDocumentWithHeaderFooter.docx` →
  0.00.
- **Images** — no rendering and no placeholder text, so image-only documents
  produce a tiny "blank page" PDF (1457 B).
- **Footnotes / endnotes** — not read, `footnotes.docx` and `notes.docx` drop.
- **Bold / italic visual distinction** — we read the flags but render
  everything in Regular. Saves the cost of bundling Bold + Italic faces
  (~325 KB extra TTF, ~80 KB more gzipped after subsetting).
- **Real numbering resolution** — numbered lists use a per-paragraph local
  counter, not the OOXML `numId/abstractNumId` tree. Works on flat lists,
  doesn't restart correctly across nested numbering schemes.
- **Page breaks from sectPr / explicit `<w:br type="page">`** — not honored.

## Biggest unexpected dep cost
None, really. The build came in light:
- `krilla` with `default-features = false` drops `rustybuzz` and the image
  decoders — that was the biggest planned win.
- `docx-rs` pulls `quick-xml`, `zip`, `serde`, `serde_json`. `serde_json` was
  the only mild surprise (~150 KB raw), needed because `docx-rs` exposes a
  serde-based AST. Couldn't easily strip it without forking the crate.
- `skrifa` + `subsetter` arrive transitively for krilla's font handling. Used
  them instead of adding our own glyph-id lookup path → smaller total.

## What we cut to get there
- No text shaping. `draw_glyphs` with per-codepoint glyph IDs and
  ttf-parser-measured advances. Latin-only, no kerning, no ligatures, no
  combining marks. Good enough for the Latin Tier 1 corpus.
- Single font face. Bold/italic markers are read from the AST but not visually
  rendered.
- One thread-local output buffer + four C ABI entry points (`awd_alloc`,
  `awd_convert`, `awd_out_ptr`, `awd_out_len`, `awd_out_free`). No
  `wasm-bindgen`.

## Recommendation
**A viable Workers candidate** at < 1 MiB gzip. Tier-1 gate cleanly cleared.
If we want to also clear T2's text-recall gate, the cheapest additions are:
1. A minimal table renderer that just concatenates cell text into paragraphs
   (would push tables.docx from 0.19 → ~0.95 without doing real layout).
2. Read `w:hdr` / `w:ftr` parts and render them as inline paragraphs at the
   top/bottom of each page.

Both are < 1 MiB of additional code, well within our 9.5 MiB headroom.

## Reproducing
```bash
# native
cd approach-a-custom/converter
cargo build --release --bin approach-a-custom-cli

# wasm
cargo build --release --lib --target wasm32-unknown-unknown
../../harness/wasm-size.sh target/wasm32-unknown-unknown/release/approach_a_custom.wasm

# scorecard
cd ../..
python3 harness/score.py approach-a-custom \
    approach-a-custom/converter/target/release/approach-a-custom-cli
```

## Font attribution
Liberation Serif Regular, copyright Red Hat, Inc., released under the SIL
Open Font License v1.1. Source:
https://github.com/liberationfonts/liberation-fonts (release 2.1.5).
Full license at `converter/fonts/LICENSE`.

---
slug: rdocx-pdf-carlito-tounicode-cmap
severity: high
status: confirmed-with-clean-room-repro
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-pdf 0.1.2)
affects-versions:
  - rdocx 0.1.2
  - rdocx-pdf 0.1.2
  - rdocx-layout 0.1.2 (when bundled-fonts feature enabled)
discovered-on: 2026-05-20
project-impact: |
  Before the workaround, real-world DOCX corpus recall was 0.71 (vs 0.94 on the
  same corpus rendered natively without bundled-fonts). 8 of 25 representative
  business / government documents had body text scrambled in the PDF.
workaround-in-this-repo: |
  approach-c-rdocx-optimized/opt-5-complex-corpus/ — disable rdocx-layout's
  bundled-fonts feature, bundle Liberation Sans/Serif/Mono full faces only, and
  register them with Document::to_pdf_with_fonts(...) under 23 alias names
  (Calibri, Calibri Light, Cambria, Arial, Times New Roman, Tahoma, Verdana,
  Courier New, Consolas, ...). User-supplied fonts win lookup priority, so
  Liberation always wins over the buggy Carlito.
repro-available: yes
repro-fixture: fixtures/complex/cdc_ngs_validation.docx
---

# rdocx-pdf produces scrambled ToUnicode CMap when text is rendered with Carlito (and likely Caladea)

## Summary
When `rdocx-layout`'s `bundled-fonts` feature is enabled and rdocx's font resolver picks **Carlito** for a paragraph, the resulting PDF's `/ToUnicode` CMap stream maps the embedded glyph IDs to the wrong codepoints. The rendered glyphs are visually plausible (or nearly so) but `pdftotext`, search, copy-paste, and screen readers see scrambled text.

The same code path resolved to any non-Carlito family (system Tahoma/Helvetica/Calibri, or user-supplied Liberation Sans/Serif) renders the same documents correctly.

## Steps to reproduce
1. Build a minimal crate that:
   ```toml
   [dependencies]
   rdocx = "0.1.2"
   rdocx-layout = { version = "0.1.2", features = ["bundled-fonts"] }
   ```
2. Use a document whose runs request `Calibri Light` or `Cambria` (any modern MS Office doc — the cmap-drift fixture in our corpus is `fixtures/complex/cdc_ngs_validation.docx`, but a freshly-saved-from-Word file works).
3. Render:
   ```rust
   let doc = rdocx::Document::from_bytes(&docx_bytes)?;
   let pdf = doc.to_pdf()?;            // resolves to bundled Carlito because no system fonts on wasm32
   std::fs::write("out.pdf", &pdf)?;
   ```
4. `pdftotext out.pdf -` and compare to the original DOCX text.

Expected: `"The Next Generation Sequencing utility is a collaboration between..."`
Got: `"The Next eeeeatioe Sequeecieg uutiit neiitiie i t coiitloatioe letbeee..."`

The shapes on the page are close to right; the codepoints in the text layer are systematically wrong.

## Why this hid in earlier testing
- On platforms with system fonts installed (e.g. a macOS dev box with LibreOffice installed → has Tahoma, Helvetica, Calibri available system-wide), `fontdb.load_system_fonts()` resolves common families to *those* fonts, and Carlito never wins lookup, so the bug never triggers.
- Bug ALWAYS triggers in `wasm32-unknown-unknown` because there are no system fonts and Carlito always wins.
- Native macOS builds with `bundled-fonts` enabled DID reproduce the bug in a clean-room test where we forced Carlito to win by suppressing system-font discovery — but this only became obvious after extensive WASM/native A-B comparisons.

## Probable area of the bug
The PDF text layer is built in `rdocx-pdf` 0.1.2 (`src/lib.rs` `render_to_pdf` and the font-embed helpers it calls). The ToUnicode CMap is written from the layout's positioned-glyph table; somewhere in that path, the glyph ID → codepoint reverse map is wrong specifically for Carlito.

Suspect: an indexing or subset-cmap step that assumes a particular cmap subtable format which Carlito's TTF doesn't conform to, OR a write-fonts (`skrifa` / `write-fonts` ecosystem) issue when consuming Carlito's specific glyph table layout.

Caladea (also shipped under `bundled-fonts`) appears to share architecture with Carlito and may have the same problem — see [issue 007](007-caladea-suspected-cmap-bug.md).

## Suggested fixes
1. Verify the `/ToUnicode` CMap stream emitted for a Carlito-rendered page is the inverse of the cmap subtable in the embedded font subset.
2. Compare against the same code path with a Liberation Sans subset to see where the two diverge.
3. If subsetter is at fault, ensure `--retain-gids`-equivalent semantics so glyph IDs in the embedded subset match the IDs used in the page content stream.

## Repro artifacts in this repo
- `fixtures/complex/cdc_ngs_validation.docx` — input
- `reference-pdfs/complex/cdc_ngs_validation.pdf` — LibreOffice reference (clean)
- `results/approach-c-baseline-wasm-complex/complex/cdc_ngs_validation.pdf` — scrambled (rdocx WASM, bundled-fonts)
- `results/approach-c-baseline-native-complex/complex/cdc_ngs_validation.pdf` — clean (rdocx native, system fonts win)
- `results/opt-5-complex-corpus/complex/cdc_ngs_validation.pdf` — clean (rdocx WASM, our Liberation-alias workaround)

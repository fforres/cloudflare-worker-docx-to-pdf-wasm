# opt-8 — Liberation aliases + pyftsubset + textbox preprocessor

Composition of three previous variants' learnings into a single ship-quality build.

## Stack
1. **From opt-5** — bundle only Liberation Sans/Serif/Mono (12 OFL faces) and register them under 23 alias names (Calibri, Calibri Light, Cambria, Arial, Times, Tahoma, Verdana, Courier, Consolas, etc.) via `Document::to_pdf_with_fonts(&[(name, bytes)])`. User-supplied fonts win lookup priority, so Carlito (which trips [foundissues/001](../../foundissues/001-rdocx-pdf-carlito-tounicode-cmap.md)) is never resolved.
2. **From opt-6** — build-time `pyftsubset` (fontTools / Python) trims each Liberation face to Latin-1 + Latin Extended-A/B + general punctuation + currency. ~88 % per-font byte reduction, ~1.8 MiB gz total reduction.
3. **From opt-7** — Rust preprocessor walks the DOCX zip + `document.xml`, finds every `<w:txbxContent>` block, lifts its child `<w:p>` elements to the end of `<w:body>` (before `<w:sectPr>`), strips emptied drawing/pict wrappers, re-zips. Sidesteps [foundissues/002](../../foundissues/002-rdocx-textbox-content-not-extracted.md). No-op for documents without textboxes.

## Build deps
- Rust 1.95+ with `wasm32-unknown-unknown` target
- Python 3 + `fontTools` + `brotli` (`pip3 install --user --break-system-packages fontTools brotli`) — used by `build.rs` to invoke `pyftsubset`.
- The `[patch.crates-io] rdocx-opc` patch (vendored at `converter/patches/rdocx-opc/`) — disables `zip`'s C-backed default features (zstd/bzip2) so the build works on `wasm32-unknown-unknown`. See [foundissues/006](../../foundissues/006-rdocx-opc-zip-features-break-wasm.md).

## Folder
- `converter/` — Cargo crate (lib + bin + cdylib).
- `wasm-runner.mjs` — Node.js harness driver.
- `RESULTS.md` — measured numbers.

## Validation gates
- Toy corpus must keep ≥ opt-6's recall (T1 1.00 / T2 0.99 / T3 0.78).
- Complex corpus must keep ≥ opt-6's 0.94 and SHOULD improve on `un_seea_policy_brief.docx`.
- WASM compressed size ≤ 1.5 MiB (opt-6 is 0.98 MiB; the preprocessor adds ~50 KB of code + zip/quick-xml; budget some growth).
- Worker import test must succeed for at least one cmap-drift doc and one textbox doc.

## Not in scope
- Footnote / endnote body text ([foundissues/004](../../foundissues/004-rdocx-footnote-body-text-not-rendered.md)) — would need a separate preprocessor that copies `word/footnotes.xml` content into `document.xml`. Defer.
- Legacy VML textboxes (`<v:textbox>` not inside `<mc:AlternateContent>`) — preprocessor handles them in principle but the corpus has none, so unverified.
- Track-changes rendering ([foundissues/005](../../foundissues/005-rdocx-track-changes-not-rendered.md)) — scope decision.

# opt-9a — Multi-format output (PDF + HTML + Markdown)

Extends opt-8 by exposing rdocx's `to_html()` and `to_markdown()` alongside the
existing `to_pdf_with_fonts()` path. PDF behaviour is unchanged; the two new
paths reuse the same textbox preprocessor for consistency.

## Stack
1. **Inherits all of opt-8** — Liberation alias map, pyftsubset subsetting,
   textbox preprocessor.
2. **New public API** (`lib.rs`):
   - `convert_to_pdf(&[u8]) -> Result<Vec<u8>, ConvertError>` (renamed from
     `convert`; `pub use convert_to_pdf as convert;` preserves call sites).
   - `convert_to_html(&[u8]) -> Result<Vec<u8>, ConvertError>`
   - `convert_to_markdown(&[u8]) -> Result<Vec<u8>, ConvertError>`
   All three apply `preprocess::preprocess_textboxes` before `Document::from_bytes`.
   HTML/MD do not load fonts (rdocx-html renders HTML/MD without font binaries).
3. **New WASM exports**:
   - `convert_wasm` (PDF, back-compat name kept identical to opt-8)
   - `convert_html_wasm`
   - `convert_md_wasm`
   Same `(out_ptr << 32) | out_len` packing; `out_len == 0` signals error.
   All three share a generic `run_convert<F>` helper that wraps the conversion
   in `std::panic::catch_unwind` and stashes errors in the `LAST_ERROR` slot.
4. **Crate rename** — `approach_c_rdocx_opt8` → `approach_c_rdocx_opt9a`
   (`Cargo.toml`, `src/bin.rs`, `wasm-runner.mjs` path).

## Build deps
Same as opt-8. `rdocx-html` and `tiny-skia` are already in opt-8's transitive
dep graph (verified via `Cargo.lock`); enabling the HTML/MD code paths only
pulls in the additional code through LTO. No new direct `[dependencies]`.

## Folder
- `converter/` — Cargo crate (lib + bin + cdylib).
- `wasm-runner.mjs` — PDF Node driver.
- `wasm-runner-html.mjs` — HTML Node driver.
- `wasm-runner-md.mjs` — Markdown Node driver.

## Validation gates
- WASM gz <= 1.5 MiB (opt-8 is 1.03 MiB; budgeted +50–200 KB).
- Toy corpus: T1 1.00 / T2 0.99 / T3 0.89 (opt-8 levels, no regression).
- Complex corpus: >= 0.98 avg (opt-8 level).
- 6-doc HTML smoke: non-empty bytes, starts with `<!DOCTYPE`, tidy reports
  0 errors, token recall vs LibreOffice reference plausible.
- 6-doc Markdown smoke: non-empty, has paragraph breaks / headings, token
  recall vs LibreOffice reference plausible.

## Not in scope
- Footnote/endnote body text (same upstream limitation as opt-8 — affects MD/HTML
  too; `footnotes.docx` shows 0.67 recall across all three formats).
- HTML/MD scorecard automation in `score.py` (manual smoke only this round).
- Worker integration (would need three routes / a format query param).

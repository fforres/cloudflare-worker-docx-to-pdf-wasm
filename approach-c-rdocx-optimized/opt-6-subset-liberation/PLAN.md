# opt-5-complex-corpus — Plan (executed — see RESULTS.md)

**STATUS: shipped, Path C succeeded in Phase 0–1 with a non-obvious twist.**
Bug was *not* a WASM codegen issue in rdocx-pdf. It is a Carlito-specific
font-data interaction in rdocx-pdf's ToUnicode-CMap writer that fires on
any target (native included) once Carlito wins the font lookup. Fix:
swap `rdocx-layout`'s `bundled-fonts` feature for an explicit
Liberation-only bundle, aliased to every common Word font name and
passed via `doc.to_pdf_with_fonts(...)`. See RESULTS.md.


## The bug

`rdocx-pdf` produces correct PDFs on the **native** target (recall 0.94) but
emits **scrambled text** on `wasm32-unknown-unknown` (recall 0.71). Same code,
same fonts, same docs. Smoking-gun example:

- Reference / native: `"The Next Generation Sequencing utility..."`
- WASM: `"The Next eeeeatioe Sequeecieg uutiit..."`

The PDF is structurally fine (right page count, layout). The **text layer**
(ToUnicode CMap and/or CIDToGIDMap streams) is scrambled. Affects ~32% of real
DOCX, predominantly those using Calibri Light / Cambria / Tahoma.

The bug is **specific to rdocx-pdf on wasm32**. Approach A (krilla, WASM) and
Approach B (typst-pdf, native) both render the same documents correctly,
proving the bug lives in rdocx-pdf's PDF-serialization code under wasm32.

## Strategy & budget (4 h hard cap)

### Phase 0 — Setup [done]
1. Copy baseline converter to `opt-5-complex-corpus/converter/`.
2. Build native + WASM, reproduce `eeeeatioe` gibberish on cdc_ngs_validation.

### Phase 1 — Compile-flag sweep (≤ 45 min)
Run cdc_ngs_validation.docx → pdftotext → grep "Next Generation". Bail at
first success.

| Variant | Result |
|---|---|
| baseline (`opt-level=z lto=fat cgu=1 panic=unwind`) + wasm-opt -Oz | tbd |
| `opt-level=s` | tbd |
| `lto=false` | tbd |
| `lto=thin` | tbd |
| `panic=abort` | tbd |
| `codegen-units=16` | tbd |
| no wasm-opt | tbd |
| wasm-opt -O0 | tbd |
| wasm-opt -O3 | tbd |
| wasm-opt --converge | tbd |
| no --enable-nontrapping-float-to-int | tbd |
| `RUSTFLAGS=-C target-feature=-bulk-memory` | tbd |

If any of these clear `eeeeatioe` -> proceed to Phase 4 immediately.

### Phase 2 — PDF diff diagnostic (≤ 45 min, only if Phase 1 fails)
1. `qpdf --qdf` native + WASM PDFs of cdc_ngs_validation.
2. Diff. Focus on `/ToUnicode`, `/CIDToGIDMap`, font dictionaries.
3. Grep rdocx-pdf source (and write-fonts / skrifa) for that stream code.
4. Look for pointer-width (`as usize`), endianness, unchecked indexing.

### Phase 3 — Implementation path (pick one)
- **Path A**: small patch via `[patch.crates-io]` if Phase 2 finds a localized bug.
- **Path B**: bypass rdocx-pdf: use `rdocx-layout::layout_document` → emit PDF via
  `krilla` (approach A's stack). Bigger refactor but A proves the approach
  works in WASM at 0.89 MiB.
- **Path C** (fallback): rewrite DOCX XML to swap `Calibri Light` → `Calibri`
  before passing to rdocx. Won't fix all cmap drift but should catch the
  Calibri Light subset.

### Phase 4 — Validation
- `harness/score_complex.py` on opt-5 — target avg ≥ 0.85, cdc recall ≥ 0.85.
- `harness/score.py` on toy fixtures — no regression.

### Phase 5 — Worker
- Port **8792**. Same per-request instantiation. Smoke-test 3-5 docs.
- `wrangler deploy --dry-run` for final bundle size. Kill wrangler.

### Phase 6 — Reports
- `RESULTS.md` with scorecard, what worked, honest assessment.

# opt-1: trim bundled fonts

## Scope
Drop `rdocx-layout`'s `bundled-fonts` feature (which ships 22 TTFs ~6.8 MiB). Embed only 4 fonts via `include_bytes!` and pass them to `Document::to_pdf_with_fonts`. Everything else (worker, ABI, patches) stays identical to the parent `approach-c-rdocx` baseline.

## Font choices
- Carlito Regular + Bold (covers Calibri, Calibri Light, Segoe UI, generic sans, default fallback chain ends with "Carlito")
- Liberation Serif Regular + Bold (covers Times New Roman, Cambria→Caladea→Serif generic fallback, Georgia, Garamond)

Source: copied from `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rdocx-layout-0.1.2/fonts/` (same OFL TTFs rdocx ships).

The `family` string in `FontFile` is unused for lookup; fontdb queries the TTF's embedded family name. So we don't need to register under multiple aliases — rdocx's `map_font_name` already remaps Calibri→Carlito etc., and the generic fallback chain ends at "Carlito"/"Liberation Sans"/"Helvetica".

## Expected size
- Baseline gzipped: 4.04 MiB (4.0 MiB of which is fonts)
- 4 Carlito/LibSerif TTFs ≈ ~1.2 MiB raw → ~0.6–0.8 MiB gzipped
- Plus 0.65 MiB code = target ~1.3 MiB gzipped (~70% smaller than baseline)

## What could go wrong
- Italic/BoldItalic faces missing: rdocx may synthesize via fontdb stretch query (style mismatch → falls back to Regular). Acceptable per project's "content > font fidelity" stance.
- Documents that explicitly request Liberation Mono / Courier / monospace get the generic Serif/Sans fallback. Likely slight recall drop on a few T2/T3 docs.
- A document that requests a font *not* in `map_font_name` and not in our 4 might fail with `FontNotFound`. The generic Serif/Sans final fallback uses fontdb generic families — those only resolve if a face advertises itself as serif/sans-serif in OS/2. Carlito and Liberation Serif both do, so we should be covered.

## Workflow
1. Copy `approach-c-rdocx/converter/` (minus `target/`).
2. Strip `bundled-fonts` feature from Cargo.toml. Keep `[patch.crates-io] rdocx-opc`.
3. Add `converter/fonts/` with the 4 TTFs.
4. Replace `convert()` in `src/lib.rs` to call `to_pdf_with_fonts` with `include_bytes!` slices.
5. Build wasm32, measure, copy into worker, score via harness, curl-test worker on :8788.

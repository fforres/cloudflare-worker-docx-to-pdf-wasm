# opt-3 — build-time font subsetting

## Strategy
**Strategy A (pyftsubset)** — `pyftsubset` is in PATH at `~/Library/Python/3.14/bin/pyftsubset`. Build script invokes it per font. Strategy B (subsetter crate) was not attempted because pyftsubset works reliably and is already proven by spot-test below.

## Fonts to subset
All 20 TTFs from `rdocx-layout-0.1.2/fonts/` (Carlito ×4, Caladea ×4, LiberationSans ×4, LiberationSerif ×4, LiberationMono ×4). Disable the `bundled-fonts` feature on `rdocx-layout` and pass our subset fonts via `to_pdf_with_fonts`.

## Codepoint coverage
- U+0000–U+024F — Basic Latin, Latin-1 Supplement, Latin Extended-A, Latin Extended-B
- U+2000–U+206F — General Punctuation (smart quotes, em/en dash, ellipsis…)
- U+20A0–U+20CF — Currency
- U+2070–U+209F — Super/subscript

Hinting dropped, all OT layout features preserved (`--layout-features='*'`), CFF desubroutinized.

## Estimated sizes (spot tests vs source)
| Font | Original | Subset | Ratio |
|---|---|---|---|
| Carlito-Regular | 636 KB | 119 KB | 19% |
| Caladea-Regular | 59 KB | 48 KB | 82% (already Latin-only) |
| LiberationSans-Regular | 350 KB | 42 KB | 12% |
| LiberationSerif-Regular | 388 KB | 46 KB | 12% |
| LiberationMono-Regular | 313 KB | 43 KB | 14% |

Carlito family is the heaviest (~2.8 MB raw) — expected savings ~80% on those. Liberation/Caladea more modest. Total fonts raw is ~6.8 MB; expected subset total ~1.0–1.3 MB raw. Gzipped WASM target: ~1.5–2.0 MiB.

## Files
- `converter/Cargo.toml` — drop `bundled-fonts` feature; drop the `[[bin]]` entry to keep build minimal (lib only).
- `converter/build.rs` — runs pyftsubset on each font into `$OUT_DIR/fonts/`. Path to pyftsubset overridable via `PYFTSUBSET` env var; default `~/Library/Python/3.14/bin/pyftsubset`.
- `converter/src/lib.rs` — `include_bytes!` each subset font, register them via `to_pdf_with_fonts`.
- `converter/patches/rdocx-opc/` — same patch as baseline.
- `worker/` — port 8790, same per-request instantiation.

## Pass criteria
- Builds for `wasm32-unknown-unknown`.
- Gzipped WASM substantially smaller than 4.04 MiB baseline.
- Worker import + curl test produces a valid PDF.
- Harness scorecard T1 recall ≥ 0.85.

## Fallback
If pyftsubset fails or subset fonts break rdocx, fall back to opt-1 (ship 4 full Carlito Reg/Bold + Liberation Serif Reg/Bold) and document why in RESULTS.md.

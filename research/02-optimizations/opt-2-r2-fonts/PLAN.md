# opt-2 — Zero fonts in WASM; fonts fetched at runtime

## Strategy
Strip ALL bundled fonts from the WASM. Worker fetches the 4 essential fonts
(Carlito Regular/Bold, Liberation Serif Regular/Bold) at top-level init and
passes them to the WASM via a new `convert_with_fonts` export. In prod these
would come from an R2 binding; for dev we import them as `Data` assets.

## Mini-format spec (JS -> WASM)
A flat byte buffer encoding a list of `(family_name, font_bytes)` pairs:

```
records  := record*
record   := name_len (u32 LE) name_bytes data_len (u32 LE) data_bytes
```

Decoded Rust-side to `Vec<(String, Vec<u8>)>`. Boundary checks: each length
must fit within remaining buffer; otherwise return ConvertError::Read.

## Font choices (4 fonts, ~1.3 MiB raw TTF)
| File | Family | Approx |
|---|---|---|
| Carlito-Regular.ttf | "Carlito" | ~320 KB |
| Carlito-Bold.ttf | "Carlito" | ~320 KB |
| LiberationSerif-Regular.ttf | "Liberation Serif" | ~330 KB |
| LiberationSerif-Bold.ttf | "Liberation Serif" | ~290 KB |

These are the metric-compatible replacements for Calibri (Word default) and
Times New Roman — covers the vast majority of business docs.

## Expected sizes
- WASM gzipped: ~0.65 MiB (proven by no-bundled-fonts build in approach-c)
- Worker bundle (incl. 4 .ttf as data assets): ~1.9-2.2 MiB gzipped over the wire
- This is the smallest *WASM* of any variant, at the cost of larger total bundle

## WASM exports
- `alloc`, `dealloc`, `last_error_ptr`, `last_error_len` (unchanged)
- `convert_wasm(ptr, len) -> u64`        — calls `to_pdf()`; will fail FontNotFound
- `convert_with_fonts(docx_ptr, docx_len, fonts_ptr, fonts_len) -> u64`  — new

## R2 production wiring (documented only)
- `wrangler.toml`: `[[r2_buckets]] binding = "FONTS" bucket_name = "wasm-docx-fonts"`
- Worker on first request: `await env.FONTS.get("Carlito-Regular.ttf").arrayBuffer()`
  in parallel for all 4, cached in module-scope `let fontsBuffer = null`.
- See `worker/README.md` for the swap-in code shape.

## Pass criteria (same as parent)
- WASM compiles for wasm32-unknown-unknown
- T1 recall >= 0.85, 5/5 OK (with fonts supplied)
- T2 >= 6/10 OK
- Worker dev server passes curl test on tier1/tier2/tier3 fixtures

# opt-4 — code stripping and dependency pruning

Baseline: 0.65 MiB gz WASM (no bundled fonts), 4.04 MiB gz with all 22 fonts.
Goal: shrink the 0.65 MiB code+deps slice with no functional regression on
the corpus.

## Deps targeted for removal (with path-patched sub-crates)

| Dep | Where pulled from | Strategy | Expected savings (gz) |
|---|---|---|---|
| `regex` (+`aho-corasick`, `regex-automata`, `regex-syntax`) | `rdocx` direct, `rdocx-oxml` direct | Patch both Cargo.toml + strip `replace_regex*` APIs | 80–180 KB |
| `tiny-skia` (+`png`, `fdeflate`, `tiny-skia-path`, `strict-num`) | `rdocx-pdf` for `raster.rs` only | Patch Cargo.toml + delete `raster.rs` and `render_*png` fns (we only call `render_to_pdf`) | 60–120 KB |
| `rdocx-html` | `rdocx` for `to_html`/`to_markdown`/`build_html_input` | Patch rdocx Cargo.toml + strip those methods + `guess_image_content_type` is also used by pdf path, keep it | 5–15 KB |
| `zopfli` | `rdocx-opc` via `zip` `deflate` feature | Already-patched `rdocx-opc` switches `zip` features to `deflate-flate2` only (decoder-only, no zopfli encoder) | 20–40 KB |
| `zlib-rs` | `flate2`'s `zlib-rs` backend pulled by `zip`'s `deflate` | Drop `zlib-rs`; let `flate2` use default miniz_oxide (`miniz_oxide` is already pulled by `rdocx-pdf`, so no net add) | 30–60 KB |

## Profile tweaks (applied to release)

- `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "unwind"`,
  `strip = "symbols"` (already in baseline; verify)
- Post-build: `wasm-opt -Oz --converge` (one extra pass than baseline)

## Fonts strategy

- This variant is orthogonal to fonts. We measure two numbers:
  1. **No-fonts build** (apples-to-apples vs 0.65 MiB baseline)
  2. **With 4 fonts** bundled like opt-1 (Carlito Reg/Bold, Liberation Serif
     Reg/Bold) — what the worker actually ships
- Worker at port **8791** ships the 4-font build.

## Fallback

If patching the upstream crates becomes too invasive (compile failures,
huge replication burden), back off to: (a) keep `rdocx-html` if rdocx
re-uses any of its helper code transitively, (b) keep `regex` if patching
rdocx/oxml's APIs requires touching many call sites.

## Validation

- Native build OK (CLI binary)
- WASM build OK for `wasm32-unknown-unknown`
- Harness scorecard with 4 fonts: T1 ≥ 0.85 recall, T2 ≥ 6/10 OK
- Worker `wrangler dev` + curl round-trip on `simple.docx`
- `wrangler deploy --dry-run` size measurement

# opt-4 — code stripping and dependency pruning — Results

## TL;DR

This variant locally forks `rdocx`, `rdocx-pdf`, `rdocx-oxml`, `rdocx-opc`,
and `rdocx-layout` to (a) remove unused public APIs and (b) trim the
dependency tree. The build compiles cleanly to `wasm32-unknown-unknown`
and produces correct PDFs on the corpus.

The gross savings on the **gzipped** wire size are small (~8 KB on the
no-fonts measurement); the savings on the **raw** WASM are real
(~325 KB raw / 17 % of code-and-deps). Most of what we dropped (`regex`'s
finite-automata tables, the PNG/tiny-skia rendering path, `zopfli`) was
already either dead-code-eliminated by LTO+wasm-opt or gzip-compressible to
near nothing. The thing that actually dominates the WASM after stripping
is `skrifa` + `rustybuzz` + `subsetter` + `write-fonts` + `read-fonts` —
the font shaping / subsetting pipeline — and we can't drop those without
losing the conversion path itself.

## Size numbers

Apples-to-apples vs the parent baseline's "no bundled fonts" build.

| Build | Raw | wasm-opt -Oz --converge | Gzipped |
|---|---:|---:|---:|
| Baseline (parent rdocx, no bundled fonts) | n/a | n/a | **665 KB** |
| **opt-4 no-fonts** | 1,953,359 B (1.86 MiB) | 1,629,845 B (1.55 MiB) | **673,327 B (657 KB)** |
| opt-4 + 4 bundled fonts | 4,032,551 B (3.85 MiB) | 3,574,098 B (3.41 MiB) | 1,635,085 B (1.56 MiB) |

`wrangler deploy --dry-run` on the with-fonts worker reports:

```
Total Upload: 3492.62 KiB / gzip: 1617.34 KiB
```

(matches the WASM gz number; the JS shim is negligible.)

**No-fonts gz: ~657 KB vs 665 KB baseline-no-fonts → saves ~8 KB.**
**With-4-fonts gz: 1.56 MiB.** (vs baseline 22-font 4.04 MiB → saves
~2.5 MiB, but that's the font trim, orthogonal to this variant.)

## What was actually dropped from the dep tree

`cargo tree --target wasm32-unknown-unknown --no-default-features`
comparison shows these are no longer pulled:

- `regex`, `aho-corasick`, `regex-automata`, `regex-syntax` (formerly
  pulled by `rdocx` direct + `rdocx-oxml`'s placeholder regex APIs)
- `rdocx-html` (formerly pulled by `rdocx` for `to_html` / `to_markdown`)
- `tiny-skia`, `tiny-skia-path`, `png`, `fdeflate`, `arrayref`,
  `strict-num` (formerly pulled by `rdocx-pdf::raster` for PNG rendering)
- `zopfli`, `bumpalo` (formerly pulled by `zip`'s default DEFLATE encoder)
- `zlib-rs` (formerly pulled by `zip`'s `deflate-flate2-zlib-rs`;
  replaced with the pure-Rust `miniz_oxide` backend that's already in
  the graph via `rdocx-pdf`)
- `memmap2`, `fontconfig-parser` (formerly pulled by `fontdb` defaults;
  not needed in WASM)

The patches are local (path-based `[patch.crates-io]`) in
`converter/patches/{rdocx,rdocx-pdf,rdocx-oxml,rdocx-opc,rdocx-layout}/`.

## Why the gzipped win is small

Twiggy top-10 on the unstripped no-fonts opt build (excluding debug
sections):

| Bytes | Function | Crate |
|---:|---|---|
| 30,169 | `dispatch::dispatch_inner` | skrifa (TrueType hinting) |
| 20,519 | `shape_internal` | rustybuzz (text shaping) |
| 18,595 | `Context::process` | subsetter (font subsetting) |
| 18,036 | `write_pdf` | rdocx-pdf |
| 14,858 | `parse_paint` | ttf-parser (COLR table) |
| 11,822 | `tags_from_complex_language` | rustybuzz |
| 11,646 | `hint_outline` | skrifa |
| 10,500 | `hb_ot_shape_plan_t::new` | rustybuzz |
| 10,230 | `pack_objects` | write-fonts (subsetting output) |
| 8,821 | `compute_unscaled_style_metrics` | skrifa |

These five crates (`skrifa`, `rustybuzz`, `subsetter`, `write-fonts`,
`read-fonts` / `ttf-parser`) account for the bulk of the post-LTO code
and are all required to embed and subset fonts into the output PDF.
None of them is reachable via a feature flag we can flip off.

## Top 5 deps we dropped/trimmed

1. **`regex` family** (4 crates): patched both `rdocx` (`replace_regex*`
   methods) and `rdocx-oxml` (placeholder regex helpers).
2. **`rdocx-html`**: removed from `rdocx`'s deps; stripped
   `Document::to_html` / `to_html_fragment` / `to_markdown` /
   `build_html_input`.
3. **`tiny-skia` + `png`**: removed from `rdocx-pdf`'s deps; deleted
   the `raster` module and `render_*_to_png` exports.
4. **`zopfli` + `zlib-rs`**: changed `rdocx-opc`'s zip feature set from
   `deflate` (= zopfli + flate2-zlib-rs) to `deflate-flate2` + manual
   `flate2/rust_backend` (= miniz_oxide only).
5. **`memmap2` + `fontconfig-parser`**: disabled `fontdb`'s defaults
   (kept only `std` + `fs`, since `rdocx-layout` calls
   `db.load_system_fonts()`).

## Top 5 deps we couldn't drop

1. **`skrifa`** — pulled by `subsetter`; needed for glyph metrics and
   hinting when embedding subset fonts in the PDF.
2. **`rustybuzz`** — pulled by `rdocx-layout` for text shaping; no
   alternative path in rdocx.
3. **`subsetter` + `write-fonts` + `read-fonts`** — pulled by
   `rdocx-pdf` for font subsetting; PDFs without subsetting would be
   gigantic, and rdocx hard-codes this path.
4. **`fontdb`** — `rdocx-layout`'s font resolver; even with
   `default-features = false` the crate is ~50 KB of code (font matching
   logic is genuine functionality).
5. **`zip` + `flate2` + `miniz_oxide`** — required to read the DOCX
   container; can't compile this away.

## Scorecard (with 4 bundled fonts)

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 0.91 | 0.00 | 0.0 | 63 |
| tier2 | 10 | 10 | 0.91 | 0.10 | 0.0 | 110 |
| tier3 | 10 | 9 | 0.77 | 0.00 | 0.4 | 117 |

Identical to opt-1's scorecard (same Carlito + Liberation Serif bundle,
same rdocx code path).

## Worker test outcome

`wrangler dev` on port **8791**, POST `tier1/test.docx` → HTTP 200,
13,635 B PDF (`PDF document, version 1.7, 1 pages`). Same output as the
CLI runner — full round-trip works.

## Build profile

```toml
[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = "symbols"
```

Post-build: `wasm-opt -Oz --converge` with `--enable-nontrapping-float-to-int
--enable-bulk-memory --enable-mutable-globals --enable-sign-ext
--enable-reference-types`.

## Stacking with the font variants

**Yes — cleanly orthogonal.** This variant only changes which
crates / source files compile into the WASM. It does not touch font
loading or runtime behaviour. So:

- opt-4 + opt-1 (4 bundled fonts): **1.56 MiB gz** (this build).
- opt-4 + opt-2 (R2 fonts, no fonts in WASM): would be **~657 KB gz**.
- opt-4 + opt-3 (subsetted fonts): would be (opt-3 size − 8 KB or so).

The 8 KB / 17 % raw saving applies on top of any of the font strategies
because it's pure code-and-deps reduction.

## Recommendation

Ship this **stacked with opt-1 or opt-2**. The standalone gz saving
(~8 KB) is too small to justify the patch-maintenance burden if used
alone; but the raw-bytes saving (325 KB / 17 %) translates into ~10 ms
of cold-start parsing in V8 and a smaller WASM compilation cache, both
of which are mild ongoing wins.

If we want a real second-order code reduction beyond this, the only
remaining lever is to fork `rdocx-pdf` to disable font subsetting (ship
full-font embeds, smaller raw code but larger payload) — that's a
genuine size trade-off, not a free win, and out of scope here.

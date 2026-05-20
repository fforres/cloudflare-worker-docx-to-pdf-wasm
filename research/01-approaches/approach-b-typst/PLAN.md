# Approach B — `office2pdf` / Typst

## Approach in one paragraph
The `office2pdf` crate (v0.5.0, `developer0hye/office2pdf`) is a pure-Rust
DOCX→PDF pipeline that parses DOCX with a patched `docx-rs`, lowers to an
internal IR, generates Typst markup, and renders to PDF via `typst` +
`typst-pdf` v0.14. It ships a `wasm` feature flag with `#[wasm_bindgen]`
exports and a `convert_bytes(...)` entry point that does not touch the
filesystem — exactly the shape we need for Cloudflare Workers.

This is by far the most promising candidate of the three approaches in
terms of "code already written for our problem". The only real unknown is
whether the WASM bundle fits under Workers' 10 MiB compressed cap, given
the heavy dependency graph (Typst + embedded fonts + image crate).

## Order of operations
1. **Native baseline**: `cargo install office2pdf-cli@0.5.0`. Score the
   corpus with `harness/score.py office2pdf-native ./native-wrapper.sh`
   so we know the upper bound of fidelity the Typst pipeline can achieve.
2. **Custom converter crate**: a thin wrapper around the `office2pdf`
   library, exposing both a native CLI and a `cdylib` build target for
   `wasm32-unknown-unknown` (using the upstream `wasm` feature). The CLI
   binary signature is `binary INPUT.docx OUTPUT.pdf` to match the harness.
3. **WASM build**: `cargo build --release --target wasm32-unknown-unknown
   --features wasm`. Strip + `wasm-opt -Oz`, then `gzip -9`. Report size.
4. **Re-score with our binary** to make sure we did not regress anything
   relative to the upstream CLI.
5. **RESULTS.md**.

## What I expect to work
- DOCX text extraction with reasonable token recall (target ≥ 0.85 on T1).
- Inline formatting (bold/italic), headings, basic lists, simple tables.
- Native build "out of the box" — this is the install path the upstream
  README documents.

## What I expect to be ugly
- **WASM size**: Typst's IR + PDF backend + `typst-kit` font embeddings is
  the biggest single concern. Even with `default-features = false` and
  only `embed-fonts`, expect the compressed bundle to be 6–15 MiB. If it
  blows past 10 MiB compressed, the approach is a Containers candidate
  rather than a Workers candidate.
- Headers/footers and page numbers (T2). The Typst IR may or may not wire
  these through correctly — we will know from `headers.docx`.
- Complex tables (T3): merged cells, nested tables, rowspan. Likely
  partial success at best.
- Tracked changes, drop caps, text boxes (T3): expect failure / silent
  skip.

## Binding / build risks
- **Patched dependencies**: the workspace `Cargo.toml` has
  `[patch.crates-io]` overrides for `umya-spreadsheet` and `docx-rs`. The
  published crate on crates.io must vendor these patches inline; when we
  consume `office2pdf` as a dependency we get the published version
  without those patches. **Possible parse-tolerance regressions on real
  fixtures.** Mitigation: if we hit parse failures, optionally apply the
  same `[patch.crates-io]` block in our own `Cargo.toml`.
- **`typst-kit` font embedding** pulls in ~5 MiB of fonts. We will accept
  this for the PoC; a later pass can strip to one OFL font.
- **`comemo` / `typst` proc-macros**: should work on `wasm32-unknown-unknown`,
  but `typst-kit` has historically had issues with non-`std` runtime font
  loading. The `embed-fonts` feature should avoid this.
- **`getrandom`** is already pinned to `wasm_js` for `wasm32` targets in
  upstream `Cargo.toml`. Good sign — they have done WASM work.
- **`image` crate** with default features pulls in a lot. We accept this
  for the PoC.

## Definition of done
- `native-wrapper.sh` works and `results/office2pdf-native/summary.md`
  exists.
- `converter/target/release/approach-b-converter` exists and
  `results/approach-b/summary.md` exists.
- `wasm-size.sh` has been run against the produced `.wasm` and the
  number is recorded in `RESULTS.md`.
- `RESULTS.md` documents pass counts per tier, recall averages, top
  blockers, and a recommendation.

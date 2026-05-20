# Approach C — rdocx-based PoC

References: [`../PLAN.md`](../PLAN.md), [`../TEST_PLAN.md`](../TEST_PLAN.md). The user's preferred approach.

## What this is
A thin wrapper around the `rdocx` library (https://github.com/tensorbee/rdocx, v0.1.2, MIT/Apache-2.0) that exposes:
- A native CLI binary: `approach_c_rdocx <in.docx> <out.pdf>`.
- A `cdylib` for `wasm32-unknown-unknown` exporting `convert(docx_bytes, font_dir_ptr) -> pdf_bytes`.

`rdocx` ships a full DOCX→PDF pipeline already (parser → layout engine → `rdocx-pdf` emitter via `pdf-writer`). We don't write any layout code; we just call the library.

## Why this is the preferred path
Initial native benchmark against the corpus (via `rdocx-cli`):

| Tier | Files | OK | Avg recall | Avg time |
|------|-------|----|-----------|----------|
| T1   | 5     | 5  | **1.00**  | 26 ms    |
| T2   | 10    | 10 | **0.99**  | 24 ms    |
| T3   | 10    | 9  | **0.78**  | 25 ms    |

One T3 failure (`deep-table-cell.docx`), and two T3 docs with poor text recall (`image_with_textbox_caption.docx`=0.0, `notes.docx`=0.44) — likely because rdocx skips text inside textboxes/footnotes. That's the known fidelity gap.

## What we still don't know
- **Compressed WASM size** — the binding constraint. `rdocx` has 7 sub-crates, transitively uses `tiny-skia`, `skrifa`, `write-fonts`, `subsetter`, `zstd`, `zip`. The 22-second native build hints at substantial code volume.
- Whether `rdocx-pdf` can compile to `wasm32-unknown-unknown` cleanly. The `--png` path uses `tiny-skia` which is pure-Rust but heavy; we don't need PNG, only PDF, so we want to disable that.
- Whether `tokio` / any threading deps sneak in.

## PoC scope (this folder)

1. **Native CLI** wrapping `rdocx`. Mirror what `rdocx-cli` does, but in our own binary so the harness measures our code path, not the upstream CLI.
2. **WASM build**. `cargo build --target wasm32-unknown-unknown --release`. Iterate on feature flags until it builds.
3. **Size measurement**: `harness/wasm-size.sh`. Gate: ≤ 10 MiB compressed.
4. **Corpus run**: confirm WASM-equivalent output matches the native baseline. If it doesn't (different deps active, different code path), document why.

## Explicit non-goals here
- Adding fidelity beyond what `rdocx` natively supports.
- Custom font handling beyond what `rdocx` exposes via `--font-dir`. We'll bundle one or two OFL fallbacks via R2 at the Worker stage, not here.
- Header/footer fixes for `SimpleHeadThreeColFoot.docx` (rdocx is at 0.94 recall — good enough).

## Pass criteria
- WASM compiles for `wasm32-unknown-unknown`.
- Compressed `.wasm` ≤ 10 MiB.
- WASM-driven corpus run reproduces native scorecard within ±5 percentage points on recall.

## If it fails
- If WASM size > 10 MiB: try `wasm-opt -Oz`, `lto = "fat"`, `panic = "abort"`, `codegen-units = 1`. If still over, try disabling `rdocx` features (no PNG render path, no validation, no diff, no HTML).
- If WASM doesn't compile: identify the offending dep (likely candidate: anything with `mio`, threads, `getrandom` without the `wasm-js` backend). Patch with feature flags or `[patch.crates-io]`.
- Worst case: document the blocker in `RESULTS.md` and recommend approach B or A.

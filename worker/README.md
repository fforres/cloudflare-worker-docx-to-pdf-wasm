# Cloudflare Worker: `wasm-docx-to-pdf`

Thin JS worker that wraps the WASM converter built by [`../approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/`](../approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/) — the current best variant. See:
- [`../results/comparison.md`](../results/comparison.md) — original ABC comparison
- [`../approach-c-rdocx-optimized/RESULTS.md`](../approach-c-rdocx-optimized/RESULTS.md) — opt-1…opt-4 size optimizations
- [`../complex-fixtures/COMPARISON.md`](../complex-fixtures/COMPARISON.md) — what real-world docs revealed (the Carlito CMap bug)
- [`../approach-c-rdocx-optimized/opt-5-complex-corpus/RESULTS.md`](../approach-c-rdocx-optimized/opt-5-complex-corpus/RESULTS.md) — Carlito fix
- [`../approach-c-rdocx-optimized/opt-6-subset-liberation/RESULTS.md`](../approach-c-rdocx-optimized/opt-6-subset-liberation/RESULTS.md) — pyftsubset of Liberation
- [`../approach-c-rdocx-optimized/opt-7-textbox-preprocessor/RESULTS.md`](../approach-c-rdocx-optimized/opt-7-textbox-preprocessor/RESULTS.md) — textbox preprocessor
- [`../approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/RESULTS.md`](../approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/RESULTS.md) — opt-8 = current build (stacks opt-5 + opt-6 + opt-7)
- [`../foundissues/`](../foundissues/) — outstanding upstream issues / limitations

## API

```
POST /convert        body: raw DOCX bytes        → 200 application/pdf
GET  /  GET /health  → 200 text/plain (banner)
*                    → 404
```

Errors (invalid DOCX, unsupported document, WASM trap) return `500` with a plain-text message.

## Architecture choices

- **Per-request WASM instance, top-level compiled module.** Compiling the WASM module is expensive; instantiation is cheap (~5–10 ms in V8 after the first call). We keep the compiled module at module scope but instantiate a fresh instance per request. This is necessary because a small number of pathological DOCX inputs (e.g. `deep-table-cell.docx` in the test corpus) trigger genuine WASM traps inside rdocx, poisoning the instance's linear memory; reusing a poisoned instance returns garbage. Fresh-per-request makes every request independent.
- **No bindings (KV/R2/D1/DO).** Self-contained; just the WASM blob.
- **No wasm-bindgen.** Custom 5-export C ABI (`alloc`, `dealloc`, `convert_wasm`, `last_error_ptr`, `last_error_len`). Smaller binary, simpler bridge.

## Local dev

```bash
npm install
npx wrangler dev

# In another shell:
curl -X POST --data-binary @../fixtures/tier1/test.docx \
  -o out.pdf http://127.0.0.1:8787/convert
```

## Deploy

```bash
npx wrangler deploy
```

Bundle size: ~2.2 MiB raw, **~1.03 MiB gzipped**. ~9.0 MiB of headroom under the 10 MiB Workers paid-plan ceiling.

Recall on the 25-doc real-world complex corpus: **0.98 avg** (up from 0.71 on early builds — see [`../complex-fixtures/COMPARISON.md`](../complex-fixtures/COMPARISON.md) for the journey).

Toy corpus: T1 1.00 / T2 0.99 / T3 0.89.

## Updating the WASM

When `approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/converter/` changes:

```bash
cd ../approach-c-rdocx-optimized/opt-8-textbox-preprocessor-subset/converter
cargo build --release --target wasm32-unknown-unknown --no-default-features
wasm-opt -Oz --enable-bulk-memory --enable-sign-ext \
  --enable-nontrapping-float-to-int --enable-mutable-globals \
  --enable-reference-types --enable-multivalue \
  target/wasm32-unknown-unknown/release/approach_c_rdocx_opt8.wasm \
  -o ../../../worker/src/converter.wasm
```

Build deps:
- Rust 1.95+ with the `wasm32-unknown-unknown` target.
- Python 3 with `fontTools` installed (`pip3 install --user fontTools brotli`). The `build.rs` invokes `pyftsubset` to shrink the bundled Liberation fonts to a Latin codepoint set. Without it, the build fails.

## Known limitations (see [`../foundissues/`](../foundissues/))

- **Textbox / sidebar content not extracted** ([issue 002](../foundissues/002-rdocx-textbox-content-not-extracted.md)) — magazine-layout documents whose body text lives inside `<w:txbxContent>` produce PDFs with the correct page count but empty body. 1 / 25 docs in our real-world corpus is affected.
- **Footnote / endnote bodies not rendered** ([issue 004](../foundissues/004-rdocx-footnote-body-text-not-rendered.md)) — references appear in the body but the footnote pane is empty.
- **Deeply nested tables rejected** ([issue 003](../foundissues/003-rdocx-deep-nested-tables-fail.md)) — rdocx returns 500 with a clear error message. Isolate stays healthy thanks to fresh-per-request instantiation.
- **Track changes inconsistently rendered** ([issue 005](../foundissues/005-rdocx-track-changes-not-rendered.md)).
- **Font fidelity**: Calibri/Cambria/Arial/Times/etc. are resolved to Liberation Sans/Serif/Mono via an alias table. Visual approximation; text content is correct. See [`../approach-c-rdocx-optimized/opt-5-complex-corpus/RESULTS.md`](../approach-c-rdocx-optimized/opt-5-complex-corpus/RESULTS.md).

## Configuration knobs

In `wrangler.toml`:

- `limits.cpu_ms = 30000` — default. Bump to `300000` (5 min) for large documents.
- Consider adding a `[[r2_buckets]]` binding for additional fonts and a runtime font-resolution path (would let us trim the embedded font set to shrink the bundle).

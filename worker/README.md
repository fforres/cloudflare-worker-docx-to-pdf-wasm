# Cloudflare Worker: `wasm-docx-to-pdf`

Thin JS worker that wraps the WASM converter from [`../approach-c-rdocx/`](../approach-c-rdocx/) (the chosen approach — see [`../results/comparison.md`](../results/comparison.md)).

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

Bundle size: ~8.1 MiB raw, **~4.1 MiB gzipped**. Under the 10 MiB Workers paid-plan ceiling.

## Updating the WASM

When `approach-c-rdocx/converter/` changes:

```bash
cd ../approach-c-rdocx/converter
cargo build --release --target wasm32-unknown-unknown --no-default-features
wasm-opt -Oz --enable-bulk-memory --enable-sign-ext \
  --enable-nontrapping-float-to-int --enable-mutable-globals \
  --enable-reference-types --enable-multivalue \
  target/wasm32-unknown-unknown/release/approach_c_rdocx.wasm \
  -o ../approach_c_rdocx.opt.wasm
cp ../approach_c_rdocx.opt.wasm ../../worker/src/converter.wasm
```

## Known limitations

- Inputs that crash rdocx (`deep-table-cell.docx`-style deeply-nested tables, and some other T3 fixtures) return HTTP 500. The isolate stays healthy thanks to fresh-per-request instantiation, but the document is not converted. Track upstream at https://github.com/tensorbee/rdocx.
- Font fidelity: documents that reference Calibri/Cambria render with bundled OFL replacements (Carlito, Caladea, Liberation). Per-glyph metrics are similar but not identical. Acceptable for the project's "content > font fidelity" priority.

## Configuration knobs

In `wrangler.toml`:

- `limits.cpu_ms = 30000` — default. Bump to `300000` (5 min) for large documents.
- Consider adding a `[[r2_buckets]]` binding for additional fonts and a runtime font-resolution path (would let us trim the embedded font set to shrink the bundle).

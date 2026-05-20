# Cloudflare Worker example

Minimal Cloudflare Worker that consumes the [`docx-to-pdf-wasm`](../../packages/docx-to-pdf-wasm/) package.

## API surface

```
POST /convert         body: raw DOCX bytes          → 200 application/pdf
GET  /  GET /health   → 200 text/plain banner
*                     → 404
```

Returns:
- `200` with `application/pdf` on success
- `400` for empty body
- `405` for non-POST on `/convert`
- `422` for a converter error (malformed DOCX, unsupported document)
- `500` for unexpected errors

## Layout

```
examples/cloudflare-worker/
├── package.json            # workspace consumer of docx-to-pdf-wasm
├── wrangler.toml           # CompiledWasm rule + cpu_ms = 30_000
├── scripts/
│   └── copy-wasm.mjs       # runs as predev/predeploy; copies the package's
│                           # .wasm into src/ so wrangler's bundler picks it up
└── src/
    ├── worker.js
    └── converter.wasm      # copied at build time (in .gitignore)
```

## Why the `copy-wasm.mjs` indirection

Wrangler's `CompiledWasm` loader rule globs are resolved relative to the **worker's** source root, not the project root. So `import wasmModule from "docx-to-pdf-wasm/wasm"` resolves to a path under `node_modules/`, which falls outside the glob match and esbuild bails with *"No loader is configured for .wasm files"*.

The fix is a one-line predev/predeploy step: copy the package's `.wasm` into `src/converter.wasm`, then import it via a relative path. The script uses `require.resolve("docx-to-pdf-wasm/wasm")` so it always picks up the package's current build artifact.

This is the only Worker-specific glue between the package and a wrangler project. The package itself stays runtime-agnostic.

## Local dev

```bash
# From the repo root
pnpm install            # symlinks the workspace package
pnpm --filter cloudflare-worker-example dev
```

In another shell:

```bash
curl -X POST --data-binary @path/to/some.docx \
  -o out.pdf http://127.0.0.1:8787/convert
```

## Deploy

```bash
pnpm --filter cloudflare-worker-example deploy
```

You'll need a Cloudflare account on a paid plan for the 10 MiB bundle limit (free plan is 3 MiB; this bundle is ~1 MiB gzipped so it would technically fit, but `cpu_ms` longer than 10 ms requires the paid plan).

## What the bundle includes

After `wrangler deploy --dry-run`:

```
Total Upload: 2286 KiB / gzip: 1078 KiB
```

That's:
- ~3 KB of JS shim (`src/worker.js` + the package's compiled TypeScript)
- ~2.2 MiB of WASM (the package's `build/docx-to-pdf.wasm`)

## Resilience notes

The worker uses the package's `convert()` function, which instantiates a fresh `WebAssembly.Instance` per request. This is necessary because a small number of pathological DOCX inputs can trap inside Rust and poison the linear memory of a reused instance. With per-request instantiation, every request is independent — a bad input returns a 422 without affecting subsequent requests.

The expensive part (compiling the `WebAssembly.Module`) happens once per isolate at module top level and stays cached by V8 / workerd.

## Configuration knobs

In `wrangler.toml`:

- `limits.cpu_ms = 30000` — default. Bump to `300000` (5 min) for very large documents (e.g. 1000-page books).
- Add `[[r2_buckets]]` if you want to fetch DOCX inputs from R2 instead of POST bodies.

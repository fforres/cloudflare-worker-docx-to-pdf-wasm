# Deploying to Cloudflare Workers

Two ways to ship this, depending on which example you want to base your worker on. Both are inside [`examples/`](./examples/).

## Prerequisites

1. **Cloudflare account on a paid plan.** The free plan has a 3 MiB bundle limit and a 10 ms CPU limit; both workers ship at ~1 MiB and need CPU time well over 10 ms for non-trivial documents. The Workers Paid plan (\$5/month) gives you 10 MiB bundles and CPU up to 5 minutes.
2. **`pnpm install`** at the repo root so the workspace symlinks resolve.
3. **`wrangler login`** once, to authenticate the wrangler CLI against your CF account:
   ```bash
   pnpm --filter cloudflare-worker-example exec wrangler login
   ```

## Option A — JavaScript worker (`examples/cloudflare-worker/`)

Thin JS shim that imports the package's WASM. Easiest to extend with JS-side logic (KV bindings, auth middleware, fan-out, etc.).

```bash
pnpm install
pnpm --filter docx-to-pdf-wasm build                # builds the package
pnpm --filter cloudflare-worker-example deploy      # builds + uploads to CF
```

What `deploy` does under the hood:
1. Runs `predeploy` → `scripts/copy-wasm.mjs`, copying the package's compiled WASM into `src/converter.wasm`.
2. Runs `wrangler deploy`, which bundles `src/worker.js` + `src/converter.wasm` and uploads.

You'll see something like:
```
Total Upload: 2286 KiB / gzip: 1078 KiB
Uploaded docx-to-pdf-worker (1.23 sec)
Deployed docx-to-pdf-worker triggers (0.45 sec)
  https://docx-to-pdf-worker.<your-subdomain>.workers.dev
```

## Option B — Pure-Rust worker (`examples/rust-worker/`)

The entire worker (HTTP routing + DOCX conversion) compiled to a single WASM module via [`workers-rs`](https://github.com/cloudflare/workers-rs). Slightly faster, uses less peak memory under burst load (see [`research/05-benchmarks/`](./research/05-benchmarks/)).

```bash
pnpm install                                       # one-time
cd examples/rust-worker
pnpm exec wrangler deploy                          # cargo + worker-build + upload
```

The first build is slow (~2-3 min) because cargo compiles the entire Rust dep tree (rdocx + transitive crates) for the `wasm32-unknown-unknown` target. Subsequent incremental builds are ~10 s.

You'll need the Rust toolchain installed locally for this option:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
```

`worker-build` is installed automatically on first run.

## Pre-flight check (dry-run, no upload)

To see the bundle that would be uploaded without actually deploying:

```bash
# JS worker
cd examples/cloudflare-worker
pnpm exec wrangler deploy --dry-run --outdir=dist
# Prints "Total Upload: … KiB / gzip: … KiB"

# Rust worker
cd examples/rust-worker
pnpm exec wrangler deploy --dry-run --outdir=dist
```

## After deploy: hit it

```bash
curl -X POST \
  --data-binary @path/to/some.docx \
  -o out.pdf \
  https://docx-to-pdf-worker.<your-subdomain>.workers.dev/convert

curl -X POST \
  --data-binary @path/to/some.docx \
  -o out.html \
  https://docx-to-pdf-worker.<your-subdomain>.workers.dev/convert/html

curl -X POST \
  --data-binary @path/to/some.docx \
  -o out.md \
  https://docx-to-pdf-worker.<your-subdomain>.workers.dev/convert/markdown
```

## Knobs in `wrangler.toml`

Both examples have a configurable CPU limit:

```toml
[limits]
cpu_ms = 30000   # default; bump to 300_000 (5 min) for very large documents
```

For documents over ~200 pages where the conversion takes >30 s, raise `cpu_ms`. For 90 % of typical business documents the default is fine.

## Memory caveat

CF Workers paid-plan isolates have a **128 MB memory limit** (V8 heap + WASM linear memory combined). The converter uses roughly 5–10 MB per output page of PDF state — see [`research/05-benchmarks/MEMORY.md`](./research/05-benchmarks/MEMORY.md). Documents up to ~100 pages convert comfortably; multi-hundred-page documents may exceed the budget and need either HTML/Markdown output (about half the memory of PDF) or a Cloudflare Containers deployment (no per-isolate memory cap).

## Roll back

```bash
pnpm exec wrangler rollback                        # interactive list of recent deployments
pnpm exec wrangler rollback <version-id>
```

## Other deploy targets

This package isn't Cloudflare-specific — the WASM runs anywhere modern WebAssembly does. To deploy to other places (Vercel Edge, Deno Deploy, AWS Lambda, etc.), see the runtime recipes in [`packages/docx-to-pdf-wasm/README.md`](./packages/docx-to-pdf-wasm/README.md). You'd typically wrap the same `convertToPdf` / `convertToHtml` / `convertToMarkdown` calls in whatever HTTP shim the target platform expects.

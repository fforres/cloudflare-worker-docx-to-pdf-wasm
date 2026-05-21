# rust-worker

A **pure-Rust Cloudflare Worker** that mirrors the JS worker in
`examples/cloudflare-worker/`. Both the HTTP routing layer and the DOCX →
PDF / HTML / Markdown conversion are written in Rust and compiled into a
single WASM module via the
[`workers-rs`](https://github.com/cloudflare/workers-rs) crate.

This sits next to the JS worker as a parallel example — the JS worker stays
the primary "use this WASM from JS" sample, and this one demonstrates the
"the entire worker is Rust" path. **Both serve the same routes with the same
response shapes.**

## Routes

| Method | Path                | Response                                  |
|--------|---------------------|-------------------------------------------|
| GET    | `/`                 | 200 text banner                           |
| GET    | `/health`           | 200 text banner                           |
| POST   | `/convert`          | 200 `application/pdf`                     |
| POST   | `/convert/html`     | 200 `text/html; charset=utf-8`            |
| POST   | `/convert/markdown` | 200 `text/markdown; charset=utf-8`        |
| *      | *                   | 404                                       |

Errors:

- empty body → 400
- `ConvertError::Read` / `Render` from rdocx → 422 with the message as text
- non-POST on `/convert*` → 405

## Prerequisites

- Rust toolchain with the `wasm32-unknown-unknown` target installed.
- pnpm (the repo is a pnpm workspace).
- `worker-build` is installed automatically by wrangler on first run via the
  `[build] command` in `wrangler.toml`.

## Local dev

```bash
# from the repo root
pnpm install

cd examples/rust-worker
pnpm exec wrangler dev --port 8793 --ip 127.0.0.1
```

Then in another shell:

```bash
curl -s http://127.0.0.1:8793/health

curl -X POST --data-binary @path/to/some.docx -o out.pdf  http://127.0.0.1:8793/convert
curl -X POST --data-binary @path/to/some.docx -o out.html http://127.0.0.1:8793/convert/html
curl -X POST --data-binary @path/to/some.docx -o out.md   http://127.0.0.1:8793/convert/markdown
```

## Deploy

```bash
pnpm exec wrangler deploy
```

## How it's built

`wrangler dev` and `wrangler deploy` both invoke `worker-build --release`,
which:

1. Runs `cargo build --release --target wasm32-unknown-unknown`.
2. Runs `wasm-bindgen` against the resulting `.wasm` to emit
   `build/worker/shim.mjs` (the JS entry point declared in `wrangler.toml`)
   plus the trimmed `build/worker/index_bg.wasm`.
3. Runs `wasm-opt -Oz` for size.

The Rust code lives in `src/lib.rs`. It depends on the opt-9a converter crate
via a relative path:

```toml
approach_c_rdocx_opt9a = { path = "../../research/02-optimizations/opt-9a-multi-format/converter", default-features = false }
```

`default-features = false` keeps the upstream `[[bin]]` (which has
host-only deps) from being pulled in for the wasm32 target.

The `[patch.crates-io]` table that the opt-9a crate uses for `rdocx-opc` is
mirrored in this `Cargo.toml` — Cargo only honors `[patch]` on the binary
crate being built.

## Comparing to the JS worker

| Aspect            | JS worker | Rust worker (this) |
|-------------------|-----------|--------------------|
| Entry point       | `src/worker.js` (ES module) | `src/lib.rs` via `worker-build` shim |
| HTTP routing      | JavaScript | Rust (`workers-rs`) |
| WASM compile cost | Once per isolate (caller manages `WebAssembly.Module`) | Once per isolate (via `wasm-bindgen` glue) |
| Bundle size (gz)  | 1.06 MiB | **1.05 MiB** |
| Cold start        | V8 compiles 2.2 MiB WASM (~30–80 ms) | V8 compiles 2.2 MiB WASM (~30–80 ms) + tiny shim |
| Conversion latency on 670 KB DOCX | PDF 84 ms, HTML 18 ms, MD 14 ms | PDF 75 ms, HTML 16 ms, MD 13 ms |
| Source maintenance | JS routing + Rust converter | Rust everything |

The two workers are functionally identical and within ~5 % of each other on
every dimension. The Rust path is slightly faster (no JS-shim conversion per
call) and one fewer language in the production codebase; the JS path has a
faster edit-reload loop for the HTTP layer.

See `FEASIBILITY.md` for the why-this-works writeup and `RESULTS.md` for the
measured bundle size and the full curl transcript.

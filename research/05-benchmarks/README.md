# 05 — Benchmarks

End-to-end performance comparison of the two example workers (`examples/cloudflare-worker/` JS-shim vs `examples/rust-worker/` pure-Rust), driven by [`../harness/bench.mjs`](../harness/bench.mjs).

## Files

- [`BENCHMARKS.md`](BENCHMARKS.md) — the comparison writeup. **Start here.**
- `js-worker-bench.json` — raw report from the JS worker run (per-doc timings, error samples, RSS samples).
- `rust-worker-bench.json` — same for the pure-Rust worker.

## What the harness does

Six phases against a running worker URL:

1. **Warmup** — 5 PDF requests on a small doc, discarded. Lets V8 JIT-compile the WASM and prime caches.
2. **Corpus sweep** — all 50 corpus docs × 3 output formats sequentially. Measures real-world-mix latency distribution.
3. **Hot sequential** — 50 PDF requests on the same medium doc (`cdc_ngs_validation.docx`, 670 KB), one at a time. The cleanest steady-state measurement.
4. **Hot concurrent c=10** — 50 PDF requests with max-in-flight = 10.
5. **Hot concurrent c=25** — same with c = 25, to surface queueing / contention behavior.
6. **Mixed format concurrent c=10** — 50 requests round-robining PDF / HTML / Markdown with c = 10. Approximates a real consumer that wants multiple representations.

Memory: throughout every phase, the harness polls the workerd process tree's RSS every 200 ms via `ps -o rss=`. Per-phase min / mean / max / p50 / p95 are reported, plus an overall run summary.

## Reproducing

Quick path:

```bash
# build everything once
pnpm install
pnpm --filter docx-to-pdf-wasm build

# JS worker
cd examples/cloudflare-worker
node scripts/copy-wasm.mjs
./node_modules/.bin/wrangler dev --port 8787 &
# … wait for "Ready", then in another shell:
node ../../research/harness/bench.mjs http://127.0.0.1:8787 /tmp/js.json
# kill the worker before benching the next one
pkill -9 workerd; pkill -f "wrangler dev"

# Rust worker
cd ../rust-worker
./node_modules/.bin/wrangler dev --port 8793 &
node ../../research/harness/bench.mjs http://127.0.0.1:8793 /tmp/rust.json
pkill -9 workerd; pkill -f "wrangler dev"
```

Each run takes ~12 seconds. Full output (console summary + JSON report) is deterministic in structure.

## Headline

Both workers hit ~30 PDF rps and ~50 mixed-format rps on a single workerd isolate. The Rust worker is consistently 15–20 % faster across every phase and uses 36–69 % less peak memory under burst load. Zero errors on the hot path and concurrent phases for both. See [`BENCHMARKS.md`](BENCHMARKS.md) for the breakdown.

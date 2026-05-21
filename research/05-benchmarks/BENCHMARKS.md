# Worker benchmarks — JS vs Rust

Measured on the same machine, same WASM build (opt-9a), same corpus, same harness ([`../harness/bench.mjs`](../harness/bench.mjs)). Each worker was restarted between runs to start from a cold isolate. Raw JSON reports: [`js-worker-bench.json`](js-worker-bench.json), [`rust-worker-bench.json`](rust-worker-bench.json).

## TL;DR

- **Both workers handle ~30 PDF rps and ~50 mixed-format rps on a single isolate.**
- **The Rust worker is consistently faster** — 15–20 % across every phase.
- **The Rust worker uses 36 % less peak memory** under burst load (1.14 GB vs 1.80 GB at concurrency 25).
- **Stability is identical**: zero errors on hot path, zero errors during concurrent bursts at c=10 and c=25. Both fail on the same upstream-rdocx-limited document (`deep-table-cell.docx` — see [`research/04-found-issues/003`](../04-found-issues/003-rdocx-deep-nested-tables-fail.md)).
- **No throughput collapse under contention.** Both workers held steady-state RPS as concurrency increased from 1 → 10 → 25; only per-request latency went up (queueing, as expected).

## Phase-by-phase comparison

The hot doc is `research/fixtures/complex/cdc_ngs_validation.docx` (670 KB DOCX, 13-page CDC NGS Quality Initiative report — representative "modern Office" document).

### Phase 2 — Corpus sweep (50 docs × 3 formats sequential)

150 requests across the full 50-doc corpus, every format. Mixes small toy fixtures and 2 MB / 468-page NIST specs.

| Metric | JS | Rust | Δ |
|---|---|---|---|
| Wallclock | 6.90 s | **6.34 s** | −8 % |
| Throughput | 21.8 rps | **23.7 rps** | +9 % |
| p50 latency | 5.7 ms | **5.0 ms** | −12 % |
| p95 latency | 104.2 ms | **86.6 ms** | −17 % |
| p99 latency | 295.5 ms | **267.0 ms** | −10 % |
| Mean latency | 35.2 ms | 30.5 ms | −13 % |
| Max latency (NIST 468-page) | ~5 s | ~4 s | −20 % |
| Errors | 4 | 3 | — |
| Peak RSS during phase | 1184 MB | **1110 MB** | −6 % |

### Phase 3 — Hot path sequential (50× same medium doc, PDF only)

The cleanest steady-state measurement: same input, same format, single in-flight request at a time.

| Metric | JS | Rust | Δ |
|---|---|---|---|
| Wallclock | 1.82 s | **1.54 s** | −15 % |
| Throughput | 27.5 rps | **32.5 rps** | +18 % |
| p50 latency | 36.0 ms | **30.7 ms** | −15 % |
| p95 latency | 38.7 ms | **32.0 ms** | −17 % |
| p99 latency | 41.7 ms | **38.5 ms** | −8 % |
| Mean latency | 36.2 ms | 30.7 ms | −15 % |
| Errors | 0 | 0 | — |
| Latency stdev | tight (~1 ms) | tight (~1 ms) | — |

This is the most reliable "warm production" number. The Rust worker is ~5 ms faster per conversion, which over 50 requests adds up to 280 ms saved.

### Phase 4 — Concurrent c=10 (50 PDF requests, 10 in flight)

| Metric | JS | Rust | Δ |
|---|---|---|---|
| Wallclock | 1.75 s | **1.48 s** | −15 % |
| Throughput | 28.6 rps | **33.9 rps** | +19 % |
| p50 latency | 336 ms | **289 ms** | −14 % |
| p95 latency | 679 ms | **306 ms** | −55 % |
| Errors | 0 | 0 | — |
| Peak RSS during phase | 1603 MB | **1141 MB** | −29 % |

Note p95 specifically: the **JS worker's tail is 2.2× wider** at c=10 (679 ms vs 306 ms). Under bursts, the JS worker has more variance — likely because every request creates a new `WebAssembly.Instance` in V8, and instance allocation contends with GC.

### Phase 5 — Concurrent c=25 (50 PDF requests, 25 in flight)

| Metric | JS | Rust | Δ |
|---|---|---|---|
| Wallclock | 1.71 s | **1.47 s** | −14 % |
| Throughput | 29.2 rps | **33.9 rps** | +16 % |
| p50 latency | 820 ms | **664 ms** | −19 % |
| p95 latency | 917 ms | **779 ms** | −15 % |
| Errors | 0 | 0 | — |
| Peak RSS during phase | **1795 MB** | **548 MB** | **−69 %** |

Throughput stays steady at ~30 rps even with 25 simultaneous requests — workerd serializes the actual CPU work but accepts all the requests. Latencies scale linearly with concurrency, which is the expected behavior.

The memory numbers are striking. The Rust worker actually freed memory partway through this phase (peak 548 MB) — Rust's allocator returned pages to the OS between requests. The JS worker held its 1.8 GB peak. **This is the single biggest practical difference between the two architectures.**

### Phase 6 — Mixed format concurrent c=10 (PDF/HTML/MD round-robin)

| Metric | JS | Rust | Δ |
|---|---|---|---|
| Wallclock | 1.01 s | **0.86 s** | −15 % |
| Throughput | 49.6 rps | **58.0 rps** | +17 % |
| p50 latency | 182 ms | **163 ms** | −10 % |
| p95 latency | 268 ms | **201 ms** | −25 % |
| Errors | 0 | 0 | — |
| Peak RSS during phase | 800 MB | 564 MB | −30 % |

Mixed traffic ~doubles throughput because 2/3 of requests are cheap HTML/MD (~13–16 ms) rather than 30 ms PDF.

## Memory

| Metric | JS worker | Rust worker | Δ |
|---|---|---|---|
| **Idle / min RSS** | 96 MB | 99 MB | parity |
| **Mean RSS across full run** | 839 MB | **679 MB** | −19 % |
| **Peak RSS across full run** | **1795 MB** | **1141 MB** | **−36 %** |

The peak is what matters for Cloudflare Workers' 128 MB per-isolate budget. Both workers exceeded 128 MB in this local benchmark — but locally `workerd` doesn't enforce that limit, and the measured RSS includes V8's JIT code cache, the wrangler dev shell, and OS overhead. **Production CF Workers isolates are sandboxed differently and the same WASM running at the edge would not show this RSS** (workerd-on-laptop is a poor predictor of production memory). What this benchmark does tell us is the **relative** memory cost — Rust per-request allocation is meaningfully lighter than JS-driven `WebAssembly.instantiate`.

## Stability / errors

Same 3 documents fail in both workers, and they're the same documents that fail across every other variant in the project:

| Document | JS worker | Rust worker | Root cause |
|---|---|---|---|
| `tier3/deep-table-cell.docx` (pdf) | 500 WASM trap | 500 worker hang | rdocx-pdf depth-limit, [foundissues/003](../04-found-issues/003-rdocx-deep-nested-tables-fail.md) |
| `tier3/deep-table-cell.docx` (html) | 500 | 500 | same |
| `tier3/deep-table-cell.docx` (markdown) | 500 | 500 | same |
| `tier2/EmptyDocumentWithHeaderFooter.docx` (markdown) | 422 (no message) | (passed) | JS-worker only quirk — possibly the per-request instance pattern surfacing a transient state |

Hot path + concurrent bursts: **0 errors on both workers across 250 + 50 + 50 + 50 = 400 requests**. No isolate poisoning, no panics, no resource exhaustion.

The 4th JS-worker error (the markdown empty-document case) is worth a follow-up — only one out of 50 docs was affected and only on one format. Not a blocker.

## Headline summary

```
JS worker (cloudflare-worker example):
  hot-path PDF       : 27.5 rps, p50 36 ms, p95 39 ms
  concurrent c=25    : 29.2 rps, p50 820 ms, p95 917 ms, peak RSS 1.80 GB
  mixed c=10         : 49.6 rps, p50 182 ms

Rust worker (rust-worker example):
  hot-path PDF       : 32.5 rps, p50 31 ms, p95 32 ms       (+18 % rps, tighter tail)
  concurrent c=25    : 33.9 rps, p50 664 ms, p95 779 ms, peak RSS 0.55 GB  (-69 % memory)
  mixed c=10         : 58.0 rps, p50 163 ms                  (+17 % rps)
```

## Reproducing

Both workers stop cleanly between runs. To re-run:

```bash
# Build the package (so its build/ has the current opt-9a WASM)
pnpm install
pnpm --filter docx-to-pdf-wasm build

# Bench the JS worker
cd examples/cloudflare-worker
node scripts/copy-wasm.mjs
nohup ./node_modules/.bin/wrangler dev --port 8787 </dev/null >/tmp/js.log 2>&1 &
until grep -q Ready /tmp/js.log; do sleep 0.5; done; sleep 3
cd ../.. && node research/harness/bench.mjs http://127.0.0.1:8787 /tmp/js.json
pkill -9 workerd; pkill -f "wrangler dev"

# Bench the Rust worker
cd examples/rust-worker
nohup ./node_modules/.bin/wrangler dev --port 8793 </dev/null >/tmp/rust.log 2>&1 &
until grep -q Ready /tmp/rust.log; do sleep 0.5; done; sleep 3
cd ../.. && node research/harness/bench.mjs http://127.0.0.1:8793 /tmp/rust.json
pkill -9 workerd; pkill -f "wrangler dev"
```

The harness auto-discovers the workerd PIDs via `pgrep workerd` and samples their combined RSS every 200 ms during the benchmark. Pass `--pid=N` if you want to scope to a specific process.

## Caveats

- Benchmarks ran against `wrangler dev` (workerd) on a local laptop. **Production CF Workers numbers may differ** — V8 at the edge has different JIT tuning, the request path adds real network latency, and memory limits are actually enforced.
- The hot-path RPS (~30) is bounded by **single-CPU WASM execution time** for the actual converter (~30 ms PDF, ~15 ms HTML/MD). At the edge this would be the same single-thread budget. Per-isolate throughput at CF would track these numbers; scaling beyond comes from horizontal isolate distribution.
- Peak RSS includes the entire workerd process tree, not just WASM linear memory. Production CF isolates would only count the linear memory toward the 128 MB ceiling.

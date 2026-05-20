# 01 — Three approaches, evaluated in parallel

Before committing to a single Rust stack, we built three different proofs-of-concept end-to-end and scored them against the same toy corpus. Each PoC is a self-contained Cargo crate that produces both a native CLI and a `wasm32-unknown-unknown` cdylib.

| Approach | Stack | Bundle (gz) | T1 / T2 / T3 recall | Fits Workers (≤ 10 MiB)? |
|---|---|---|---|---|
| **A — custom** | `docx-rs` + `krilla` + hand-rolled minimal layout (paragraphs only) | **0.89 MiB** | 1.00 / 0.79* / 0.62* | ✓ |
| **B — Typst** | `docx-rs` → Typst markup → `typst-pdf` (via `office2pdf`) | 14.06 MiB | 1.00 / 0.99 / 0.91 | ✗ over by 4 MiB |
| **C — rdocx** | The [`rdocx`](https://github.com/tensorbee/rdocx) library (parser + layout + PDF) | 4.04 MiB | 0.97 / 0.94 / 0.77 | ✓ |

\* A's T2/T3 scores are low *by design* — it intentionally skips tables, images, headers, footers, and footnotes. With that scope expansion it'd land closer to T2 ≈ 0.95.

## What each one taught us

### Approach A — [`approach-a-custom/`](approach-a-custom/)
A *floor* on size. 0.89 MiB gz proves you can ship a working DOCX→PDF WASM under 1 MiB if you're willing to skip everything but plain text. krilla writes clean ToUnicode CMaps; A's WASM doesn't suffer the cmap-drift bug that later turned up in C's `rdocx-pdf`. Useful as a comparison data point when we were diagnosing that bug.

### Approach B — [`approach-b-typst/`](approach-b-typst/)
A *ceiling* on quality. Typst's layout + PDF writer is excellent — it cleanly handles cmap, font fallback, and complex tables — but the bundle is 14 MiB gzipped, well over Cloudflare Workers' 10 MiB ceiling. We did not ship B, but it stayed in our pocket as the reference quality bar. Approach B's per-document scorecard was the gold standard we measured everything else against, and when real-world testing revealed C's cmap bug, B's data confirmed the input documents were fine — the problem was downstream.

### Approach C — [`approach-c-rdocx/`](approach-c-rdocx/)
The *winner* on Workers. 4 MiB gz with strong T1/T2 fidelity, off-the-shelf integration (~30 minutes to wire up), and the only single library that gave us a complete parse + layout + PDF pipeline. Subsequent optimization work in [`../02-optimizations/`](../02-optimizations/) is built entirely on C.

## Decision

Picked **C**. The biggest binding constraint was Workers' 10 MiB bundle ceiling; B failed it outright and A would have required months of additional layout work to reach C's recall.

The original cross-approach scorecard is at [`../results/comparison.md`](../results/comparison.md).

## Folder layout (per approach)

```
approach-X-…/
├── PLAN.md             # approach-specific scope and non-goals
├── RESULTS.md          # what built, what didn't, scorecard, recommendation
├── converter/          # Rust crate (lib + native bin + cdylib)
│   ├── Cargo.toml
│   └── src/
└── wasm-runner.mjs     # Node.js driver for the harness (when present)
```

Each `RESULTS.md` is the single most useful entry point per approach.

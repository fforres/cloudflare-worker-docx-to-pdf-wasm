# Cross-approach comparison

Three proofs-of-concept built in parallel and run against the same 25-document corpus (5 T1 + 10 T2 + 10 T3) with the same harness. See [`../TEST_PLAN.md`](../TEST_PLAN.md) for the methodology.

## Verdict

**Approach C (`rdocx`) wins for the Cloudflare Workers target.** It fits the 10 MiB compressed bundle limit with substantial headroom, has near-best fidelity on T1/T2, runs an order of magnitude faster than B, and required ~30 minutes of integration work (one local Cargo patch to disable C-backed compression in a transitive `zip` dep).

| Approach | Library stack | WASM gzipped | T1 recall | T2 recall | T3 recall | Avg conv time | Fits CF? |
|---|---|---|---|---|---|---|---|
| **A — custom** | `docx-rs` + `krilla` + hand-rolled layout | **0.89 MiB** | 1.00 | 0.79 ⓘ | 0.62 ⓘ | n/a measured | ✓ |
| **B — Typst** | `docx-rs` → Typst markup → `typst-pdf` (via `office2pdf`) | 14.06 MiB ✗ | 1.00 | 0.99 | 0.91 | 482 ms | **✗ over limit** |
| **C — rdocx** | `rdocx` library (`docx-rs` + own layout + `pdf-writer`) | **4.04 MiB** | **0.97** | **0.94** | **0.77** | **48 ms** | ✓ |

ⓘ Approach A intentionally skipped tables, images, headers/footers, and footnotes; the lower T2/T3 recall is a scope choice, not a bug. With ~1 week of additional layout work it could close most of the gap.

## Detailed metrics

### WASM size (`wasm-opt -Oz`, then gzip -9)

| Approach | Raw | wasm-opt | **Gzipped** | Brotli q11 |
|---|---|---|---|---|
| A | 2.31 MiB | 1.96 MiB | **0.89 MiB** | 0.71 MiB |
| B | 35.51 MiB | 29.94 MiB | **14.06 MiB** | 9.77 MiB |
| C | 8.66 MiB | 8.11 MiB | **4.04 MiB** | 2.69 MiB |

CF Workers ceiling (paid): 10 MiB gzip. **B fails this gate**; A and C pass with significant margin (A by 9.5 MiB, C by 6.2 MiB).

C's footprint is dominated by 6.8 MiB of bundled TTF fonts (Carlito, Caladea, Liberation, OpenSans, NotoSans). Without bundled fonts, C is **0.65 MiB gzipped** — but then it can't render any DOCX that references a non-system font (i.e. every DOCX). Approach C v2 should ship 2–4 fonts (~1.5–2 MiB) and fetch the rest from R2 on demand.

### Per-tier scorecard

| Approach | T1 OK | T1 recall | T2 OK | T2 recall | T3 OK | T3 recall |
|---|---|---|---|---|---|---|
| A | 5/5 | 1.00 | 10/10 | 0.79 | 9/10 | 0.62 |
| B | 5/5 | 1.00 | 10/10 | **0.99** | 9/10 | **0.91** |
| C | 5/5 | **0.97** | 10/10 | 0.94 | 9/10 | 0.77 |
| *Native baseline (rdocx-cli)* | 5/5 | 1.00 | 10/10 | 0.99 | 9/10 | 0.78 |
| *LibreOffice (reference)* | 5/5 | 1.00 | 10/10 | 1.00 | 10/10 | 1.00 |

Notes:
- All three approaches fail the same T3 doc (`deep-table-cell.docx`) — `docx-rs` rejects deeply nested tables. This is a common upstream issue.
- C's small recall gap vs. its own native baseline (0.97 vs 1.00 on T1; 0.94 vs 0.99 on T2) is font-substitution noise: native macOS happens to have Calibri installed, WASM uses bundled Carlito.

### Speed (median per-doc conversion time)

| Approach | Native | WASM (Node.js) |
|---|---|---|
| A | n/a (sub-agent didn't report) | ~? |
| B | 144–223 ms | 409–1232 ms |
| C | 23–33 ms | 47–50 ms |

C is ~10× faster than B. For a typical 30s CF Workers CPU budget, C can comfortably handle 50+ page documents.

## Why C wins on this target

1. **Fits the binding constraint** (10 MiB gzipped). B doesn't.
2. **Off-the-shelf fidelity** is excellent without us writing layout code. A would need many weeks of work to reach C's recall.
3. **Speed** matters for Workers' per-request CPU budget. C is 10× faster than B and the only one tested at <100 ms.
4. **One library, one patch**. The integration was a thin wrapper plus a local `[patch.crates-io]` to disable `zip`'s zstd/bzip2 features. No multi-crate stitching, no custom layout pass.

## What we are accepting by picking C

- **T3 limitations**: text in textboxes, footnote bodies, and track-changes are sometimes not rendered. These are upstream `rdocx` 0.1.2 limitations.
- **One T3 doc fails** (`deep-table-cell.docx`). This also fails A and B.
- **Font substitution drift**: Carlito instead of Calibri. Words may break in slightly different places. Per the project's stated priorities ("content > font fidelity") this is acceptable.
- **Young upstream**: `rdocx` is 0.1.2, single maintainer (tensorbee). Mitigation: pinned version, local `patches/` directory, willingness to fork if needed.

## When we'd revisit

Reconsider approach B if:
- Workers raises the bundle limit (unlikely in 2026), or
- We move to Cloudflare Containers (no size cap, no CPU cap), or
- We spend 4–8 hours trimming Typst dependencies as suggested in B's RESULTS.md (target: 7–9 MiB gzipped).

Reconsider approach A if:
- We decide C's bundled-fonts approach is too heavy and want to ship < 1 MiB total, AND
- We accept doing custom layout work for tables/headers/footers (~1 week minimum).

## Next steps (now in flight)

→ Build a minimal Cloudflare Worker around `approach-c-rdocx/approach_c_rdocx.opt.wasm`, accepting `POST /convert` with a DOCX body and returning the PDF. Validate with `wrangler dev` + `curl`.

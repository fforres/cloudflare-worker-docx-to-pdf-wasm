# Approach C — optimization scorecard

All four variants built natively + to `wasm32-unknown-unknown`, scored against the shared corpus, and tested end-to-end via a `wrangler dev` worker.

## Bottom line

**Ship opt-3 (font subsetting).** It is the only variant with **zero fidelity regression** vs the baseline and the smallest single-variant deploy bundle. The runner-up is opt-2 (zero fonts in WASM), which has a smaller WASM-only artifact if you're willing to wire R2 and accept a minor fidelity drop.

## Scorecard

| Variant | Strategy | Raw WASM | gz WASM | **Worker deploy (gz)** | T1 recall | T2 recall | T3 recall | Conv ms (avg) |
|---|---|---|---|---|---|---|---|---|
| baseline | 22 full fonts | 8.66 MiB | 4.04 MiB | 4.04 MiB | 0.97 | 0.94 | 0.77 | 48 |
| **opt-1 trim** | 4 full fonts | 3.89 MiB | 1.57 MiB | 1.63 MiB | 0.91 | 0.91 | 0.77 | 104 |
| **opt-2 R2** | 0 fonts in WASM, runtime-loaded | 1.91 MiB | **0.65 MiB** | 1.60 MiB (fonts as data) / **0.65 MiB if R2** | 0.91 | 0.91 | 0.77 | 62 |
| **opt-3 subset** ✅ | 20 fonts subset to Latin | 3.09 MiB | 1.31 MiB | **1.34 MiB** | **0.97** | **0.94** | 0.77 | 46 |
| **opt-4 strip** | 4 fonts + dep prune | 3.41 MiB | 1.56 MiB | 1.62 MiB | 0.91 | 0.91 | 0.77 | 110 |

Per-tier `Files OK` is identical across all variants (5/5 T1, 10/10 T2, 9/10 T3 — same `deep-table-cell.docx` failure as the baseline). All variants pass every gate.

## Why opt-3 wins

1. **No recall regression.** opt-1, opt-2, opt-4 all lose ~6 % T1 / 3 % T2 recall because they ship only 4 fonts. The single doc that drives most of the drop (`lists_level_override.docx`) references a font outside the 4-font subset; with 20 subset fonts available, opt-3 still has it.
2. **Smallest single-variant deploy bundle** (1.34 MiB gz vs opt-1's 1.63 MiB, opt-2's 1.60 MiB, opt-4's 1.62 MiB) — because the subset fonts are 19 % of original size on average, so even shipping 20 of them is < shipping 4 full ones.
3. **Lowest conversion time** (46 ms) — no runtime font-buffer construction (opt-2) or font-lookup miss penalties.
4. **Only operational cost**: requires Python + fontTools at build time. The build dep is well isolated in `build.rs`.

## Why each runner-up loses

- **opt-1 trim**: Same code as opt-3 with fewer fonts; slightly bigger bundle (full fonts > subset fonts) AND slightly worse recall (more font misses).
- **opt-2 R2**: Smallest **WASM artifact** by far (0.65 MiB gz), but the same fonts have to be deployed *somewhere*. As `wrangler dev` data assets the deploy bundle is 1.60 MiB (parity with opt-1). The only real win is when fonts move to R2 (separate quota), at which point the worker bundle is 0.65 MiB. Adds operational complexity and ~15 ms per-request overhead.
- **opt-4 code strip**: Real raw-WASM savings (1.55 vs 1.88 MiB no-fonts) but gzip compresses most of the dropped code away — the on-the-wire win is ~5 KB. The sub-agent's own verdict: "ship stacked with opt-1 or opt-2, not standalone."

## Stacking matrix (estimated)

opt-4 is orthogonal to the font variants and stacks cleanly. The agents didn't actually build stacked combos, but their numbers project to:

| Combination | Estimated gz | Notes |
|---|---|---|
| opt-3 | 1.34 MiB | **Current recommendation** |
| opt-3 + opt-4 | ~1.27 MiB | Marginal additional savings; doubles patch maintenance burden |
| opt-2 + opt-4 (with R2) | ~0.58 MiB | Smallest possible; biggest operational complexity |
| opt-1 + opt-4 | ~1.56 MiB | Worst of both worlds — bigger than opt-3, same fidelity cost |

## End-to-end worker tests

Every variant ran `wrangler dev` on its own port, served `POST /convert` with real DOCX fixtures, and returned valid PDFs. Each also passed the trap-recovery test (bad input → 500, immediately followed by a good 200). The per-request-instantiation pattern from the parent worker (`worker/src/worker.js`) was reused unchanged.

Ports: opt-1 → 8788, opt-2 → 8789, opt-3 → 8790, opt-4 → 8791.

## What to do next

1. **Replace `worker/src/converter.wasm` in the top-level worker with opt-3's WASM** (and add the build.rs Python dep documentation). This is the smallest-disruption ship.
2. *(Optional)* Build the **opt-3 + opt-4 stack** if the ~70 KB additional gz savings matters for your specific deploy target. Likely not worth the patch maintenance.
3. *(Optional)* Move to **opt-2 + R2** if you have many isolates being created (e.g. very low traffic per location) and want to amortize font bytes across them via R2 cache.
4. *(Out of scope for these PoCs)* — improve T3 fidelity (textbox text, footnotes, track changes). All variants share the same T3 recall = 0.77, capped by upstream rdocx limitations.

## File locations

- `opt-1-trim-fonts/{PLAN.md, RESULTS.md, converter/, worker/, wasm-runner.mjs}`
- `opt-2-r2-fonts/{PLAN.md, RESULTS.md, converter/, worker/, wasm-runner.mjs}`
- `opt-3-font-subsetting/{PLAN.md, RESULTS.md, converter/, worker/, wasm-runner.mjs}` ← **winner**
- `opt-4-code-stripping/{PLAN.md, RESULTS.md, converter/, worker/}`
- `../results/opt-*/summary.md` per-document scorecards

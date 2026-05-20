# 02 — Optimizations on approach C

Eight optimization variants built on top of [`../01-approaches/approach-c-rdocx/`](../01-approaches/approach-c-rdocx/). The first four (opt-1 to opt-4) target bundle size; opt-5 onward addresses fidelity issues uncovered by real-world testing.

| Variant | Headline change | Bundle (gz) | Real-world recall | Status |
|---|---|---|---|---|
| C-baseline | rdocx, all 22 bundled fonts | 4.04 MiB | 0.71 (cmap drift) | superseded |
| [`opt-1-trim-fonts`](opt-1-trim-fonts/) | Ship 4 fonts instead of 22 | 1.57 MiB | n/a (not stress-tested) | exploratory |
| [`opt-2-r2-fonts`](opt-2-r2-fonts/) | Move all fonts to R2; zero in WASM | 0.65 MiB (WASM) / 1.60 MiB (deploy) | n/a | exploratory |
| [`opt-3-font-subsetting`](opt-3-font-subsetting/) | Build-time pyftsubset of all 20 bundled fonts | 1.31 MiB | 0.71 (cmap drift) | superseded |
| [`opt-4-code-stripping`](opt-4-code-stripping/) | Drop rdocx-html, regex, unused codecs | 1.56 MiB (with 4 fonts) | n/a | stacks orthogonally |
| **[`opt-5-complex-corpus`](opt-5-complex-corpus/)** | Bypass the Carlito CMap bug via Liberation aliases | 2.82 MiB | **0.94** | superseded by opt-6 |
| [`opt-6-subset-liberation`](opt-6-subset-liberation/) | opt-5 + opt-3's pyftsubset stacked | 0.98 MiB | 0.94 | superseded by opt-8 |
| [`opt-7-textbox-preprocessor`](opt-7-textbox-preprocessor/) | DOCX-XML preprocessor that lifts `<w:txbxContent>` into `<w:body>` | (built on opt-5) | (one doc 0.00 → 0.93) | folded into opt-8 |
| **[`opt-8-textbox-preprocessor-subset`](opt-8-textbox-preprocessor-subset/)** | opt-6 + opt-7 stacked | **1.03 MiB** | **0.98** | **shipped** |

[`RESULTS.md`](RESULTS.md) is the **original size-optimization scorecard** comparing opt-1 through opt-4 (before real-world testing exposed the cmap bug). [`PLAN.md`](PLAN.md) is the master plan for that first optimization round.

## Reading order

1. [`PLAN.md`](PLAN.md) — opt-1…opt-4 master plan
2. [`RESULTS.md`](RESULTS.md) — opt-1…opt-4 scorecard. opt-3 wins on toy corpus.
3. [`../03-real-world-testing/COMPARISON.md`](../03-real-world-testing/COMPARISON.md) — the *plot twist*: opt-3 (and everything else built on rdocx + bundled-fonts) fails on 32 % of real-world docs.
4. [`opt-5-complex-corpus/RESULTS.md`](opt-5-complex-corpus/RESULTS.md) — the Carlito CMap bug isolated and fixed.
5. [`opt-6-subset-liberation/RESULTS.md`](opt-6-subset-liberation/RESULTS.md) — stack the opt-3 subsetting pipeline on opt-5's fix.
6. [`opt-7-textbox-preprocessor/RESULTS.md`](opt-7-textbox-preprocessor/RESULTS.md) — a *separate* upstream gap (`<w:txbxContent>` not extracted) and a small Rust preprocessor that works around it.
7. [`opt-8-textbox-preprocessor-subset/RESULTS.md`](opt-8-textbox-preprocessor-subset/RESULTS.md) — opt-6 + opt-7 in one variant. This is what ships.

## Folder layout (per variant)

```
opt-N-…/
├── PLAN.md             # the change being tried + expected outcome
├── RESULTS.md          # measured numbers
├── converter/
│   ├── Cargo.toml      # often includes a [patch.crates-io] override
│   ├── build.rs        # opt-3, opt-6, opt-8: pyftsubset font pipeline
│   ├── src/
│   ├── fonts/ or fonts-source/   # bundled OFL TTFs (input)
│   └── patches/        # vendored sub-crates with adjusted manifests
├── wasm-runner.mjs     # Node driver for the harness
└── worker/             # standalone CF Worker for end-to-end testing on a unique port
    ├── package.json
    ├── wrangler.toml
    └── src/
```

Each variant's `worker/` is fully self-contained so we could run wrangler-dev tests on distinct ports without conflicts (opt-1 → 8788, opt-2 → 8789, …, opt-8 → 8787 once it became the top-level worker).

## Patterns worth borrowing

- **`[patch.crates-io]` to fix transitive deps cheaply.** opt-1 onward includes `patches/rdocx-opc/Cargo.toml` with `zip = { default-features = false, features = ["deflate"] }` to remove the C-backed zstd/bzip2 deps that block wasm32 cross-compilation. The whole patch is a 3-line manifest tweak.
- **`build.rs` invoking `pyftsubset`.** opt-3 and opt-6/opt-8 ship a build.rs that auto-discovers `pyftsubset` (path / homebrew / `~/Library/Python/*/bin`) and applies a fixed Unicode coverage at compile time. ~88 % per-font byte reduction with kerning/ligature features preserved (`--layout-features='*'`).
- **Alias-based font fallback to bypass an upstream bug.** opt-5+ bundles only the Liberation family and registers Carlito-targeted names (Calibri, Cambria, Arial, Times, …) under those faces via `Document::to_pdf_with_fonts`. User-supplied fonts win lookup priority, so the buggy Carlito codepath never executes.
- **Pre-processing the input document.** opt-7's preprocessor mutates `word/document.xml` to surface text-box paragraphs into the main body before parsing — a tiny defensive XML byte-scanner that's a no-op on documents without textboxes. Useful template for working around any "rdocx doesn't walk this subtree" gap.

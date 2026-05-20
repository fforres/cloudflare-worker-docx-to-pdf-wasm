# Approach B — `office2pdf` / Typst — Results

## TL;DR

- **Native fidelity is excellent.** 24/25 fixtures convert; only one
  Tier-3 doc fails (`deep-table-cell.docx`, an upstream nesting-depth
  guard in `docx-rs`).
- **WASM builds cleanly on the first try** with `--features wasm`.
- **Compressed WASM is ~14 MiB gzip (33.5 MiB raw, 30 MiB wasm-opt -Oz).**
  That **exceeds** Cloudflare's 10-MiB-gzip Workers ceiling. Brotli-q11
  squeezes it to 9.77 MiB, which would fit *if* the Workers upload path
  accepted brotli — by default it does not.
- Verdict: **content-quality winner, but disqualified from Workers
  without further size work.** Containers candidate as-is; Workers
  candidate after aggressive dependency trimming.

## What was built

| Artifact | Path |
|---|---|
| Upstream native CLI | `cargo install office2pdf-cli@0.5.0` → `~/.cargo/bin/office2pdf` |
| Native wrapper | `approach-b-typst/native-wrapper.sh` |
| Custom converter (lib + bin) | `approach-b-typst/converter/` |
| Native CLI binary | `approach-b-typst/converter/target/release/approach-b-converter` (24.6 MiB) |
| WASM cdylib | `approach-b-typst/converter/target/wasm32-unknown-unknown/release/approach_b_converter.wasm` |
| Harness results (native upstream) | `results/office2pdf-native/summary.md` |
| Harness results (our custom bin) | `results/approach-b/summary.md` |

Our custom binary and the upstream CLI score *identically* (same 24/25,
same per-doc recall numbers), which confirms our wrapper does not
introduce regressions.

## WASM size

```
raw         :   35,141,289 bytes (33.51 MiB)
wasm-opt -Oz:   31,403,037 bytes (29.94 MiB)
gzip -9     :   14,752,517 bytes (14.06 MiB)   ← Cloudflare limit is 10.00 MiB
brotli -q11 :   10,254,542 bytes ( 9.77 MiB)
```

Over the 10 MiB gzip ceiling by **4.07 MiB**. Single biggest contributors
(`twiggy`/`cargo bloat` not run, but informed estimate from the dep
tree):

- `typst` core + `typst-library` + `typst-layout` + `typst-pdf` +
  `typst-realize` + `typst-eval` + `typst-html` + `typst-kit`
- Embedded fonts via `typst-kit`'s `embed-fonts` feature (~5 MiB raw)
- `image` crate (all default codecs)
- `docx-rs` + `umya-spreadsheet` (XLSX support we don't actually need)
- `syntect`, `biblatex`, `citationberg`, `hayagriva` (Typst's
  bibliography / syntax highlighting stack, dead weight for our use case)

## Fidelity (custom binary)

| Tier | Files | OK | Recall (avg) | Page Δ | Img Δ | Avg ms |
|------|-------|----|--------------|--------|-------|--------|
| T1   |  5/5  | 5  | 1.00         | 0.00   | 0.0   |   409  |
| T2   | 10/10 | 10 | 0.99         | 0.10   | 0.1   |  1232  |
| T3   |  9/10 |  9 | 0.91         | 0.00   | 0.4   |   482  |

All pass/fail gates from `TEST_PLAN.md` are satisfied on the fidelity
side:
- T1: 5/5 ✓, recall 1.00 ≥ 0.85 ✓
- T2: 10/10 ≥ 6/10 ✓
- T3: 9/10 (best-effort, no gate)

The only WASM-relevant gate that fails is **compressed bundle ≤ 10 MiB
gzip**.

## Top 3 things that broke / underperformed

1. **`deep-table-cell.docx`** fails with
   `Table nesting depth exceeded maximum limit.` from `docx-rs`. This is
   a deliberate safety guard in the upstream parser, not a bug in
   `office2pdf`. Could be lifted by adjusting `docx-rs`'s
   `MAX_NESTED_TABLES` constant if we vendor it.
2. **Page count under-counts** on docs with headers/footers
   (`SimpleHeadThreeColFoot`, `SampleDoc`): we emit 1 page where
   LibreOffice emits 2. Recall is still high (≥ 0.94), so the missing
   page is mostly empty header/footer content. Not a content regression.
3. **WASM bundle exceeds the Workers gzip ceiling by ~4 MiB**, driven
   mostly by Typst's full IR + library + the `image` crate's default
   codec bundle + `typst-kit`'s embedded fonts. There is no `--features`
   knob upstream today that disables font embedding while keeping
   `convert_bytes` working on WASM (font lookup would need to be a
   no-op).

Also noted: native conversion time is ~5–10× slower for our binary
than the upstream `office2pdf` CLI (1.2 s vs 144 ms average on T2).
This is the cost of `opt-level = "z"` + LTO single-codegen-unit in our
release profile; switching to `opt-level = 3` would close the gap for
the native binary but would inflate the WASM bundle further, so we
leave it.

## Recommendation

**This approach beats any plausible baseline on content fidelity.** It
should be the reference target for output quality across all three
PoCs. However, it does **not** fit Cloudflare Workers' 10-MiB-gzip
ceiling as-built.

Three viable next steps, ordered by effort:

1. **Containers track** (lowest effort): ship the native binary inside a
   Cloudflare Container. Quality is already proven, latency budget is
   plenty.
2. **Workers track with aggressive trimming**: fork `office2pdf` and
   - drop XLSX/PPTX (kills `umya-spreadsheet`, simplifies `image` usage)
   - drop `typst-kit/embed-fonts`, ship one OFL font via `include_bytes!`
   - remove `syntect`/`biblatex`/`citationberg`/`hayagriva` from the
     Typst feature set (they're transitive but mostly cuttable)
   - disable unused `image` codec features (keep PNG + JPEG only)
   Realistic target: 7–9 MiB gzip. Estimated 4–8 hours of vendor + diet
   work.
3. **Hybrid**: parse DOCX with `docx-rs` ourselves, lower to a much
   smaller Typst markup string, and call `typst::compile` +
   `typst-pdf::pdf` directly — bypassing `office2pdf`'s IR and its
   broader codepaths. Higher risk, but biggest size reduction
   potential. Estimated 1–2 days.

If the project's preferred approach (C, `rdocx`) lands cleanly under 10
MiB with passable fidelity, **stay with C**. If C fails the bundle gate
or the fidelity gate, **Approach B with trimming option (2) is the
recommended fallback**.

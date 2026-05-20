# 03 — Real-world stress testing

After optimizing approach C four ways ([`../02-optimizations/`](../02-optimizations/)), we stress-tested it on 25 real public DOCX files: federal government reports (NIST, NASA, EPA, CDC, HHS, GAO, SEC), international body publications (UN, WHO, WIPO), and US-university thesis templates. Sizes ranged from 17 KB / 1 page to 2 MB / 468 pages.

## Files in this folder

- [`RESULTS.md`](RESULTS.md) — the original stress-test report on opt-3 (font-subsetting variant). 25/25 conversions succeeded, but average text recall dropped from 0.97 on the toy corpus to **0.71** on real-world documents.
- [`COMPARISON.md`](COMPARISON.md) — head-to-head of approach A, approach B, C-baseline, and opt-3 on the same 25 docs. This is the file that uncovered the real bug.

The DOCX files themselves live at [`../fixtures/complex/`](../fixtures/complex/) (downloaded fresh from public sources; license notes in [`RESULTS.md`](RESULTS.md#corpus)).

## What real-world testing revealed

**Synthetic toy fixtures lied.** Our T1/T2/T3 scorecard (Pandoc + Apache POI + python-docx test files) consistently showed 0.94–0.97 recall and we thought we were done. The 25 real-world documents told a different story:

| Variant | Toy avg recall | Real-world recall |
|---|---|---|
| opt-3 (font-subsetting) | 0.94 | **0.71** |
| Approach B (Typst, native) | 0.99 | 0.98 |
| C-baseline native (full fonts) | 0.94 | 0.94 |
| **C-baseline WASM (full fonts)** | 0.94 | **0.71** |

The hypothesis "subsetting fonts breaks the ToUnicode CMap" turned out to be wrong. The smoking gun was C-baseline WASM scoring **identically** to opt-3 on real-world docs despite shipping full TTFs without any subsetting. Same fonts, same WASM target, same code path, same scrambled text on the same 8 documents.

The actual bug is in [`rdocx-pdf`](https://github.com/tensorbee/rdocx) version 0.1.2: when it renders text using Carlito (one of the fonts shipped in `rdocx-layout`'s `bundled-fonts` feature), it emits a wrong ToUnicode CMap. On native macOS, where fontdb's `load_system_fonts()` finds Tahoma/Helvetica/Calibri installed by LibreOffice and they win the font lookup race, Carlito never gets used and the bug never fires. In WASM, no system fonts exist, Carlito always wins, the bug always fires.

Full root-cause writeup: [`../04-found-issues/001-rdocx-pdf-carlito-tounicode-cmap.md`](../04-found-issues/001-rdocx-pdf-carlito-tounicode-cmap.md).

## What we did with the finding

This investigation drove the design of opt-5: bundle only Liberation Sans/Serif/Mono and register them as aliases for Carlito-targeted requests, sidestepping the buggy code path entirely. Real-world recall jumped from 0.71 to 0.94. See [`../02-optimizations/opt-5-complex-corpus/RESULTS.md`](../02-optimizations/opt-5-complex-corpus/RESULTS.md).

A second pass (opt-7) discovered another upstream gap — `<w:txbxContent>` paragraphs never reach `rdocx-oxml`'s body walker, so magazine-layout documents like UN policy briefs produced empty PDFs. We worked around it with a Rust preprocessor that lifts textbox content into `<w:body>` before parsing. See [`../02-optimizations/opt-7-textbox-preprocessor/RESULTS.md`](../02-optimizations/opt-7-textbox-preprocessor/RESULTS.md).

Final build (opt-8) combines opt-5's Carlito workaround, opt-3's pyftsubset pipeline, and opt-7's textbox preprocessor:
- 1.03 MiB gzipped (–75 % from original C-baseline)
- 0.98 avg recall on real-world corpus
- All 25 docs convert cleanly

## Lessons

1. **Toy fixtures don't reflect production traffic.** Pandoc and Apache POI test files happened to avoid the cmap-drift trigger because they don't use Calibri Light / Cambria. Modern Microsoft Office documents do, by default. Our toy corpus was systematically biased toward "documents that pre-2014 OOXML test suites happened to produce."
2. **Cross-platform parity is not assured.** rdocx native worked perfectly on macOS dev boxes. The same Rust source compiled to WASM produced different output. The variable wasn't the code — it was which fonts were available at runtime. Always test on the deploy target with the deploy target's runtime environment.
3. **The clean-room repro paid off.** Sub-agent debug attempts initially blamed the wasm32 codegen ("it's a Rust panic", "it's an LTO miscompilation"). A clean-room test with rdocx alone (no opt-N wrapper) and `bundled-fonts` enabled reproduced the bug *on native macOS* — proving the bug was in rdocx, not in our build, and pointing at the actual culprit (Carlito).

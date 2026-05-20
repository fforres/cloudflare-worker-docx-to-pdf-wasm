# docx-to-pdf-wasm

> [!NOTE]
> This project is a **research prototype**.
> Here be dragons and stuff. Done through a curated Auto-research approach
> The API is not stable, the package is not published, and the README is somwehat comprehensive.
> See [`research/`](research/) for the full story.


Convert Microsoft Word documents (`.docx`) to PDF inside a WebAssembly module.

Runs anywhere modern WebAssembly does — **Cloudflare Workers, Node.js, Bun, Deno, browsers** — with **no JavaScript-runtime dependencies**, **no LibreOffice**, **no headless browser**.

```text
.docx bytes  ─►  docx-to-pdf-wasm  ─►  .pdf bytes
                   ~2.3 MiB raw
                   ~1.0 MiB gzipped
                   ~50–200 ms per typical document
```

## Status

| Metric | Value |
|---|---|
| Compressed WASM bundle | **1.03 MiB gzipped** (8.97 MiB headroom under Cloudflare Workers' 10 MiB ceiling) |
| Toy-corpus recall (T1 / T2 / T3) | 1.00 / 0.99 / 0.89 |
| Real-world-corpus recall (25 docs) | **0.98 avg** |
| Conversion latency | ~50 ms for typical 1-page docs; 3.7 s for a 468-page NIST spec |
| Crashes on the 25-doc real-world corpus | 0 |

See [`research/`](research/) for how we got these numbers — three competing approaches, eight optimization variants, two corpora, and seven upstream issues documented along the way.

## Layout

```
packages/
  docx-to-pdf-wasm/             # The published package. pnpm-installable.
                                 # Runtime-agnostic — exports a `convert(module, bytes)` function.
                                 # Ships the compiled WASM in build/.
examples/
  cloudflare-worker/             # Minimal Cloudflare Worker consuming the package.
                                 # POST /convert with a .docx body → PDF.
research/                        # Everything we built and measured to get here.
  README.md                      # Index of the journey.
  01-approaches/                 # Approach A (custom), B (Typst), C (rdocx) side-by-side.
  02-optimizations/              # opt-1 … opt-8 — size + correctness variants on C.
                                 # opt-8 is what the package ships.
  03-real-world-testing/         # Stress test on 25 real DOCX files; comparison vs all approaches.
  04-found-issues/               # Outstanding upstream bugs + workarounds we shipped.
  fixtures/                      # Toy + real-world test corpus.
  reference-pdfs/                # Ground-truth PDFs from LibreOffice headless.
  harness/                       # Python scoring scripts.
  results/                       # Per-variant scorecards.
  PLAN.md, TEST_PLAN.md          # Top-level planning docs.
LICENSE
package.json                     # pnpm workspace root.
pnpm-workspace.yaml
```

## Quick start

### Use the package

```bash
pnpm add docx-to-pdf-wasm
```

```ts
import { convert } from "docx-to-pdf-wasm";

// Get the compiled WebAssembly.Module however your runtime allows:
// - Cloudflare Workers: import wasmModule from "docx-to-pdf-wasm/wasm";
// - Node.js: const wasmModule = await WebAssembly.compile(await fs.readFile(...));
// - Browser: const wasmModule = await WebAssembly.compileStreaming(fetch(...));

const docxBytes: Uint8Array = /* … */;
const pdfBytes: Uint8Array = await convert(wasmModule, docxBytes);
```

See [`packages/docx-to-pdf-wasm/README.md`](packages/docx-to-pdf-wasm/README.md) for runtime-specific recipes.

### Run the example Worker locally

```bash
pnpm install
pnpm --filter cloudflare-worker-example dev
# In another shell:
curl -X POST --data-binary @some.docx -o out.pdf http://127.0.0.1:8787/convert
```

## What this does well

- **Robust** — zero crashes across 25 real-world public documents (NIST, NASA, EPA, CDC, UN, WHO, university theses) spanning 1 page to 468 pages.
- **Fast** — 50 ms for typical 1-page docs; 4 s for a 468-page federal security spec. Well inside Cloudflare Workers' 30 s default CPU budget.
- **Content-faithful** — 0.98 avg text-token recall vs LibreOffice on the real-world corpus. Searchable, copyable, screen-reader-friendly output.
- **Font-substitution-safe** — Calibri / Calibri Light / Cambria / Arial / Times / Tahoma / Verdana / Courier all rendered via metric-compatible OFL substitutes (Liberation Sans / Serif / Mono).
- **Self-contained** — no LibreOffice, no Chromium, no remote services. Just a WASM blob.

## What it doesn't do (yet)

- **Footnote / endnote body text** is not rendered ([issue 004](research/04-found-issues/004-rdocx-footnote-body-text-not-rendered.md)).
- **Track changes** are inconsistently rendered ([issue 005](research/04-found-issues/005-rdocx-track-changes-not-rendered.md)).
- **Deeply-nested tables** (5+ levels) return an error ([issue 003](research/04-found-issues/003-rdocx-deep-nested-tables-fail.md)). Worker returns HTTP 422; the rest of the worker stays healthy.
- **Font fidelity** is approximate, not pixel-exact. Liberation Sans replaces Calibri etc. Per-glyph advance widths are similar but not identical, so paginations may drift slightly compared to Word.
- **Non-Latin scripts** beyond what fits in U+0000–U+024F + punctuation + currency are not in the bundled font subset. Bring your own font path for CJK / Cyrillic / Arabic / Greek.

## License

MIT. See [`LICENSE`](LICENSE). The bundled subset fonts (Liberation Sans / Serif / Mono) are licensed under SIL OFL 1.1.

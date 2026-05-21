# Per-document memory footprint

How much WebAssembly linear memory does a single conversion need? Cloudflare Workers' paid-plan isolate budget is **128 MB**, shared between V8's JS heap and the WASM linear memory. The numbers below are the linear-memory size of the converter instance immediately after the conversion completes — i.e. the peak working set per request.

Measured via [`../harness/single-doc-memory.mjs`](../harness/single-doc-memory.mjs), which loads the package's WASM directly (not through a worker) so the numbers reflect just the converter, with no V8 overhead from the worker shell.

## Numbers

| Document | DOCX size | Output | Linear memory after convert |
|---|---|---|---|
| tier1/test.docx (1 page) | 31 KB | 7 KB PDF | **10.3 MB** |
| tier1/test.docx | 31 KB | 1 KB HTML | 2.3 MB |
| tier1/test.docx | 31 KB | 0 KB MD | 2.3 MB |
| tier2/SampleDoc.docx | 11 KB | 9 KB PDF | 10.1 MB |
| tier3/having-images.docx | 130 KB | 186 KB PDF (w/ images) | 10.6 MB |
| complex/edu_mtu_thesis.docx (20 pages) | 897 KB | 851 KB PDF | **30.8 MB** |
| complex/edu_mtu_thesis.docx | 897 KB | 1.1 MB HTML | 14.8 MB |
| complex/nasa_report_bianco.docx (13 pages) | 1.5 MB | 1.5 MB PDF | 44.2 MB |
| complex/nasa_sewp_rfp.docx (167 pages) | 444 KB | 1.4 MB PDF | **107.0 MB** |
| complex/nasa_sewp_rfp.docx | 444 KB | 781 KB HTML | 46.6 MB |
| complex/nist_sp800_53.docx (**468 pages**) | 2 MB | 3.6 MB PDF | **403.8 MB** ⚠️ |
| complex/nist_sp800_53.docx | 2 MB | 6 MB HTML | 238.8 MB ⚠️ |
| complex/nist_sp800_53.docx | 2 MB | 1.4 MB MD | 215.3 MB ⚠️ |

## What this tells you

### Baseline cost ≈ 10 MB

Even the smallest 1-page document needs ~10 MB of linear memory. That's the bundled-and-subsetted Liberation font set (~1.3 MB raw decoded, but loaded into multiple internal structures by the layout engine) plus the rdocx allocator's initial commits.

The HTML/MD paths skip the PDF object graph and font emission, so their baseline drops to ~2 MB.

### PDF memory scales roughly with output size, not input size

The NASA SEWP RFP is **444 KB DOCX but renders to 167 pages** — its 107 MB working set reflects the layout engine holding all positioned glyphs / pages in memory before serializing the PDF. Compare to the 1.5 MB NASA tech report which only needs 44 MB because it's just 13 pages.

For PDF: rough rule of thumb is **5–10 MB per output page** of glyph + layout state.

### HTML and Markdown use about half the memory of PDF

For every document tested, HTML output uses roughly 0.5× the memory of PDF and runs ~10× faster. Markdown is similar. If your downstream consumer doesn't strictly need PDF (e.g. you're piping into an LLM for analysis), HTML or Markdown is the cheaper path.

### The 128 MB Workers ceiling

For 90 % of business documents (≤ 100 pages, ≤ 1 MB DOCX), a single PDF conversion stays well under 100 MB linear memory — fits the budget with margin.

For multi-hundred-page documents:
- **167 pages → 107 MB**: tight but fits.
- **300+ pages → 200+ MB**: will likely exceed the per-isolate limit and the worker will be killed mid-request.
- **468 pages (the NIST SP 800-53 spec) → 404 MB**: will not run on Workers. Confirmed by the bench harness producing this PDF in ~3 s native — at the edge it would OOM.

If you need to handle these reliably, options in order of effort:
1. **Use HTML output instead of PDF** (216 MB vs 404 MB for the NIST spec — still over budget but much closer).
2. **Use Markdown** (smallest memory footprint).
3. **Move to Cloudflare Containers** instead of Workers — no per-isolate memory cap.
4. **Pre-chunk the document** before conversion (split a 468-page DOCX into 4× 120-page chunks, render each, stitch the PDFs).

Most projects don't need any of these. The point is to know where the cliff is.

## Reproducing

```bash
node --expose-gc research/harness/single-doc-memory.mjs

# Or test specific documents:
node --expose-gc research/harness/single-doc-memory.mjs \
  /path/to/big-doc.docx /path/to/another.docx
```

The harness instantiates a fresh `WebAssembly.Instance` per measurement and reports the linear memory size after the conversion returns. It also samples Node's RSS at 5 ms intervals for an extra "JS-side overhead" reference, but those numbers include the harness itself and aren't directly comparable to Workers production.

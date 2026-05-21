# docx-to-pdf-wasm

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Convert Microsoft Word documents (`.docx`) to **PDF / HTML / Markdown** inside a WebAssembly module. **Runtime-agnostic** — runs on Cloudflare Workers, Node.js, Bun, Deno, and modern browsers with the same API.

```text
                        ┌─►  .pdf bytes        ~80 ms typical
.docx bytes  ─►  WASM   ├─►  HTML string       ~15 ms typical
                        └─►  Markdown string   ~10 ms typical

       1.04 MiB gzipped — all three formats from one binary
```

## Install

```bash
pnpm add docx-to-pdf-wasm
# or
npm install docx-to-pdf-wasm
# or
yarn add docx-to-pdf-wasm
```

## API

```ts
import {
  // High-level (recommended) — instantiate + convert per call:
  convert,                    // alias for convertToPdf, kept for back-compat
  convertToPdf,               // → Uint8Array (PDF bytes)
  convertToHtml,              // → string  (UTF-8 HTML document)
  convertToHtmlBytes,         // → Uint8Array (HTML, raw UTF-8 bytes)
  convertToMarkdown,          // → string  (UTF-8 Markdown / CommonMark + GFM)
  convertToMarkdownBytes,     // → Uint8Array (MD, raw UTF-8 bytes)
  convertTo,                  // (module, docx, "pdf" | "html" | "markdown") → Uint8Array
  // Low-level (advanced) — bring your own pre-instantiated instance:
  convertWithInstance,
  convertHtmlWithInstance,
  convertMarkdownWithInstance,
  instantiate,                // helper: WebAssembly.instantiate with the right imports
  // Errors / types
  ConvertError,
} from "docx-to-pdf-wasm";

// Separate entry point exposing the precompiled WASM binary:
//   "docx-to-pdf-wasm/wasm"
```

### `convertToPdf(module, docx)` — primary use case

```ts
const pdf: Uint8Array = await convertToPdf(wasmModule, docxBytes);
```

Instantiates a fresh `WebAssembly.Instance` per call. Recommended for serverless / per-request contexts: pathological inputs that trap inside the converter can't poison the next request's memory. The expensive part — compiling the `WebAssembly.Module` — happens once and is cached by V8.

### `convertToHtml(module, docx)` and `convertToMarkdown(module, docx)`

```ts
const html: string = await convertToHtml(wasmModule, docxBytes);
const md:   string = await convertToMarkdown(wasmModule, docxBytes);
```

HTML output is a full `<!DOCTYPE html>` document, UTF-8. Markdown is CommonMark with GFM-style pipe tables. Both are ~5× faster than PDF (no rendering pipeline).

**Known upstream quirk**: `rdocx-markdown` 0.1.2 emits headings as `**bold**` rather than ATX `# Heading`. Body content, lists, tables, and links render correctly. Tracked in [`research/04-found-issues/`](../../research/04-found-issues/).

### `convertTo(module, docx, format)` — generic dispatcher

When the output format is determined at runtime (e.g. from a query parameter):

```ts
import { convertTo, type OutputFormat } from "docx-to-pdf-wasm";

const format = (new URL(req.url)).searchParams.get("format") ?? "pdf";
const bytes = await convertTo(wasmModule, docxBytes, format as OutputFormat);
```

### `*WithInstance(instance, docx)` — advanced

```ts
const instance = await instantiate(wasmModule);
const pdf  = convertWithInstance(instance, docxBytes);          // Uint8Array
const html = convertHtmlWithInstance(instance, docxBytes);      // string
const md   = convertMarkdownWithInstance(instance, docxBytes);  // string
```

Synchronous after instantiation. Reuses one instance across calls — faster, but a single trap leaves the instance unusable for subsequent conversions. Only use if you have a specific reason to manage instance lifetimes yourself.

### Error handling

```ts
try {
  const pdf = await convert(wasmModule, docxBytes);
} catch (e) {
  if (e instanceof ConvertError) {
    // Carries the upstream Rust error message, e.g.
    //   "docx read: Opc(Zip(InvalidArchive(...)))" for malformed input
    //   "pdf render: ..." for layout/render errors
    console.error("conversion failed:", e.message);
  } else {
    // Unexpected — bug, OOM, etc.
    throw e;
  }
}
```

## Runtime recipes

### Cloudflare Workers

```toml
# wrangler.toml
name = "my-worker"
main = "src/worker.js"
compatibility_date = "2025-01-01"

[[rules]]
type = "CompiledWasm"
globs = ["**/*.wasm"]
fallthrough = true

[limits]
cpu_ms = 30000  # default; bump to 300_000 for very large docs
```

```ts
// src/worker.js
import { convert, ConvertError } from "docx-to-pdf-wasm";
// Note: see examples/cloudflare-worker/scripts/copy-wasm.mjs — wrangler's
// CompiledWasm loader needs the .wasm in the same source tree as the worker.
import wasmModule from "./converter.wasm";

export default {
  async fetch(req: Request) {
    if (req.method !== "POST") return new Response("Use POST", { status: 405 });
    const docx = new Uint8Array(await req.arrayBuffer());
    try {
      const pdf = await convert(wasmModule, docx);
      return new Response(pdf, {
        headers: { "content-type": "application/pdf" },
      });
    } catch (e) {
      const status = e instanceof ConvertError ? 422 : 500;
      return new Response(String(e), { status });
    }
  },
};
```

A complete working example lives at [`examples/cloudflare-worker/`](../../examples/cloudflare-worker/) — including the small `copy-wasm.mjs` script that copies the package's `.wasm` into the worker's source tree so wrangler's bundler picks it up.

### Node.js (≥ 18)

```ts
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { convert } from "docx-to-pdf-wasm";

const require = createRequire(import.meta.url);
const wasmBytes = await readFile(require.resolve("docx-to-pdf-wasm/wasm"));
const wasmModule = await WebAssembly.compile(wasmBytes);

const docx = new Uint8Array(await readFile("input.docx"));
const pdf = await convert(wasmModule, docx);
await import("node:fs/promises").then(fs => fs.writeFile("out.pdf", pdf));
```

### Bun

```ts
import { convert } from "docx-to-pdf-wasm";

const wasmFile = Bun.file(import.meta.resolveSync("docx-to-pdf-wasm/wasm"));
const wasmModule = await WebAssembly.compile(await wasmFile.arrayBuffer());

const docx = new Uint8Array(await Bun.file("input.docx").arrayBuffer());
const pdf = await convert(wasmModule, docx);
await Bun.write("out.pdf", pdf);
```

### Deno

```ts
import { convert } from "npm:docx-to-pdf-wasm";

const wasmUrl = import.meta.resolve("npm:docx-to-pdf-wasm/wasm");
const wasmModule = await WebAssembly.compileStreaming(fetch(wasmUrl));

const docx = await Deno.readFile("input.docx");
const pdf = await convert(wasmModule, docx);
await Deno.writeFile("out.pdf", pdf);
```

### Browser

```ts
import { convert } from "docx-to-pdf-wasm";
import wasmUrl from "docx-to-pdf-wasm/wasm?url"; // Vite-style; adjust to your bundler

const wasmModule = await WebAssembly.compileStreaming(fetch(wasmUrl));

document.querySelector("input[type=file]")!.addEventListener("change", async (e) => {
  const file = (e.target as HTMLInputElement).files![0];
  const docx = new Uint8Array(await file.arrayBuffer());
  const pdf = await convert(wasmModule, docx);
  const blob = new Blob([pdf], { type: "application/pdf" });
  window.open(URL.createObjectURL(blob));
});
```

## Bundle size

| Build stage | Size |
|---|---|
| Raw WASM (`cargo build --release --target wasm32-unknown-unknown`) | 2.57 MiB |
| After `wasm-opt -Oz` | 2.23 MiB |
| **Gzipped (what CF Workers sees on deploy)** | **1.03 MiB** |
| Brotli q11 | 0.79 MiB |

The bundle contains:
- Rust compiled to WebAssembly: the OOXML parser ([`rdocx`](https://github.com/tensorbee/rdocx)), layout engine, PDF emitter, with a textbox preprocessor for [text-box content extraction](../../research/04-found-issues/002-rdocx-textbox-content-not-extracted.md).
- 12 Liberation font faces (Sans / Serif / Mono × Regular / Bold / Italic / BoldItalic), each subset to a Latin codepoint set via [pyftsubset](https://fonttools.readthedocs.io/en/latest/subset/index.html). ~88 % per-font byte reduction.
- An alias table mapping common Microsoft Office font names (Calibri, Cambria, Arial, Times, Tahoma, Verdana, Courier, Consolas, …) to the Liberation faces, so documents render with reasonable typography out of the box.

## Quality / fidelity

| Corpus | Files | Avg recall vs LibreOffice |
|---|---|---|
| Toy fixtures (T1 simple / T2 business / T3 complex) | 25 | **1.00 / 0.99 / 0.89** |
| Real-world (gov / academic / business, 1 page – 468 pages) | 25 | **0.98** |

Conversion latency on a single Cloudflare Workers isolate (V8 / workerd):
- 1-page document → ~50 ms
- 13-page NASA tech report → ~1.2 s
- 167-page NASA RFP → ~0.7 s
- 468-page NIST SP 800-53 spec → ~3.7 s

All comfortably inside Workers' default 30 s CPU budget.

## Known limitations

See [`research/04-found-issues/`](../../research/04-found-issues/) for upstream bugs we've documented, including:

- Footnote / endnote body text not rendered ([issue 004](../../research/04-found-issues/004-rdocx-footnote-body-text-not-rendered.md))
- Track changes inconsistently handled ([issue 005](../../research/04-found-issues/005-rdocx-track-changes-not-rendered.md))
- Deeply nested tables (5+ levels) rejected ([issue 003](../../research/04-found-issues/003-rdocx-deep-nested-tables-fail.md))

For each, the converter returns a clear error rather than corrupted output. The Cloudflare Worker isolate stays healthy across failures thanks to the fresh-per-call instantiation pattern baked into `convert()`.

## How it's built

The WASM artifact in `build/docx-to-pdf.wasm` is compiled from Rust source at [`research/02-optimizations/opt-8-textbox-preprocessor-subset/converter/`](../../research/02-optimizations/opt-8-textbox-preprocessor-subset/converter/). To rebuild it:

```bash
# From the repo root
pnpm build:wasm
```

Build requirements:
- Rust 1.95+ with the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
- Python 3 with `fontTools` + `brotli` (`pip3 install --user fontTools brotli`) — used by the crate's `build.rs` to subset fonts
- `wasm-opt` (from `binaryen`) — optional but recommended for `-Oz` optimization

To rebuild the TypeScript bindings only:

```bash
pnpm build:ts
```

This produces `build/index.js`, `build/index.d.ts`, `build/convert.js`, `build/convert.d.ts` from the `src/*.ts` sources.

## Background

This package is the production artifact of a multi-week R&D project. The `research/` folder at the repo root documents the full history: three different approaches evaluated in parallel, eight optimization variants on the chosen approach, stress testing against real-world public documents, and the discovery of an upstream Carlito ToUnicode-CMap bug that drove the final architecture. See [`research/README.md`](../../research/README.md) for the journey.

## License

MIT. See [`LICENSE`](./LICENSE). Bundled fonts (Liberation Sans / Serif / Mono) are licensed under SIL OFL 1.1.

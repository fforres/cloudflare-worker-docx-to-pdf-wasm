// Minimal Cloudflare Worker that consumes the `docx-to-pdf-wasm` package.
//
// `POST /convert` with a DOCX body returns the corresponding PDF.
//
// The package's `convert()` function instantiates a fresh WASM instance per
// call. We keep the compiled `WebAssembly.Module` at module scope so the
// expensive compilation happens once per isolate; instantiation itself is
// cheap (~5 ms) and gives us trap-resilience: pathological inputs that trap
// inside the converter can't poison the next request's memory.

import { convert, ConvertError } from "docx-to-pdf-wasm";
// converter.wasm is copied into this directory by `scripts/copy-wasm.mjs`,
// which runs automatically via `predev` / `predeploy` npm scripts. The copy
// step is needed because wrangler's CompiledWasm loader rule globs are
// resolved relative to this worker's source root, not the project root.
import wasmModule from "./converter.wasm";

export default {
  async fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/" || url.pathname === "/health") {
      return new Response(
        "docx-to-pdf-wasm — POST a .docx body to /convert\n",
        { headers: { "content-type": "text/plain" } },
      );
    }

    if (url.pathname !== "/convert") {
      return new Response("Not found", { status: 404 });
    }
    if (req.method !== "POST") {
      return new Response("Use POST with a .docx body", { status: 405 });
    }

    let docx;
    try {
      docx = new Uint8Array(await req.arrayBuffer());
    } catch (e) {
      return new Response(`read body: ${e}`, { status: 400 });
    }
    if (docx.length === 0) {
      return new Response("empty body", { status: 400 });
    }

    try {
      const pdf = await convert(wasmModule, docx);
      return new Response(pdf, {
        headers: {
          "content-type": "application/pdf",
          "content-length": String(pdf.length),
        },
      });
    } catch (e) {
      const status = e instanceof ConvertError ? 422 : 500;
      return new Response(String(e), { status });
    }
  },
};

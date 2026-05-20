// Cloudflare Worker that wraps approach-c-rdocx's WASM module.
// Accepts POST /convert with a DOCX body, returns a PDF.
//
// We keep the compiled module (expensive) at module top level, and lazily
// instantiate one fresh `instance` per request. Some malformed DOCX inputs
// cause genuine WASM traps inside rdocx that poison the linear memory; the
// only safe recovery is a fresh instance. Instantiation of this module is
// ~few ms in V8 since the compiled bytecode is cached.

import converterModule from "./converter.wasm";

function readLastError(exports) {
  const ptr = exports.last_error_ptr();
  const len = Number(exports.last_error_len());
  if (!len) return "(no error message)";
  return new TextDecoder().decode(
    new Uint8Array(exports.memory.buffer, Number(ptr), len),
  );
}

async function convert(docxBytes) {
  // `converterModule` is a compiled `WebAssembly.Module` (provided by
  // wrangler's bundler). Instantiating from a Module returns the Instance
  // directly, not a `{ module, instance }` pair.
  const instance = await WebAssembly.instantiate(converterModule, {});
  const exports = instance.exports;

  const inPtr = exports.alloc(docxBytes.length);
  new Uint8Array(exports.memory.buffer, Number(inPtr), docxBytes.length).set(
    docxBytes,
  );

  const packed = exports.convert_wasm(inPtr, docxBytes.length);
  const outPtr = Number(packed >> 32n);
  const outLen = Number(packed & 0xffffffffn);

  if (outLen === 0) {
    const msg = readLastError(exports);
    throw new Error(`convert failed: ${msg}`);
  }

  // Copy out before the instance goes out of scope.
  return new Uint8Array(exports.memory.buffer, outPtr, outLen).slice();
}

export default {
  async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === "/" || url.pathname === "/health") {
      return new Response(
        "wasm-docx-to-pdf — POST a .docx body to /convert\n",
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
      const pdf = await convert(docx);
      return new Response(pdf, {
        headers: {
          "content-type": "application/pdf",
          "content-length": String(pdf.length),
        },
      });
    } catch (e) {
      return new Response(String(e), { status: 500 });
    }
  },
};

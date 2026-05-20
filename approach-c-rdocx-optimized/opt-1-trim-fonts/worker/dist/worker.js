var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// src/worker.js
import converterModule from "./4296335daf5b3155c17a2e773e6f223c18379a26-converter.wasm";
function readLastError(exports) {
  const ptr = exports.last_error_ptr();
  const len = Number(exports.last_error_len());
  if (!len) return "(no error message)";
  return new TextDecoder().decode(
    new Uint8Array(exports.memory.buffer, Number(ptr), len)
  );
}
__name(readLastError, "readLastError");
async function convert(docxBytes) {
  const instance = await WebAssembly.instantiate(converterModule, {});
  const exports = instance.exports;
  const inPtr = exports.alloc(docxBytes.length);
  new Uint8Array(exports.memory.buffer, Number(inPtr), docxBytes.length).set(
    docxBytes
  );
  const packed = exports.convert_wasm(inPtr, docxBytes.length);
  const outPtr = Number(packed >> 32n);
  const outLen = Number(packed & 0xffffffffn);
  if (outLen === 0) {
    const msg = readLastError(exports);
    throw new Error(`convert failed: ${msg}`);
  }
  return new Uint8Array(exports.memory.buffer, outPtr, outLen).slice();
}
__name(convert, "convert");
var worker_default = {
  async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === "/" || url.pathname === "/health") {
      return new Response(
        "wasm-docx-to-pdf \u2014 POST a .docx body to /convert\n",
        { headers: { "content-type": "text/plain" } }
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
          "content-length": String(pdf.length)
        }
      });
    } catch (e) {
      return new Response(String(e), { status: 500 });
    }
  }
};
export {
  worker_default as default
};
//# sourceMappingURL=worker.js.map

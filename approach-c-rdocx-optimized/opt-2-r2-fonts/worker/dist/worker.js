var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// src/worker.js
import converterModule from "./1fef14f5ce5c6772d31f56e90ac62fb7ae25cd6c-converter.wasm";
import carlitoRegular from "./e4d65dc1935cfaf2ae3dc902cc5c1a1e02b9eb55-Carlito-Regular.ttf";
import carlitoBold from "./30a54846eecea6004353805f37e595eaadde40fd-Carlito-Bold.ttf";
import liberationSerifRegular from "./8e9b0876ef26ead1806c2c5a15f7db9bfda19be2-LiberationSerif-Regular.ttf";
import liberationSerifBold from "./f06fbb94859d36b905cc4fd008479a3176f5fc87-LiberationSerif-Bold.ttf";
function buildFontsBuffer(fonts) {
  const encoder = new TextEncoder();
  const encoded = fonts.map(([name, data]) => ({
    name: encoder.encode(name),
    data: new Uint8Array(data)
  }));
  let total = 0;
  for (const { name, data } of encoded) total += 4 + name.length + 4 + data.length;
  const buf = new Uint8Array(total);
  const view = new DataView(buf.buffer);
  let off = 0;
  for (const { name, data } of encoded) {
    view.setUint32(off, name.length, true);
    off += 4;
    buf.set(name, off);
    off += name.length;
    view.setUint32(off, data.length, true);
    off += 4;
    buf.set(data, off);
    off += data.length;
  }
  return buf;
}
__name(buildFontsBuffer, "buildFontsBuffer");
var FONTS_BUFFER = buildFontsBuffer([
  ["Carlito", carlitoRegular],
  ["Carlito", carlitoBold],
  ["Liberation Serif", liberationSerifRegular],
  ["Liberation Serif", liberationSerifBold]
]);
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
  new Uint8Array(exports.memory.buffer, Number(inPtr), docxBytes.length).set(docxBytes);
  const fontsPtr = exports.alloc(FONTS_BUFFER.length);
  new Uint8Array(exports.memory.buffer, Number(fontsPtr), FONTS_BUFFER.length).set(FONTS_BUFFER);
  const packed = exports.convert_with_fonts(
    inPtr,
    docxBytes.length,
    fontsPtr,
    FONTS_BUFFER.length
  );
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
        "wasm-docx-to-pdf opt-2 (fonts at runtime) \u2014 POST a .docx body to /convert\n",
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

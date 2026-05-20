// opt-2 worker: WASM has NO bundled fonts. We import 4 TTFs as data assets,
// encode them into a mini-format buffer once, and pass that to convert_with_fonts.
//
// In production these would be fetched from an R2 binding instead of bundled —
// see worker/README.md for the swap-in code shape.

import converterModule from "./converter.wasm";
import carlitoRegular from "./fonts/Carlito-Regular.ttf";
import carlitoBold from "./fonts/Carlito-Bold.ttf";
import liberationSerifRegular from "./fonts/LiberationSerif-Regular.ttf";
import liberationSerifBold from "./fonts/LiberationSerif-Bold.ttf";

// Mini-format the Rust side decodes:
//   record = name_len(u32 LE) name_bytes data_len(u32 LE) data_bytes
function buildFontsBuffer(fonts) {
  const encoder = new TextEncoder();
  const encoded = fonts.map(([name, data]) => ({
    name: encoder.encode(name),
    data: new Uint8Array(data),
  }));
  let total = 0;
  for (const { name, data } of encoded) total += 4 + name.length + 4 + data.length;
  const buf = new Uint8Array(total);
  const view = new DataView(buf.buffer);
  let off = 0;
  for (const { name, data } of encoded) {
    view.setUint32(off, name.length, true); off += 4;
    buf.set(name, off); off += name.length;
    view.setUint32(off, data.length, true); off += 4;
    buf.set(data, off); off += data.length;
  }
  return buf;
}

// Build the fonts buffer once at module load. In a fresh isolate this runs
// before the first request — same shape as the future R2 path (which will
// build it lazily on first request after awaiting R2 fetches in parallel).
const FONTS_BUFFER = buildFontsBuffer([
  ["Carlito", carlitoRegular],
  ["Carlito", carlitoBold],
  ["Liberation Serif", liberationSerifRegular],
  ["Liberation Serif", liberationSerifBold],
]);

function readLastError(exports) {
  const ptr = exports.last_error_ptr();
  const len = Number(exports.last_error_len());
  if (!len) return "(no error message)";
  return new TextDecoder().decode(
    new Uint8Array(exports.memory.buffer, Number(ptr), len),
  );
}

async function convert(docxBytes) {
  // Fresh instance per request: trap-resilience (a panic on bad input
  // poisons linear memory; only re-instantiation cleans it up).
  const instance = await WebAssembly.instantiate(converterModule, {});
  const exports = instance.exports;

  const inPtr = exports.alloc(docxBytes.length);
  new Uint8Array(exports.memory.buffer, Number(inPtr), docxBytes.length).set(docxBytes);

  const fontsPtr = exports.alloc(FONTS_BUFFER.length);
  new Uint8Array(exports.memory.buffer, Number(fontsPtr), FONTS_BUFFER.length).set(FONTS_BUFFER);

  const packed = exports.convert_with_fonts(
    inPtr, docxBytes.length, fontsPtr, FONTS_BUFFER.length,
  );
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
        "wasm-docx-to-pdf opt-2 (fonts at runtime) — POST a .docx body to /convert\n",
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

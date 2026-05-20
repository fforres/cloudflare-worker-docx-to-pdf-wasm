#!/usr/bin/env node
// Drive the opt-2 WASM from Node. Mirrors what the Worker does:
//   1. load 4 TTFs from worker/src/fonts/
//   2. encode them into the mini-format buffer
//   3. instantiate the WASM module
//   4. call convert_with_fonts(docx_ptr, docx_len, fonts_ptr, fonts_len)
//
// Usage: wasm-runner.mjs <input.docx> <output.pdf>
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const [, , inPath, outPath] = process.argv;
if (!inPath || !outPath) {
  console.error("usage: wasm-runner.mjs <input.docx> <output.pdf>");
  process.exit(2);
}

const wasmPath =
  process.env.WASM_PATH ||
  path.join(
    __dirname,
    "converter/target/wasm32-unknown-unknown/release/approach_c_rdocx_opt2.opt.wasm",
  );

const fontsDir = path.join(__dirname, "worker/src/fonts");
const fontList = [
  ["Carlito", "Carlito-Regular.ttf"],
  ["Carlito", "Carlito-Bold.ttf"],
  ["Liberation Serif", "LiberationSerif-Regular.ttf"],
  ["Liberation Serif", "LiberationSerif-Bold.ttf"],
];

function buildFontsBuffer(fonts) {
  const encoder = new TextEncoder();
  const encoded = fonts.map(([name, data]) => ({
    name: encoder.encode(name),
    data,
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

const fontsPayload = fontList.map(([family, file]) => [
  family,
  new Uint8Array(fs.readFileSync(path.join(fontsDir, file))),
]);
const fontsBuffer = buildFontsBuffer(fontsPayload);

const wasmBytes = fs.readFileSync(wasmPath);
const docxBytes = fs.readFileSync(inPath);

const { instance } = await WebAssembly.instantiate(wasmBytes, {});
const {
  memory, alloc, dealloc, convert_with_fonts, last_error_ptr, last_error_len,
} = instance.exports;

const inPtr = alloc(docxBytes.length);
new Uint8Array(memory.buffer, inPtr, docxBytes.length).set(docxBytes);

const fontsPtr = alloc(fontsBuffer.length);
new Uint8Array(memory.buffer, fontsPtr, fontsBuffer.length).set(fontsBuffer);

const packed = convert_with_fonts(inPtr, docxBytes.length, fontsPtr, fontsBuffer.length);
const outPtr = Number(packed >> 32n);
const outLen = Number(packed & 0xffffffffn);

if (outLen === 0) {
  const errPtr = last_error_ptr();
  const errLen = last_error_len();
  const msg = errLen > 0
    ? Buffer.from(new Uint8Array(memory.buffer, errPtr, errLen)).toString("utf8")
    : "(no error message)";
  console.error("convert_with_fonts failed: " + msg);
  dealloc(inPtr, docxBytes.length);
  dealloc(fontsPtr, fontsBuffer.length);
  process.exit(1);
}

const pdf = Buffer.from(new Uint8Array(memory.buffer, outPtr, outLen).slice());
fs.writeFileSync(outPath, pdf);

dealloc(inPtr, docxBytes.length);
dealloc(fontsPtr, fontsBuffer.length);
dealloc(outPtr, outLen);

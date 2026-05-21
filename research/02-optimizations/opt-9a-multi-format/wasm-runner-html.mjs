#!/usr/bin/env node
// Drive the opt-9a WASM module to produce HTML.
// Usage: wasm-runner-html.mjs <input.docx> <output.html>
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const [, , inPath, outPath] = process.argv;
if (!inPath || !outPath) {
  console.error("usage: wasm-runner-html.mjs <input.docx> <output.html>");
  process.exit(2);
}

const wasmPath =
  process.env.WASM_PATH ||
  path.join(
    __dirname,
    "converter/target/wasm32-unknown-unknown/release/approach_c_rdocx_opt9a.wasm",
  );

const wasmBytes = fs.readFileSync(wasmPath);
const docxBytes = fs.readFileSync(inPath);

const { instance } = await WebAssembly.instantiate(wasmBytes);
const {
  memory,
  alloc,
  dealloc,
  convert_html_wasm,
  last_error_ptr,
  last_error_len,
} = instance.exports;

const inPtr = alloc(docxBytes.length);
new Uint8Array(memory.buffer, inPtr, docxBytes.length).set(docxBytes);

const packed = convert_html_wasm(inPtr, docxBytes.length);
const outPtr = Number(packed >> 32n);
const outLen = Number(packed & 0xffffffffn);

if (outLen === 0) {
  const errPtr = last_error_ptr();
  const errLen = last_error_len();
  const msg = errLen > 0
    ? Buffer.from(new Uint8Array(memory.buffer, errPtr, errLen)).toString("utf8")
    : "(no error message)";
  console.error("convert_html_wasm failed: " + msg);
  dealloc(inPtr, docxBytes.length);
  process.exit(1);
}

const html = Buffer.from(new Uint8Array(memory.buffer, outPtr, outLen).slice());
fs.writeFileSync(outPath, html);

dealloc(inPtr, docxBytes.length);
dealloc(outPtr, outLen);

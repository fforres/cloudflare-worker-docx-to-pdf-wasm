#!/usr/bin/env node
// Drive the opt-1-trim-fonts WASM module from Node.js so the harness can score it.
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

// Allow override via env var (e.g., post wasm-opt build).
const wasmPath =
  process.env.WASM_PATH ||
  path.join(__dirname, "converter/target/wasm32-unknown-unknown/release/approach_c_rdocx.opt.wasm");

const wasmBytes = fs.readFileSync(wasmPath);
const docxBytes = fs.readFileSync(inPath);

const { instance } = await WebAssembly.instantiate(wasmBytes);
const { memory, alloc, dealloc, convert_wasm, last_error_ptr, last_error_len } = instance.exports;

// Copy DOCX bytes into WASM linear memory.
const inPtr = alloc(docxBytes.length);
new Uint8Array(memory.buffer, inPtr, docxBytes.length).set(docxBytes);

// Convert.
const packed = convert_wasm(inPtr, docxBytes.length);
// packed is a BigInt (i64): (out_ptr << 32) | out_len
const outPtr = Number(packed >> 32n);
const outLen = Number(packed & 0xffffffffn);

if (outLen === 0) {
  const errPtr = last_error_ptr();
  const errLen = last_error_len();
  const msg = errLen > 0
    ? Buffer.from(new Uint8Array(memory.buffer, errPtr, errLen)).toString("utf8")
    : "(no error message)";
  console.error("convert_wasm failed: " + msg);
  dealloc(inPtr, docxBytes.length);
  process.exit(1);
}

// Copy PDF out before deallocating.
const pdf = Buffer.from(
  new Uint8Array(memory.buffer, outPtr, outLen).slice(),
);
fs.writeFileSync(outPath, pdf);

dealloc(inPtr, docxBytes.length);
dealloc(outPtr, outLen);

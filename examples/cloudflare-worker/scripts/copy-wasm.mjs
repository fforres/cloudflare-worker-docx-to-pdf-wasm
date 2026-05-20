#!/usr/bin/env node
// Copy the docx-to-pdf-wasm package's compiled WASM into src/ so wrangler's
// CompiledWasm rule (which globs relative to the worker source root) can pick
// it up. Runs as `predev` / `predeploy` via npm scripts.
import { copyFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";

const require = createRequire(import.meta.url);
const wasmSrc = require.resolve("docx-to-pdf-wasm/wasm");
const workerSrc = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "src",
);
mkdirSync(workerSrc, { recursive: true });
const wasmDst = path.join(workerSrc, "converter.wasm");
copyFileSync(wasmSrc, wasmDst);
console.log(`copied ${wasmSrc}\n     → ${wasmDst}`);

#!/usr/bin/env node
// Measure the per-request memory footprint for a single conversion.
//
// Two numbers are reported:
//
//   linear_memory_bytes — WebAssembly linear memory used by the converter
//                         instance. This is the production-relevant number;
//                         Cloudflare Workers' 128 MB isolate limit counts
//                         this against the budget.
//
//   node_rss_bytes      — Node's resident-set-size delta from baseline to
//                         peak during conversion. Includes the docx input
//                         buffer, the pdf output buffer, V8 overhead, etc.
//                         Not directly comparable to a production isolate
//                         but gives a sense of the JS-side allocations.
//
// Usage:
//   node single-doc-memory.mjs <docx-path> [<docx-path> ...]
//
// Defaults to running through a fixed list of fixtures across size tiers.

import { readFile } from "node:fs/promises";
import {
  convertToPdf,
  convertToHtml,
  convertToMarkdown,
} from "/Users/fforres/GITHUB/skyward/wasm-docx-to-pdf/packages/docx-to-pdf-wasm/build/index.js";

const PKG_WASM =
  "/Users/fforres/GITHUB/skyward/wasm-docx-to-pdf/packages/docx-to-pdf-wasm/build/docx-to-pdf.wasm";
const F = "/Users/fforres/GITHUB/skyward/wasm-docx-to-pdf/research/fixtures";

const DEFAULT_FIXTURES = [
  `${F}/tier1/test.docx`,
  `${F}/tier2/SampleDoc.docx`,
  `${F}/tier3/having-images.docx`,
  `${F}/complex/edu_mtu_thesis.docx`,
  `${F}/complex/nasa_report_bianco.docx`,
  `${F}/complex/nasa_sewp_rfp.docx`,
  `${F}/complex/nist_sp800_53.docx`,
];

const fixtures = process.argv.slice(2).length
  ? process.argv.slice(2)
  : DEFAULT_FIXTURES;

const FORMATS = {
  pdf: convertToPdf,
  html: convertToHtml,
  markdown: convertToMarkdown,
};

function mb(bytes) {
  return +(bytes / (1024 * 1024)).toFixed(2);
}

async function main() {
  const wasmBytes = await readFile(PKG_WASM);
  const wasmModule = await WebAssembly.compile(wasmBytes);

  console.log("docx-to-pdf-wasm — single-document memory footprint");
  console.log(
    `wasm: ${mb(wasmBytes.byteLength)} MB raw / ${mb(
      (await import("node:zlib")).gzipSync(new Uint8Array(wasmBytes)).length,
    )} MB gzipped`,
  );
  console.log();

  const header = [
    "doc",
    "in_KB",
    "fmt",
    "out_KB",
    "ms",
    "linear_MB",
    "rss_baseline_MB",
    "rss_peak_MB",
    "rss_delta_MB",
  ].join("\t");
  console.log(header);

  for (const fixturePath of fixtures) {
    const docxBytes = new Uint8Array(await readFile(fixturePath));
    const name = fixturePath.split("/").slice(-2).join("/");

    for (const [fmtName, convertFn] of Object.entries(FORMATS)) {
      // Fresh instance per measurement so we observe the worst-case linear
      // memory growth of a single conversion in isolation.
      const instance = await WebAssembly.instantiate(wasmModule, {});

      // Sample linear memory size before and after the convert call.
      const memBefore = (instance.exports.memory).buffer.byteLength;

      // RSS baseline. Force a GC pass first if --expose-gc was passed.
      if (typeof global.gc === "function") global.gc();
      await new Promise((r) => setTimeout(r, 10));
      const rssBaseline = process.memoryUsage.rss();

      // Run via the package's public API (uses its own fresh instance, but
      // we still want the linear-memory size that the *package's* convert
      // ends up growing to — so we re-measure right after).
      let rssPeak = rssBaseline;
      const sampler = setInterval(() => {
        const r = process.memoryUsage.rss();
        if (r > rssPeak) rssPeak = r;
      }, 5);

      const t0 = performance.now();
      let outBytes = 0;
      try {
        const out = await convertFn(wasmModule, docxBytes);
        outBytes =
          typeof out === "string" ? Buffer.byteLength(out, "utf8") : out.length;
      } catch (e) {
        clearInterval(sampler);
        console.log(`${name}\t${(docxBytes.length / 1024).toFixed(0)}\t${fmtName}\tFAIL: ${e.message?.slice(0, 60)}`);
        continue;
      }
      const elapsed = performance.now() - t0;
      clearInterval(sampler);

      // Re-measure the original instance to get its grown linear memory.
      // (The package call uses a fresh instance internally; this is a rough
      // proxy — we approximate by running once more directly on `instance`.)
      try {
        const inPtr = instance.exports.alloc(docxBytes.length);
        new Uint8Array(instance.exports.memory.buffer, inPtr, docxBytes.length).set(docxBytes);
        const packed =
          fmtName === "pdf"
            ? instance.exports.convert_wasm(inPtr, docxBytes.length)
            : fmtName === "html"
            ? instance.exports.convert_html_wasm(inPtr, docxBytes.length)
            : instance.exports.convert_md_wasm(inPtr, docxBytes.length);
        const outLen = Number(packed & 0xffffffffn);
        const outPtr = Number(packed >> 32n);
        if (outLen > 0) instance.exports.dealloc(outPtr, outLen);
        instance.exports.dealloc(inPtr, docxBytes.length);
      } catch (e) {
        // ignore — we still have memBefore/memAfter snapshots
      }
      const memAfter = instance.exports.memory.buffer.byteLength;

      console.log(
        [
          name,
          (docxBytes.length / 1024).toFixed(0),
          fmtName,
          (outBytes / 1024).toFixed(0),
          elapsed.toFixed(0),
          mb(memAfter),
          mb(rssBaseline),
          mb(rssPeak),
          mb(rssPeak - rssBaseline),
        ].join("\t"),
      );
    }
    console.log();
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

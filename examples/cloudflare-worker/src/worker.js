// Minimal Cloudflare Worker that consumes the `docx-to-pdf-wasm` package.
//
// Endpoints:
//   GET  /  GET /health         → 200 text/plain banner
//   POST /convert               → 200 application/pdf
//   POST /convert/html          → 200 text/html; charset=utf-8
//   POST /convert/markdown      → 200 text/markdown; charset=utf-8
//   *                           → 404
//
// Each /convert request instantiates a fresh WASM instance (trap resilience).
// The compiled `WebAssembly.Module` stays at module scope so it's compiled
// once per isolate; instantiation is cheap (~5 ms warm).

import {
  ConvertError,
  convertToPdf,
  convertToHtmlBytes,
  convertToMarkdownBytes,
} from "docx-to-pdf-wasm";
// The .wasm is copied into this directory by `scripts/copy-wasm.mjs`
// (predev / predeploy npm scripts).
import wasmModule from "./converter.wasm";

const ROUTES = {
  "/convert": {
    contentType: "application/pdf",
    convert: convertToPdf,
  },
  "/convert/html": {
    contentType: "text/html; charset=utf-8",
    convert: convertToHtmlBytes,
  },
  "/convert/markdown": {
    contentType: "text/markdown; charset=utf-8",
    convert: convertToMarkdownBytes,
  },
};

export default {
  async fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/" || url.pathname === "/health") {
      return new Response(
        [
          "docx-to-pdf-wasm",
          "",
          "POST /convert            DOCX body → PDF",
          "POST /convert/html       DOCX body → HTML",
          "POST /convert/markdown   DOCX body → Markdown",
          "",
        ].join("\n"),
        { headers: { "content-type": "text/plain; charset=utf-8" } },
      );
    }

    const route = ROUTES[url.pathname];
    if (!route) {
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
      const out = await route.convert(wasmModule, docx);
      return new Response(out, {
        headers: {
          "content-type": route.contentType,
          "content-length": String(out.length),
        },
      });
    } catch (e) {
      const status = e instanceof ConvertError ? 422 : 500;
      return new Response(String(e), { status });
    }
  },
};

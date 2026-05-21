/**
 * Error thrown when the WASM converter returns a failure.
 *
 * The `message` field carries the upstream Rust error text (e.g.
 * "docx read: Opc(Zip(InvalidArchive(...)))" for malformed inputs).
 */
export class ConvertError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ConvertError";
  }
}

/**
 * The output formats the WASM module can emit.
 */
export type OutputFormat = "pdf" | "html" | "markdown";

/**
 * The shape of the WebAssembly instance exports produced by this package's
 * Rust source.
 */
export interface ConverterExports {
  memory: WebAssembly.Memory;
  alloc(size: number): number;
  dealloc(ptr: number, size: number): void;
  /** PDF output. Packed `(out_ptr << 32) | out_len`; len === 0 signals failure. */
  convert_wasm(ptr: number, len: number): bigint;
  /** HTML output (UTF-8 string bytes). Same packing as `convert_wasm`. */
  convert_html_wasm(ptr: number, len: number): bigint;
  /** Markdown output (UTF-8 string bytes). Same packing. */
  convert_md_wasm(ptr: number, len: number): bigint;
  last_error_ptr(): number;
  last_error_len(): number;
}

/**
 * Instantiate the converter `WebAssembly.Module`.
 *
 * Prefer creating a fresh instance per conversion (the high-level functions
 * do this automatically). Some pathological inputs can trap inside Rust and
 * leave the linear memory in an undefined state — reusing such an instance
 * would silently produce garbage. The compiled `WebAssembly.Module` itself is
 * safe to hold at module scope, since compilation is the expensive part and
 * V8 / workerd keep the compiled artifact cached across instantiations.
 */
export async function instantiate(
  module: WebAssembly.Module,
): Promise<WebAssembly.Instance> {
  return WebAssembly.instantiate(module, {});
}

// --------------------------------------------------------------------------
// Low-level: invoke a specific WASM export against a fresh instance.
// --------------------------------------------------------------------------

function invokeExport(
  instance: WebAssembly.Instance,
  exportName: "convert_wasm" | "convert_html_wasm" | "convert_md_wasm",
  docx: Uint8Array,
): Uint8Array {
  const exports = instance.exports as unknown as ConverterExports;
  const { memory, alloc, dealloc, last_error_ptr, last_error_len } = exports;
  const fn = exports[exportName];

  const inPtr = alloc(docx.length);
  new Uint8Array(memory.buffer, inPtr, docx.length).set(docx);

  const packed = fn(inPtr, docx.length);
  const outPtr = Number(packed >> 32n);
  const outLen = Number(packed & 0xffffffffn);

  if (outLen === 0) {
    const errPtr = last_error_ptr();
    const errLen = Number(last_error_len());
    const msg =
      errLen > 0
        ? new TextDecoder().decode(
            new Uint8Array(memory.buffer, errPtr, errLen),
          )
        : "(no error message)";
    dealloc(inPtr, docx.length);
    throw new ConvertError(msg);
  }

  const out = new Uint8Array(memory.buffer, outPtr, outLen).slice();
  dealloc(inPtr, docx.length);
  dealloc(outPtr, outLen);
  return out;
}

// --------------------------------------------------------------------------
// PDF (the primary, default output)
// --------------------------------------------------------------------------

/**
 * Convert DOCX bytes to PDF bytes.
 *
 * Creates a fresh WASM instance per call. Recommended for serverless or
 * per-request contexts: pathological inputs that trap inside the converter
 * cannot poison the next request's memory.
 *
 * @example Cloudflare Workers
 * ```ts
 * import { convertToPdf } from "docx-to-pdf-wasm";
 * import wasmModule from "./converter.wasm";
 *
 * const pdf = await convertToPdf(wasmModule, docxBytes);
 * ```
 *
 * @throws {ConvertError} when the underlying Rust converter returns an error.
 */
export async function convertToPdf(
  module: WebAssembly.Module,
  docx: Uint8Array,
): Promise<Uint8Array> {
  const instance = await instantiate(module);
  return invokeExport(instance, "convert_wasm", docx);
}

/**
 * Alias for `convertToPdf`. Kept as the canonical, back-compatible name —
 * this is the function exported as `convert` from the package.
 */
export const convert = convertToPdf;

// --------------------------------------------------------------------------
// HTML
// --------------------------------------------------------------------------

/**
 * Convert DOCX bytes to an HTML document (UTF-8).
 *
 * Returns a complete `<!DOCTYPE html>` document. For just the body fragment,
 * call this and slice between `<body>` and `</body>`, or use
 * `convertToHtmlBytes` directly if you want to skip the string decoding.
 *
 * Note: `rdocx-markdown` 0.1.2 emits headings as `<strong>` rather than
 * `<h1>`/`<h2>` in some cases. Body content, lists, tables, and links are
 * preserved correctly.
 *
 * @throws {ConvertError}
 */
export async function convertToHtml(
  module: WebAssembly.Module,
  docx: Uint8Array,
): Promise<string> {
  const bytes = await convertToHtmlBytes(module, docx);
  return new TextDecoder().decode(bytes);
}

/** Same as `convertToHtml` but returns raw UTF-8 bytes (zero-copy if you stream it out). */
export async function convertToHtmlBytes(
  module: WebAssembly.Module,
  docx: Uint8Array,
): Promise<Uint8Array> {
  const instance = await instantiate(module);
  return invokeExport(instance, "convert_html_wasm", docx);
}

// --------------------------------------------------------------------------
// Markdown
// --------------------------------------------------------------------------

/**
 * Convert DOCX bytes to a Markdown document (UTF-8 CommonMark + GFM tables).
 *
 * Note: `rdocx-markdown` 0.1.2 emits headings as `**bold**` rather than ATX
 * `# Heading` — this is a known upstream limitation. Body paragraphs, lists,
 * and tables are correct.
 *
 * @throws {ConvertError}
 */
export async function convertToMarkdown(
  module: WebAssembly.Module,
  docx: Uint8Array,
): Promise<string> {
  const bytes = await convertToMarkdownBytes(module, docx);
  return new TextDecoder().decode(bytes);
}

/** Same as `convertToMarkdown` but returns raw UTF-8 bytes. */
export async function convertToMarkdownBytes(
  module: WebAssembly.Module,
  docx: Uint8Array,
): Promise<Uint8Array> {
  const instance = await instantiate(module);
  return invokeExport(instance, "convert_md_wasm", docx);
}

// --------------------------------------------------------------------------
// Generic dispatcher
// --------------------------------------------------------------------------

/**
 * Convenience dispatcher when the output format is determined at runtime
 * (e.g. from a query parameter). Returns bytes regardless of format — the
 * caller decodes if needed.
 *
 * @example
 * ```ts
 * const format = (new URL(req.url)).searchParams.get("format") ?? "pdf";
 * const bytes = await convertTo(wasmModule, docxBytes, format as OutputFormat);
 * ```
 *
 * @throws {ConvertError}
 */
export async function convertTo(
  module: WebAssembly.Module,
  docx: Uint8Array,
  format: OutputFormat,
): Promise<Uint8Array> {
  const instance = await instantiate(module);
  switch (format) {
    case "pdf":
      return invokeExport(instance, "convert_wasm", docx);
    case "html":
      return invokeExport(instance, "convert_html_wasm", docx);
    case "markdown":
      return invokeExport(instance, "convert_md_wasm", docx);
    default: {
      // Exhaustiveness check.
      const _exhaustive: never = format;
      throw new TypeError(`unknown output format: ${String(_exhaustive)}`);
    }
  }
}

// --------------------------------------------------------------------------
// Low-level: pre-instantiated instance variants
// --------------------------------------------------------------------------

/**
 * Lower-level synchronous convert using an already-instantiated instance.
 *
 * Most users should prefer `convertToPdf`. Use this only if you have a
 * specific reason to manage instance lifetimes yourself. Beware: a single
 * trap leaves the instance unusable.
 */
export function convertWithInstance(
  instance: WebAssembly.Instance,
  docx: Uint8Array,
): Uint8Array {
  return invokeExport(instance, "convert_wasm", docx);
}

/** Synchronous HTML conversion against a pre-instantiated instance. */
export function convertHtmlWithInstance(
  instance: WebAssembly.Instance,
  docx: Uint8Array,
): string {
  return new TextDecoder().decode(
    invokeExport(instance, "convert_html_wasm", docx),
  );
}

/** Synchronous Markdown conversion against a pre-instantiated instance. */
export function convertMarkdownWithInstance(
  instance: WebAssembly.Instance,
  docx: Uint8Array,
): string {
  return new TextDecoder().decode(
    invokeExport(instance, "convert_md_wasm", docx),
  );
}

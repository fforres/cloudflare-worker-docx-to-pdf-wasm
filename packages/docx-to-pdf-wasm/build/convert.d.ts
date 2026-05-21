/**
 * Error thrown when the WASM converter returns a failure.
 *
 * The `message` field carries the upstream Rust error text (e.g.
 * "docx read: Opc(Zip(InvalidArchive(...)))" for malformed inputs).
 */
export declare class ConvertError extends Error {
    constructor(message: string);
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
export declare function instantiate(module: WebAssembly.Module): Promise<WebAssembly.Instance>;
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
export declare function convertToPdf(module: WebAssembly.Module, docx: Uint8Array): Promise<Uint8Array>;
/**
 * Alias for `convertToPdf`. Kept as the canonical, back-compatible name —
 * this is the function exported as `convert` from the package.
 */
export declare const convert: typeof convertToPdf;
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
export declare function convertToHtml(module: WebAssembly.Module, docx: Uint8Array): Promise<string>;
/** Same as `convertToHtml` but returns raw UTF-8 bytes (zero-copy if you stream it out). */
export declare function convertToHtmlBytes(module: WebAssembly.Module, docx: Uint8Array): Promise<Uint8Array>;
/**
 * Convert DOCX bytes to a Markdown document (UTF-8 CommonMark + GFM tables).
 *
 * Note: `rdocx-markdown` 0.1.2 emits headings as `**bold**` rather than ATX
 * `# Heading` — this is a known upstream limitation. Body paragraphs, lists,
 * and tables are correct.
 *
 * @throws {ConvertError}
 */
export declare function convertToMarkdown(module: WebAssembly.Module, docx: Uint8Array): Promise<string>;
/** Same as `convertToMarkdown` but returns raw UTF-8 bytes. */
export declare function convertToMarkdownBytes(module: WebAssembly.Module, docx: Uint8Array): Promise<Uint8Array>;
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
export declare function convertTo(module: WebAssembly.Module, docx: Uint8Array, format: OutputFormat): Promise<Uint8Array>;
/**
 * Lower-level synchronous convert using an already-instantiated instance.
 *
 * Most users should prefer `convertToPdf`. Use this only if you have a
 * specific reason to manage instance lifetimes yourself. Beware: a single
 * trap leaves the instance unusable.
 */
export declare function convertWithInstance(instance: WebAssembly.Instance, docx: Uint8Array): Uint8Array;
/** Synchronous HTML conversion against a pre-instantiated instance. */
export declare function convertHtmlWithInstance(instance: WebAssembly.Instance, docx: Uint8Array): string;
/** Synchronous Markdown conversion against a pre-instantiated instance. */
export declare function convertMarkdownWithInstance(instance: WebAssembly.Instance, docx: Uint8Array): string;

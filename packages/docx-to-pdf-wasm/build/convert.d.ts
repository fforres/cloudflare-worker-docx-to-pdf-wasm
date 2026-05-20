/**
 * Error thrown when the WASM converter returns a failure.
 *
 * The `message` field carries the upstream error text from the Rust side
 * (e.g. "docx read: Opc(Zip(InvalidArchive(...)))" for malformed inputs,
 * or "pdf render: ..." for layout/render errors).
 */
export declare class ConvertError extends Error {
    constructor(message: string);
}
/**
 * The minimal shape of the WebAssembly instance exports produced by this package's
 * Rust source. Exposed publicly so callers building exotic flows (e.g. worker-pool
 * setups that pre-instantiate) can reference the shape.
 */
export interface ConverterExports {
    memory: WebAssembly.Memory;
    alloc(size: number): number;
    dealloc(ptr: number, size: number): void;
    /**
     * Packs the result pointer and length into a single `i64`:
     * `(out_ptr << 32) | out_len`. `out_len === 0` signals failure;
     * the JS side then reads the error via `last_error_ptr` + `last_error_len`.
     */
    convert_wasm(ptr: number, len: number): bigint;
    last_error_ptr(): number;
    last_error_len(): number;
}
/**
 * Instantiate the converter `WebAssembly.Module`.
 *
 * Prefer creating a fresh instance per conversion (the high-level `convert()`
 * does this automatically). Some pathological inputs can trap inside Rust and
 * leave the linear memory in an undefined state — reusing such an instance
 * would silently produce garbage. The compiled `WebAssembly.Module` itself is
 * safe to hold at module scope, since compilation is the expensive part and
 * V8 / workerd keep the compiled artifact cached across instantiations.
 */
export declare function instantiate(module: WebAssembly.Module): Promise<WebAssembly.Instance>;
/**
 * Convert DOCX bytes to PDF bytes.
 *
 * Creates a fresh WASM instance per call — recommended for serverless or
 * per-request contexts. Keep the `WebAssembly.Module` at module scope so it's
 * compiled once and the cost of fresh instantiation stays low (typically
 * single-digit milliseconds after the first cold start).
 *
 * @example Cloudflare Workers
 * ```ts
 * import { convert } from "docx-to-pdf-wasm";
 * import wasmModule from "docx-to-pdf-wasm/wasm";
 *
 * export default {
 *   async fetch(req: Request) {
 *     const docx = new Uint8Array(await req.arrayBuffer());
 *     const pdf = await convert(wasmModule, docx);
 *     return new Response(pdf, { headers: { "content-type": "application/pdf" } });
 *   },
 * };
 * ```
 *
 * @example Node.js (>= 18)
 * ```ts
 * import { readFile } from "node:fs/promises";
 * import { createRequire } from "node:module";
 * import { convert } from "docx-to-pdf-wasm";
 *
 * const require = createRequire(import.meta.url);
 * const wasmBytes = await readFile(require.resolve("docx-to-pdf-wasm/wasm"));
 * const wasmModule = await WebAssembly.compile(wasmBytes);
 *
 * const docx = await readFile("input.docx");
 * const pdf = await convert(wasmModule, new Uint8Array(docx));
 * ```
 *
 * @throws {ConvertError} when the underlying Rust converter returns an error
 *   (e.g. malformed input, unsupported document).
 */
export declare function convert(module: WebAssembly.Module, docx: Uint8Array): Promise<Uint8Array>;
/**
 * Lower-level synchronous convert that uses an already-instantiated instance.
 *
 * Most users should prefer `convert()`. Use this only if you have a specific
 * reason to manage instance lifetimes yourself, and be aware that a trap
 * inside the converter will leave the instance unusable for subsequent calls.
 */
export declare function convertWithInstance(instance: WebAssembly.Instance, docx: Uint8Array): Uint8Array;

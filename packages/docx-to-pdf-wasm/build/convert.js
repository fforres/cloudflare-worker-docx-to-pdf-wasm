/**
 * Error thrown when the WASM converter returns a failure.
 *
 * The `message` field carries the upstream error text from the Rust side
 * (e.g. "docx read: Opc(Zip(InvalidArchive(...)))" for malformed inputs,
 * or "pdf render: ..." for layout/render errors).
 */
export class ConvertError extends Error {
    constructor(message) {
        super(message);
        this.name = "ConvertError";
    }
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
export async function instantiate(module) {
    return WebAssembly.instantiate(module, {});
}
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
export async function convert(module, docx) {
    const instance = await instantiate(module);
    return convertWithInstance(instance, docx);
}
/**
 * Lower-level synchronous convert that uses an already-instantiated instance.
 *
 * Most users should prefer `convert()`. Use this only if you have a specific
 * reason to manage instance lifetimes yourself, and be aware that a trap
 * inside the converter will leave the instance unusable for subsequent calls.
 */
export function convertWithInstance(instance, docx) {
    const exports = instance.exports;
    const { memory, alloc, dealloc, convert_wasm, last_error_ptr, last_error_len } = exports;
    // Copy the DOCX bytes into the WASM linear memory.
    const inPtr = alloc(docx.length);
    new Uint8Array(memory.buffer, inPtr, docx.length).set(docx);
    // packed = (out_ptr << 32) | out_len, where out_len === 0 signals failure.
    const packed = convert_wasm(inPtr, docx.length);
    const outPtr = Number(packed >> 32n);
    const outLen = Number(packed & 0xffffffffn);
    if (outLen === 0) {
        const errPtr = last_error_ptr();
        const errLen = Number(last_error_len());
        const msg = errLen > 0
            ? new TextDecoder().decode(new Uint8Array(memory.buffer, errPtr, errLen))
            : "(no error message)";
        dealloc(inPtr, docx.length);
        throw new ConvertError(msg);
    }
    // Copy out of the WASM memory before freeing it. The returned buffer is
    // independent of the WASM instance and safe to keep after the instance
    // is dropped.
    const pdf = new Uint8Array(memory.buffer, outPtr, outLen).slice();
    dealloc(inPtr, docx.length);
    dealloc(outPtr, outLen);
    return pdf;
}

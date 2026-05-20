//! Thin wrapper around `office2pdf` that exposes both a native CLI binary
//! (see `src/main.rs`) and a `cdylib` for `wasm32-unknown-unknown`.
//!
//! On native targets, callers usually go through the binary in `src/main.rs`.
//! On WASM, the upstream `office2pdf::wasm` module exports
//! `convertDocxToPdf` / `convertToPdf` etc. via `wasm-bindgen`; we re-export
//! a single small entry point so a Cloudflare Worker can call this crate
//! directly.

use office2pdf::config::{ConvertOptions, Format};

/// Convert a DOCX byte buffer to a PDF byte buffer. Works on every target,
/// including `wasm32-unknown-unknown`.
pub fn convert_docx(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let opts = ConvertOptions::default();
    office2pdf::convert_bytes(bytes, Format::Docx, &opts)
        .map(|r| r.pdf)
        .map_err(|e| e.to_string())
}

// Note: when built with `--features wasm` on `wasm32-unknown-unknown`,
// the underlying `office2pdf` crate's `wasm` feature already exports
// `convertDocxToPdf`, `convertToPdf`, etc. via `wasm-bindgen`, so a
// Cloudflare Worker can call those directly. We deliberately do NOT
// re-export them here — doing so causes duplicate-symbol link errors.
//
// If we ever want a different JS surface, we can rename the upstream
// exports by adding a feature flag on `office2pdf` upstream.

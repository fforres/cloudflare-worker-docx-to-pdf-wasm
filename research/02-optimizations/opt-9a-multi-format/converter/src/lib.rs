//! opt-9a: opt-8 + multi-format output (PDF, HTML, Markdown).
//!
//! opt-8 base: combines three workarounds for upstream rdocx limitations:
//!
//! 1. Avoids Carlito/Caladea, which trigger a ToUnicode-CMap drift bug in
//!    rdocx-pdf 0.1.2. Instead bundles Liberation Sans/Serif/Mono and
//!    registers them as aliases for every common Microsoft Office font.
//! 2. Subsets each bundled face at build time via pyftsubset (build.rs)
//!    to a Latin codepoint set — shrinks the WASM by ~1.8 MiB gzipped
//!    compared to shipping full TTFs.
//! 3. Pre-processes the DOCX XML to lift `<w:txbxContent>` paragraphs
//!    into the main `<w:body>` before handing the bytes to rdocx-oxml,
//!    so text-box / sidebar content is no longer silently dropped.
//!
//! Security hardening (post opt-9a security audit):
//! - Hard cap on input size, per-zip-part size, and `document.xml` element
//!   nesting depth (see `preprocess::{MAX_PART_BYTES, MAX_XML_DEPTH}` and
//!   `MAX_INPUT_BYTES` below).
//! - CDATA-aware XML scanner — `<![CDATA[ … ]]>` cannot smuggle fake
//!   `<w:txbxContent>` markers into the rewritten output.
//! - Fast-path early-out for documents that contain no textbox markers
//!   (≥ 88 % of the production corpus).
//! - WASM ABI `alloc` / `dealloc` use `std::alloc` with an explicit
//!   `Layout` instead of `Vec::from_raw_parts`, eliminating the latent UB
//!   that lived in opt-9a and earlier.

mod preprocess;

use std::alloc::{alloc as raw_alloc, dealloc as raw_dealloc, Layout};

/// Maximum DOCX input size accepted by the converter. Largest production
/// fixture is the 2 MB / 468-page NIST SP 800-53; 32 MiB is a deliberately
/// generous ceiling. Inputs above this are rejected before any unzipping
/// happens, protecting against trivial body-size denial-of-service.
pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug)]
pub enum ConvertError {
    /// Failed to parse the DOCX (malformed input, unsupported feature, or
    /// validation rejection from the preprocessor).
    Read(String),
    /// Failed to render the parsed document (rdocx layout / PDF emit).
    Render(String),
}

impl core::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConvertError::Read(m) => write!(f, "docx read: {m}"),
            ConvertError::Render(m) => write!(f, "pdf render: {m}"),
        }
    }
}

impl std::error::Error for ConvertError {}

// --- Bundled Liberation family (OFL). Avoiding Carlito/Caladea on purpose: ---
// --- those families trigger a wrong-ToUnicode bug in rdocx-pdf 0.1.2 that ---
// --- scrambles glyph IDs in the text layer (cmap drift).                  ---
// opt-6: pyftsubset trims each face to Latin/punct/currency coverage in build.rs;
// the resulting TTFs land in OUT_DIR/fonts and are embedded below.
const SANS_R: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSans-Regular.ttf"));
const SANS_B: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSans-Bold.ttf"));
const SANS_I: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSans-Italic.ttf"));
const SANS_BI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSans-BoldItalic.ttf"));
const SERIF_R: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSerif-Regular.ttf"));
const SERIF_B: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSerif-Bold.ttf"));
const SERIF_I: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSerif-Italic.ttf"));
const SERIF_BI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationSerif-BoldItalic.ttf"));
const MONO_R: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationMono-Regular.ttf"));
const MONO_B: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationMono-Bold.ttf"));
const MONO_I: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationMono-Italic.ttf"));
const MONO_BI: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fonts/LiberationMono-BoldItalic.ttf"));

#[derive(Copy, Clone)]
enum FamilyKind {
    Sans,
    Serif,
    Mono,
}

struct Alias {
    name: &'static str,
    family: FamilyKind,
}

const ALIASES: &[Alias] = &[
    // Sans
    Alias { name: "Liberation Sans", family: FamilyKind::Sans },
    Alias { name: "Calibri", family: FamilyKind::Sans },
    Alias { name: "Calibri Light", family: FamilyKind::Sans },
    Alias { name: "Arial", family: FamilyKind::Sans },
    Alias { name: "Helvetica", family: FamilyKind::Sans },
    Alias { name: "Tahoma", family: FamilyKind::Sans },
    Alias { name: "Verdana", family: FamilyKind::Sans },
    Alias { name: "Segoe UI", family: FamilyKind::Sans },
    Alias { name: "Trebuchet MS", family: FamilyKind::Sans },
    Alias { name: "Lucida Sans", family: FamilyKind::Sans },
    // Serif
    Alias { name: "Liberation Serif", family: FamilyKind::Serif },
    Alias { name: "Cambria", family: FamilyKind::Serif },
    Alias { name: "Times New Roman", family: FamilyKind::Serif },
    Alias { name: "Times", family: FamilyKind::Serif },
    Alias { name: "Georgia", family: FamilyKind::Serif },
    Alias { name: "Book Antiqua", family: FamilyKind::Serif },
    Alias { name: "Garamond", family: FamilyKind::Serif },
    Alias { name: "Palatino Linotype", family: FamilyKind::Serif },
    // Mono
    Alias { name: "Liberation Mono", family: FamilyKind::Mono },
    Alias { name: "Courier New", family: FamilyKind::Mono },
    Alias { name: "Courier", family: FamilyKind::Mono },
    Alias { name: "Consolas", family: FamilyKind::Mono },
    Alias { name: "Lucida Console", family: FamilyKind::Mono },
];

fn faces_for(kind: FamilyKind) -> [&'static [u8]; 4] {
    match kind {
        FamilyKind::Sans => [SANS_R, SANS_B, SANS_I, SANS_BI],
        FamilyKind::Serif => [SERIF_R, SERIF_B, SERIF_I, SERIF_BI],
        FamilyKind::Mono => [MONO_R, MONO_B, MONO_I, MONO_BI],
    }
}

/// Validate input size + run the textbox preprocessor. Returns the
/// (possibly rewritten) DOCX bytes to feed to rdocx, or a `ConvertError`
/// the caller should propagate.
fn validate_and_preprocess(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    if docx_bytes.len() > MAX_INPUT_BYTES {
        return Err(ConvertError::Read(format!(
            "input size {} exceeds MAX_INPUT_BYTES ({})",
            docx_bytes.len(),
            MAX_INPUT_BYTES
        )));
    }
    preprocess::preprocess_textboxes(docx_bytes)
        .map_err(|msg| ConvertError::Read(format!("preprocess: {msg}")))
}

/// Convert a DOCX byte slice into a PDF byte vector.
pub fn convert_to_pdf(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let preprocessed = validate_and_preprocess(docx_bytes)?;
    let doc = rdocx::Document::from_bytes(&preprocessed)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;

    let mut fonts: Vec<(&str, &[u8])> = Vec::with_capacity(ALIASES.len() * 4);
    for alias in ALIASES {
        let faces = faces_for(alias.family);
        for face in faces.iter() {
            fonts.push((alias.name, face));
        }
    }

    doc.to_pdf_with_fonts(&fonts)
        .map_err(|e| ConvertError::Render(format!("{e:?}")))
}

/// Back-compat alias for `convert_to_pdf`.
pub use convert_to_pdf as convert;

/// Convert a DOCX byte slice into HTML bytes (UTF-8). The textbox preprocessor
/// is applied first for consistency with the PDF path.
pub fn convert_to_html(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let preprocessed = validate_and_preprocess(docx_bytes)?;
    let doc = rdocx::Document::from_bytes(&preprocessed)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    Ok(doc.to_html().into_bytes())
}

/// Convert a DOCX byte slice into Markdown bytes (UTF-8). The textbox
/// preprocessor is applied first for consistency with the PDF path.
pub fn convert_to_markdown(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let preprocessed = validate_and_preprocess(docx_bytes)?;
    let doc = rdocx::Document::from_bytes(&preprocessed)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    Ok(doc.to_markdown().into_bytes())
}

// ---------- WASM ABI ----------
//
// Exports:
//   alloc(size)              -> *mut u8         : allocate `size` bytes
//   dealloc(ptr, size)                          : free; size MUST match alloc
//   convert_wasm(ptr, len)   -> u64             : DOCX -> PDF
//   convert_html_wasm(ptr, len) -> u64          : DOCX -> HTML bytes
//   convert_md_wasm(ptr, len)   -> u64          : DOCX -> Markdown bytes
//   last_error_ptr() -> *const u8               : per-thread error buffer
//   last_error_len() -> usize                   : length of the above
//
// All `convert_*_wasm` exports return a packed `u64`:
//   (out_ptr as u64) << 32 | (out_len as u64)
//
// `out_len == 0` signals failure. The caller may then read the error
// message via `last_error_ptr()` / `last_error_len()` — see safety note
// at those exports about pointer lifetime.

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use super::{convert_to_html, convert_to_markdown, convert_to_pdf, ConvertError};
    use super::{raw_alloc, raw_dealloc, Layout};
    use core::cell::RefCell;

    thread_local! {
        /// Per-thread error message buffer. Mutated in place by every
        /// `convert_*_wasm` call. **The pointer returned by
        /// `last_error_ptr()` is invalidated by the next `convert_*_wasm`
        /// or `alloc` call** — see the safety doc-comment on
        /// `last_error_ptr` below.
        static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    /// Build a `Layout` for `size` bytes of `u8`. Returns `None` for
    /// `size == 0` so callers can shortcut without going through the global
    /// allocator (whose zero-sized-allocation behaviour is platform-defined).
    #[inline]
    fn layout_for(size: usize) -> Option<Layout> {
        if size == 0 {
            return None;
        }
        Layout::array::<u8>(size).ok()
    }

    /// Allocate `size` bytes of WASM linear memory and return a pointer.
    ///
    /// Returns `core::ptr::null_mut()` if `size == 0` or if the size would
    /// overflow a Rust `Layout`. The caller must eventually pass the same
    /// `size` to `dealloc` for the allocation to be released soundly.
    #[unsafe(no_mangle)]
    pub extern "C" fn alloc(size: usize) -> *mut u8 {
        match layout_for(size) {
            // SAFETY: `Layout::array::<u8>(size)` returned Ok above, so the
            // layout is valid for the global allocator.
            Some(layout) => unsafe { raw_alloc(layout) },
            None => core::ptr::null_mut(),
        }
    }

    /// Release `size` bytes previously returned by `alloc(size)` or by a
    /// `convert_*_wasm` call (the output buffer length packed in the result).
    ///
    /// # Safety
    /// `ptr` must have been returned by a previous `alloc(size)` or
    /// `convert_*_wasm` call with the *exact same* `size`. Passing a
    /// mismatched size is undefined behaviour.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
        if ptr.is_null() {
            return;
        }
        if let Some(layout) = layout_for(size) {
            // SAFETY: the caller's contract is documented above.
            unsafe { raw_dealloc(ptr, layout) };
        }
    }

    fn run_convert<F>(ptr: *const u8, len: usize, f: F) -> u64
    where
        F: FnOnce(&[u8]) -> Result<Vec<u8>, ConvertError> + std::panic::UnwindSafe,
    {
        let input_ptr = ptr;
        let input_len = len;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let input = unsafe { core::slice::from_raw_parts(input_ptr, input_len) };
            f(input)
        }));

        match result {
            Ok(Ok(out)) => {
                // Shrink to exact length before forgetting — this makes
                // dealloc(out_ptr, out_len) sound under our std::alloc-based
                // allocator (Layout::array::<u8>(out_len) matches the actual
                // allocation).
                let mut boxed: Box<[u8]> = out.into_boxed_slice();
                let len = boxed.len();
                let out_ptr = boxed.as_mut_ptr();
                core::mem::forget(boxed);
                ((out_ptr as u64) << 32) | (len as u64)
            }
            Ok(Err(e)) => {
                let msg = format!("{e}").into_bytes();
                LAST_ERROR.with(|c| *c.borrow_mut() = msg);
                0
            }
            Err(payload) => {
                let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                    format!("panic: {s}")
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    format!("panic: {s}")
                } else {
                    "panic: (unknown)".to_string()
                };
                LAST_ERROR.with(|c| *c.borrow_mut() = msg.into_bytes());
                0
            }
        }
    }

    /// PDF — back-compat alias name kept identical to opt-8.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn convert_wasm(ptr: *const u8, len: usize) -> u64 {
        run_convert(ptr, len, convert_to_pdf)
    }

    /// HTML output. Returns packed (ptr<<32)|len; len==0 signals failure.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn convert_html_wasm(ptr: *const u8, len: usize) -> u64 {
        run_convert(ptr, len, convert_to_html)
    }

    /// Markdown output. Returns packed (ptr<<32)|len; len==0 signals failure.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn convert_md_wasm(ptr: *const u8, len: usize) -> u64 {
        run_convert(ptr, len, convert_to_markdown)
    }

    /// Pointer to the last error message UTF-8 bytes.
    ///
    /// # Pointer lifetime contract
    /// The returned pointer is valid **only until the next call to any of**:
    /// `convert_wasm`, `convert_html_wasm`, `convert_md_wasm`, `alloc`, or
    /// `dealloc`. Callers MUST copy the message into their own buffer before
    /// invoking any of those — the underlying buffer is a `thread_local!`
    /// `Vec<u8>` that gets overwritten in place on every conversion call.
    ///
    /// The current callers (the JS worker in `examples/cloudflare-worker/`
    /// and the Rust worker in `examples/rust-worker/`) read the error
    /// immediately after a failed convert and either tear down the WASM
    /// instance (JS) or return a `Response` (Rust) before invoking any
    /// further conversion, so this is safe in practice today. Future
    /// callers who pool / reuse instances must respect the contract above.
    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_ptr() -> *const u8 {
        LAST_ERROR.with(|c| c.borrow().as_ptr())
    }

    /// Length in bytes of the last error message. See `last_error_ptr` for
    /// the pointer-lifetime contract.
    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_len() -> usize {
        LAST_ERROR.with(|c| c.borrow().len())
    }
}

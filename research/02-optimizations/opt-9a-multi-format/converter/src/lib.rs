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
//! See `../foundissues/` for the upstream issues each workaround addresses.

mod preprocess;

#[derive(Debug)]
pub enum ConvertError {
    Read(String),
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

/// Convert a DOCX byte slice into a PDF byte vector.
pub fn convert_to_pdf(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    // Lift `<w:txbxContent>` paragraphs into `<w:body>` so rdocx-oxml sees them.
    // No-op (returns the original bytes) when the document has no textboxes.
    let preprocessed = preprocess::preprocess_textboxes(docx_bytes);
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
    let preprocessed = preprocess::preprocess_textboxes(docx_bytes);
    let doc = rdocx::Document::from_bytes(&preprocessed)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    Ok(doc.to_html().into_bytes())
}

/// Convert a DOCX byte slice into Markdown bytes (UTF-8). The textbox
/// preprocessor is applied first for consistency with the PDF path.
pub fn convert_to_markdown(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let preprocessed = preprocess::preprocess_textboxes(docx_bytes);
    let doc = rdocx::Document::from_bytes(&preprocessed)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    Ok(doc.to_markdown().into_bytes())
}

// ---------- WASM ABI ----------
//
// Exports:
//   alloc(size) -> *mut u8           : allocate `size` bytes, return ptr
//   dealloc(ptr, size)               : free
//   convert_wasm(ptr, len) -> u64    : convert; returns (out_ptr << 32) | out_len.
//                                     Out_len == 0 means failure.

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use super::{ConvertError, convert_to_html, convert_to_markdown, convert_to_pdf};
    use core::cell::RefCell;

    thread_local! {
        static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn alloc(size: usize) -> *mut u8 {
        let mut buf = Vec::<u8>::with_capacity(size);
        let ptr = buf.as_mut_ptr();
        core::mem::forget(buf);
        ptr
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, 0, size);
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
                let len = out.len();
                let mut boxed = out.into_boxed_slice();
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

    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_ptr() -> *const u8 {
        LAST_ERROR.with(|c| c.borrow().as_ptr())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_len() -> usize {
        LAST_ERROR.with(|c| c.borrow().len())
    }
}

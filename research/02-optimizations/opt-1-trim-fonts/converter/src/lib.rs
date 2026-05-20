//! Thin wrapper over `rdocx` exposing a single conversion function and a WASM export.

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

// Trimmed font set (opt-1). Carlito covers Calibri / Segoe UI / generic
// sans (rdocx's `map_font_name` already remaps these and the final fallback
// chain ends at "Carlito"). Liberation Serif covers Times New Roman /
// Cambria→Caladea / Georgia / generic serif. The `family` label below is
// unused for lookup — fontdb queries the TTF's embedded family name —
// but we keep it descriptive for clarity.
static CARLITO_REGULAR: &[u8] = include_bytes!("../fonts/Carlito-Regular.ttf");
static CARLITO_BOLD: &[u8] = include_bytes!("../fonts/Carlito-Bold.ttf");
static LIBERATION_SERIF_REGULAR: &[u8] =
    include_bytes!("../fonts/LiberationSerif-Regular.ttf");
static LIBERATION_SERIF_BOLD: &[u8] = include_bytes!("../fonts/LiberationSerif-Bold.ttf");

fn bundled_fonts() -> [(&'static str, &'static [u8]); 4] {
    [
        ("Carlito", CARLITO_REGULAR),
        ("Carlito", CARLITO_BOLD),
        ("Liberation Serif", LIBERATION_SERIF_REGULAR),
        ("Liberation Serif", LIBERATION_SERIF_BOLD),
    ]
}

/// Convert a DOCX byte slice into a PDF byte vector.
pub fn convert(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let doc = rdocx::Document::from_bytes(docx_bytes)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    let fonts = bundled_fonts();
    doc.to_pdf_with_fonts(&fonts)
        .map_err(|e| ConvertError::Render(format!("{e:?}")))
}

// ---------- WASM ABI ----------
//
// Exports:
//   alloc(size) -> *mut u8           : allocate `size` bytes, return ptr
//   dealloc(ptr, size)               : free
//   convert_wasm(ptr, len) -> u64    : convert; returns (out_ptr << 32) | out_len.
//                                     Out_len == 0 means failure.
//
// Caller (JS) writes DOCX bytes into the allocated buffer, calls convert_wasm,
// reads result via the returned ptr/len, then calls dealloc for both buffers.

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use super::convert;
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

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn convert_wasm(ptr: *const u8, len: usize) -> u64 {
        // Make sure panics produce a captured error rather than trapping the
        // isolate. catch_unwind requires std::panic + panic = "unwind" in the
        // release profile.
        let input_ptr = ptr;
        let input_len = len;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let input = unsafe { core::slice::from_raw_parts(input_ptr, input_len) };
            convert(input)
        }));

        match result {
            Ok(Ok(pdf)) => {
                let len = pdf.len();
                let mut boxed = pdf.into_boxed_slice();
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

    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_ptr() -> *const u8 {
        LAST_ERROR.with(|c| c.borrow().as_ptr())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn last_error_len() -> usize {
        LAST_ERROR.with(|c| c.borrow().len())
    }
}

//! Thin wrapper over `rdocx` with build-time subsetted fonts (opt-3).
//!
//! `bundled-fonts` is OFF on `rdocx-layout`; instead `build.rs` runs
//! `pyftsubset` to produce Latin-only TTFs into `$OUT_DIR/fonts/`, which we
//! embed and register via `to_pdf_with_fonts`.

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

// ---------- subset fonts (produced by build.rs) ----------
macro_rules! subset_font {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/fonts/", $name, ".ttf")).as_slice()
    };
}

fn subset_fonts() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("Carlito", subset_font!("Carlito-Regular")),
        ("Carlito", subset_font!("Carlito-Bold")),
        ("Carlito", subset_font!("Carlito-Italic")),
        ("Carlito", subset_font!("Carlito-BoldItalic")),
        ("Caladea", subset_font!("Caladea-Regular")),
        ("Caladea", subset_font!("Caladea-Bold")),
        ("Caladea", subset_font!("Caladea-Italic")),
        ("Caladea", subset_font!("Caladea-BoldItalic")),
        ("Liberation Sans", subset_font!("LiberationSans-Regular")),
        ("Liberation Sans", subset_font!("LiberationSans-Bold")),
        ("Liberation Sans", subset_font!("LiberationSans-Italic")),
        ("Liberation Sans", subset_font!("LiberationSans-BoldItalic")),
        ("Liberation Serif", subset_font!("LiberationSerif-Regular")),
        ("Liberation Serif", subset_font!("LiberationSerif-Bold")),
        ("Liberation Serif", subset_font!("LiberationSerif-Italic")),
        ("Liberation Serif", subset_font!("LiberationSerif-BoldItalic")),
        ("Liberation Mono", subset_font!("LiberationMono-Regular")),
        ("Liberation Mono", subset_font!("LiberationMono-Bold")),
        ("Liberation Mono", subset_font!("LiberationMono-Italic")),
        ("Liberation Mono", subset_font!("LiberationMono-BoldItalic")),
    ]
}

/// Convert a DOCX byte slice into a PDF byte vector.
pub fn convert(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let doc = rdocx::Document::from_bytes(docx_bytes)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    let fonts = subset_fonts();
    doc.to_pdf_with_fonts(&fonts)
        .map_err(|e| ConvertError::Render(format!("{e:?}")))
}

// ---------- WASM ABI ----------

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

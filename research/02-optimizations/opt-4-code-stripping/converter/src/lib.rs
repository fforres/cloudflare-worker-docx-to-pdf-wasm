//! Thin wrapper over (patched) `rdocx` exposing a single conversion function
//! and a WASM export. opt-4: rdocx is locally patched to drop rdocx-html,
//! regex, and tiny-skia / PNG rendering.

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

// Bundled 4-font set, only compiled in when the `bundled-fonts-min` feature
// is active. The no-fonts build is used for apples-to-apples vs the baseline
// "no bundled fonts" 0.65 MiB measurement.
#[cfg(feature = "bundled-fonts-min")]
static CARLITO_REGULAR: &[u8] = include_bytes!("../fonts/Carlito-Regular.ttf");
#[cfg(feature = "bundled-fonts-min")]
static CARLITO_BOLD: &[u8] = include_bytes!("../fonts/Carlito-Bold.ttf");
#[cfg(feature = "bundled-fonts-min")]
static LIBERATION_SERIF_REGULAR: &[u8] =
    include_bytes!("../fonts/LiberationSerif-Regular.ttf");
#[cfg(feature = "bundled-fonts-min")]
static LIBERATION_SERIF_BOLD: &[u8] = include_bytes!("../fonts/LiberationSerif-Bold.ttf");

#[cfg(feature = "bundled-fonts-min")]
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

    #[cfg(feature = "bundled-fonts-min")]
    {
        let fonts = bundled_fonts();
        return doc
            .to_pdf_with_fonts(&fonts)
            .map_err(|e| ConvertError::Render(format!("{e:?}")));
    }

    #[cfg(not(feature = "bundled-fonts-min"))]
    doc.to_pdf()
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

//! opt-2: zero fonts in WASM. Fonts are supplied at runtime by the caller via
//! a custom mini-format buffer (see `decode_fonts_buffer`).

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

/// Convert a DOCX byte slice into a PDF byte vector using only system / embedded fonts.
/// Without bundled fonts this will likely fail with FontNotFound on most docs;
/// kept for parity with the baseline ABI.
pub fn convert(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let doc = rdocx::Document::from_bytes(docx_bytes)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    doc.to_pdf()
        .map_err(|e| ConvertError::Render(format!("{e:?}")))
}

/// Convert a DOCX byte slice into a PDF using the caller-supplied fonts.
pub fn convert_with_fonts(
    docx_bytes: &[u8],
    fonts: &[(String, Vec<u8>)],
) -> Result<Vec<u8>, ConvertError> {
    let doc = rdocx::Document::from_bytes(docx_bytes)
        .map_err(|e| ConvertError::Read(format!("{e:?}")))?;
    // rdocx wants `&[(&str, &[u8])]`. Borrow from the owned vec.
    let borrowed: Vec<(&str, &[u8])> = fonts
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    doc.to_pdf_with_fonts(&borrowed)
        .map_err(|e| ConvertError::Render(format!("{e:?}")))
}

/// Decode the JS-side mini-format:
///   records := record*
///   record  := name_len (u32 LE) name_bytes data_len (u32 LE) data_bytes
pub fn decode_fonts_buffer(buf: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ConvertError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < buf.len() {
        if buf.len() - i < 4 {
            return Err(ConvertError::Read(
                "fonts buffer: truncated name_len".to_string(),
            ));
        }
        let name_len = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
        i += 4;
        if buf.len() - i < name_len {
            return Err(ConvertError::Read(
                "fonts buffer: name_len exceeds remaining".to_string(),
            ));
        }
        let name = std::str::from_utf8(&buf[i..i + name_len])
            .map_err(|e| ConvertError::Read(format!("fonts buffer: bad utf8 name: {e}")))?
            .to_string();
        i += name_len;
        if buf.len() - i < 4 {
            return Err(ConvertError::Read(
                "fonts buffer: truncated data_len".to_string(),
            ));
        }
        let data_len = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
        i += 4;
        if buf.len() - i < data_len {
            return Err(ConvertError::Read(
                "fonts buffer: data_len exceeds remaining".to_string(),
            ));
        }
        let data = buf[i..i + data_len].to_vec();
        i += data_len;
        out.push((name, data));
    }
    Ok(out)
}

// ---------- WASM ABI ----------
//
// Exports:
//   alloc(size) -> *mut u8
//   dealloc(ptr, size)
//   convert_wasm(docx_ptr, docx_len) -> u64
//   convert_with_fonts(docx_ptr, docx_len, fonts_ptr, fonts_len) -> u64
//   last_error_ptr() / last_error_len()

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use super::{convert, convert_with_fonts as rs_convert_with_fonts, decode_fonts_buffer};
    use core::cell::RefCell;

    thread_local! {
        static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    fn set_error(msg: String) {
        LAST_ERROR.with(|c| *c.borrow_mut() = msg.into_bytes());
    }

    fn pack(pdf: Vec<u8>) -> u64 {
        let len = pdf.len();
        let mut boxed = pdf.into_boxed_slice();
        let out_ptr = boxed.as_mut_ptr();
        core::mem::forget(boxed);
        ((out_ptr as u64) << 32) | (len as u64)
    }

    fn panic_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            format!("panic: {s}")
        } else if let Some(s) = payload.downcast_ref::<String>() {
            format!("panic: {s}")
        } else {
            "panic: (unknown)".to_string()
        }
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
            Ok(Ok(pdf)) => pack(pdf),
            Ok(Err(e)) => {
                set_error(format!("{e}"));
                0
            }
            Err(payload) => {
                set_error(panic_to_string(payload));
                0
            }
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn convert_with_fonts(
        docx_ptr: *const u8,
        docx_len: usize,
        fonts_ptr: *const u8,
        fonts_len: usize,
    ) -> u64 {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let docx = unsafe { core::slice::from_raw_parts(docx_ptr, docx_len) };
            let fonts_buf = unsafe { core::slice::from_raw_parts(fonts_ptr, fonts_len) };
            let fonts = decode_fonts_buffer(fonts_buf)?;
            rs_convert_with_fonts(docx, &fonts)
        }));

        match result {
            Ok(Ok(pdf)) => pack(pdf),
            Ok(Err(e)) => {
                set_error(format!("{e}"));
                0
            }
            Err(payload) => {
                set_error(panic_to_string(payload));
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

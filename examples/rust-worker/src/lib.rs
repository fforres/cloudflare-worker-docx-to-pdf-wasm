//! Pure-Rust Cloudflare Worker mirroring the JS worker in
//! `examples/cloudflare-worker/`. Both the HTTP routing and the DOCX → PDF /
//! HTML / Markdown conversion compile into a single WebAssembly module.
//!
//! Routes:
//!   GET  /                  → 200 banner
//!   GET  /health            → 200 banner
//!   POST /convert           → 200 application/pdf
//!   POST /convert/html      → 200 text/html; charset=utf-8
//!   POST /convert/markdown  → 200 text/markdown; charset=utf-8
//!   *                       → 404
//!
//! Observability: enabled in `wrangler.toml` under `[observability]` at 100%
//! head sampling. Cloudflare Workers Logs captures every invocation
//! automatically (method, path, status, CPU time, duration, colo) — no
//! application-side instrumentation needed for the standard request fields.

use approach_c_rdocx_opt9a::{
    convert_to_html, convert_to_markdown, convert_to_pdf, ConvertError,
};
use worker::*;

/// Defense-in-depth body-size cap. The converter has its own
/// `MAX_INPUT_BYTES = 32 MiB` guard; this layer rejects before bytes ever
/// reach the WASM-side allocator.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

const BANNER: &str = "\
docx-to-pdf-rust-worker

POST /convert            DOCX body → PDF
POST /convert/html       DOCX body → HTML
POST /convert/markdown   DOCX body → Markdown
";

#[event(fetch)]
async fn fetch(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();

    if path == "/" || path == "/health" {
        return banner();
    }

    if let Some(format) = parse_convert_path(path) {
        if req.method() != Method::Post {
            return text("Use POST with a .docx body", 405);
        }
        return handle_convert(&mut req, format).await;
    }

    text("Not found", 404)
}

#[derive(Copy, Clone)]
enum Format {
    Pdf,
    Html,
    Markdown,
}

fn parse_convert_path(path: &str) -> Option<Format> {
    match path {
        "/convert" => Some(Format::Pdf),
        "/convert/html" => Some(Format::Html),
        "/convert/markdown" => Some(Format::Markdown),
        _ => None,
    }
}

async fn handle_convert(req: &mut Request, format: Format) -> Result<Response> {
    // Fast-reject based on Content-Length when the client sends one.
    if let Some(declared) = req.headers().get("content-length")?
        .and_then(|s| s.parse::<usize>().ok())
    {
        if declared > MAX_BODY_BYTES {
            return text(
                &format!("payload too large ({} > {})", declared, MAX_BODY_BYTES),
                413,
            );
        }
    }

    let docx = match req.bytes().await {
        Ok(b) => b,
        Err(e) => return text(&format!("read body: {e}"), 400),
    };
    if docx.is_empty() {
        return text("empty body", 400);
    }
    if docx.len() > MAX_BODY_BYTES {
        return text(
            &format!("payload too large ({} > {})", docx.len(), MAX_BODY_BYTES),
            413,
        );
    }

    let (result, content_type) = match format {
        Format::Pdf => (convert_to_pdf(&docx), "application/pdf"),
        Format::Html => (convert_to_html(&docx), "text/html; charset=utf-8"),
        Format::Markdown => (
            convert_to_markdown(&docx),
            "text/markdown; charset=utf-8",
        ),
    };

    match result {
        Ok(bytes) => {
            let len = bytes.len();
            let mut resp = Response::from_bytes(bytes)?;
            let h = resp.headers_mut();
            h.set("content-type", content_type)?;
            h.set("content-length", &len.to_string())?;
            Ok(resp)
        }
        Err(e) => {
            let status: u16 = match e {
                ConvertError::Read(_) | ConvertError::Render(_) => 422,
            };
            text(&format!("{e}"), status)
        }
    }
}

fn banner() -> Result<Response> {
    let mut resp = Response::ok(BANNER)?;
    resp.headers_mut()
        .set("content-type", "text/plain; charset=utf-8")?;
    Ok(resp)
}

fn text(body: &str, status: u16) -> Result<Response> {
    let mut resp = Response::ok(body)?.with_status(status);
    resp.headers_mut()
        .set("content-type", "text/plain; charset=utf-8")?;
    Ok(resp)
}

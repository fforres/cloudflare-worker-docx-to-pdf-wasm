//! Approach A — minimal DOCX → PDF converter.
//!
//! Optimized for WASM binary size. Tier-1 only: paragraphs, lists, simple
//! inline runs. No tables, no images, no headers/footers.

use std::ops::Range;

use docx_rs::{DocumentChild, ParagraphChild, RunChild, read_docx};
use krilla::Document;
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::text::{Font, KrillaGlyph};

/// Bundled font. Liberation Serif Regular (OFL).
const LIBERATION_SERIF: &[u8] = include_bytes!("../fonts/LiberationSerif-Regular.ttf");

// --- Page geometry, in PDF points (1/72 inch) ---
// US Letter, 1" margins on all sides.
const PAGE_W: f32 = 612.0;
const PAGE_H: f32 = 792.0;
const MARGIN_L: f32 = 72.0;
const MARGIN_R: f32 = 72.0;
const MARGIN_T: f32 = 72.0;
const MARGIN_B: f32 = 72.0;
const TEXT_W: f32 = PAGE_W - MARGIN_L - MARGIN_R;

const BODY_SIZE: f32 = 12.0;
const HEADING_SIZE: f32 = 16.0;
const LINE_HEIGHT_MUL: f32 = 1.25;

#[derive(Debug)]
pub enum ConvertError {
    Parse(String),
    Pdf(String),
    Font,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Parse(s) => write!(f, "docx parse: {s}"),
            ConvertError::Pdf(s) => write!(f, "pdf emit: {s}"),
            ConvertError::Font => write!(f, "could not load bundled font"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// A line of text the layout has produced. `text` is the visible string;
/// `glyphs` is the parallel KrillaGlyph slice (advances normalized by upem).
struct LaidLine {
    text: String,
    glyphs: Vec<KrillaGlyph>,
    font_size: f32,
    width: f32,
}

/// A logical block before line-wrap.
#[derive(Default)]
struct Block {
    text: String,
    font_size: f32,
    space_after: f32,
}

pub fn convert(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let docx = read_docx(docx_bytes).map_err(|e| ConvertError::Parse(e.to_string()))?;

    // Build blocks from the document body.
    let mut blocks: Vec<Block> = Vec::new();
    let mut numbered_counter: u32 = 0;
    let mut last_was_numbered = false;

    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                // Determine list/heading from properties.
                let pp = &p.property;
                let is_heading = pp
                    .style
                    .as_ref()
                    .map(|s| s.val.starts_with("Heading"))
                    .unwrap_or(false);

                // List detection: numPr with numId set ⇒ list item.
                let mut is_list = false;
                let mut is_numbered = false;
                if let Some(numpr) = &pp.numbering_property {
                    if numpr.id.is_some() {
                        is_list = true;
                        // Heuristic: numId 0 is often bullets in docx-rs default
                        // numbering, but we can't know without numbering.xml.
                        // Best-effort: treat any numId > 0 as numbered.
                        let id = numpr.id.as_ref().map(|i| i.id).unwrap_or(0);
                        is_numbered = id > 0;
                    }
                }

                let mut text = String::new();
                collect_para_text(p.as_ref(), &mut text);

                if text.trim().is_empty() && !is_list {
                    // empty paragraph = vertical gap
                    blocks.push(Block {
                        text: String::new(),
                        font_size: BODY_SIZE,
                        space_after: BODY_SIZE * 0.5,
                    });
                    last_was_numbered = false;
                    continue;
                }

                if is_list {
                    if is_numbered {
                        if !last_was_numbered {
                            numbered_counter = 0;
                        }
                        numbered_counter += 1;
                        text = format!("{}. {}", numbered_counter, text);
                        last_was_numbered = true;
                    } else {
                        text = format!("• {}", text);
                        last_was_numbered = false;
                    }
                } else {
                    last_was_numbered = false;
                }

                let size = if is_heading { HEADING_SIZE } else { BODY_SIZE };
                blocks.push(Block {
                    text,
                    font_size: size,
                    space_after: size * 0.4,
                });
            }
            DocumentChild::Table(_) => {
                // Out of scope for the PoC; emit a placeholder so reviewers
                // can see the gap, and so text-recall still picks up nothing.
                // Don't add the placeholder text — it would inflate page count
                // without helping recall. Just skip.
            }
            _ => {}
        }
    }

    // Load font (krilla wraps the bytes; subsetting happens at serialize time).
    let font = Font::new(LIBERATION_SERIF.into(), 0).ok_or(ConvertError::Font)?;

    // ttf-parser face for glyph lookups + advances.
    let face = ttf_parser::Face::parse(LIBERATION_SERIF, 0).map_err(|_| ConvertError::Font)?;
    let upem = face.units_per_em() as f32;

    // Layout blocks → lines.
    let mut lines: Vec<LaidLine> = Vec::new();
    for b in &blocks {
        if b.text.is_empty() {
            lines.push(LaidLine {
                text: String::new(),
                glyphs: Vec::new(),
                font_size: b.font_size,
                width: 0.0,
            });
            continue;
        }
        wrap_into_lines(&b.text, b.font_size, &face, upem, TEXT_W, &mut lines);
        // Trailing space-after: one blank tiny line. Cheap hack.
        lines.push(LaidLine {
            text: String::new(),
            glyphs: Vec::new(),
            font_size: b.space_after / LINE_HEIGHT_MUL,
            width: 0.0,
        });
    }

    // Paginate and draw.
    let mut document = Document::new();
    let page_settings = PageSettings::from_wh(PAGE_W, PAGE_H).expect("valid page size");

    let mut page = document.start_page_with(page_settings.clone());
    let mut surface = page.surface();
    let mut y = MARGIN_T;

    for line in lines {
        let line_h = line.font_size * LINE_HEIGHT_MUL;
        if y + line_h > PAGE_H - MARGIN_B {
            // Page break
            surface.finish();
            page.finish();
            page = document.start_page_with(page_settings.clone());
            surface = page.surface();
            y = MARGIN_T;
        }
        if !line.glyphs.is_empty() {
            let baseline_y = y + line.font_size; // approximate ascent
            surface.draw_glyphs(
                Point::from_xy(MARGIN_L, baseline_y),
                &line.glyphs,
                font.clone(),
                &line.text,
                line.font_size,
                false,
            );
        }
        y += line_h;
    }

    surface.finish();
    page.finish();

    document
        .finish()
        .map_err(|e| ConvertError::Pdf(format!("{e:?}")))
}

fn collect_para_text(p: &docx_rs::Paragraph, out: &mut String) {
    for child in &p.children {
        match child {
            ParagraphChild::Run(run) => {
                for rc in &run.children {
                    match rc {
                        RunChild::Text(t) => out.push_str(&t.text),
                        RunChild::Tab(_) => out.push('\t'),
                        RunChild::Break(_) => out.push('\n'),
                        _ => {}
                    }
                }
            }
            ParagraphChild::Hyperlink(h) => {
                for hc in &h.children {
                    if let ParagraphChild::Run(run) = hc {
                        for rc in &run.children {
                            if let RunChild::Text(t) = rc {
                                out.push_str(&t.text);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Naive word-wrap: split on whitespace, measure word advance with ttf-parser,
/// place words greedily on each line.
fn wrap_into_lines(
    text: &str,
    font_size: f32,
    face: &ttf_parser::Face<'_>,
    upem: f32,
    max_width: f32,
    out: &mut Vec<LaidLine>,
) {
    // Also split on hard line breaks ('\n') we inserted from <w:br>.
    for hard_line in text.split('\n') {
        wrap_single_line(hard_line, font_size, face, upem, max_width, out);
    }
}

fn wrap_single_line(
    line: &str,
    font_size: f32,
    face: &ttf_parser::Face<'_>,
    upem: f32,
    max_width: f32,
    out: &mut Vec<LaidLine>,
) {
    if line.is_empty() {
        return;
    }

    // Pre-compute per-word glyph segments with their text byte ranges.
    // We will lay out the whole line as if it's one string, building glyphs
    // and tracking word boundaries to know where to break.
    let mut current_text = String::new();
    let mut current_glyphs: Vec<KrillaGlyph> = Vec::new();
    let mut current_width: f32 = 0.0;

    // Helper to push a word + possible leading space.
    let space_w = char_advance(' ', face, font_size, upem);

    let mut first_word = true;
    for word in line.split_whitespace() {
        // Compute width (offsets are recomputed when we actually emit).
        let word_w = measure_word(word, face, font_size, upem);

        let needed = if first_word { word_w } else { space_w + word_w };

        if !first_word && current_width + needed > max_width && !current_text.is_empty() {
            // Flush.
            out.push(LaidLine {
                text: std::mem::take(&mut current_text),
                glyphs: std::mem::take(&mut current_glyphs),
                font_size,
                width: current_width,
            });
            current_width = 0.0;
            first_word = true;
        }

        if !first_word {
            // Append a space glyph at the current end.
            let start = current_text.len();
            current_text.push(' ');
            let space_glyph = make_glyph(' ', face, upem, start..start + 1);
            current_glyphs.push(space_glyph);
            current_width += space_w;
        }
        let base = current_text.len();
        current_text.push_str(word);
        // Shape the word, anchored at `base`.
        let (wg, _ww) = shape_word(word, face, font_size, upem, base);
        current_glyphs.extend(wg);
        current_width += word_w;
        first_word = false;
    }

    if !current_text.is_empty() {
        out.push(LaidLine {
            text: current_text,
            glyphs: current_glyphs,
            font_size,
            width: current_width,
        });
    }
}

fn measure_word(
    word: &str,
    face: &ttf_parser::Face<'_>,
    font_size: f32,
    upem: f32,
) -> f32 {
    word.chars()
        .map(|c| char_advance(c, face, font_size, upem))
        .sum()
}

fn shape_word(
    word: &str,
    face: &ttf_parser::Face<'_>,
    font_size: f32,
    upem: f32,
    base: usize,
) -> (Vec<KrillaGlyph>, f32) {
    let mut glyphs = Vec::with_capacity(word.len());
    let mut total = 0.0;
    let mut byte_idx = base;
    for c in word.chars() {
        let len = c.len_utf8();
        let range = byte_idx..byte_idx + len;
        glyphs.push(make_glyph(c, face, upem, range));
        total += char_advance(c, face, font_size, upem);
        byte_idx += len;
    }
    (glyphs, total)
}

fn make_glyph(c: char, face: &ttf_parser::Face<'_>, upem: f32, range: Range<usize>) -> KrillaGlyph {
    let gid_u16 = face.glyph_index(c).map(|g| g.0).unwrap_or(0);
    let adv = face
        .glyph_hor_advance(ttf_parser::GlyphId(gid_u16))
        .unwrap_or(0) as f32;
    // Normalize advance by units-per-em (krilla expects normalized values).
    KrillaGlyph::new(
        krilla::text::GlyphId::new(gid_u16 as u32),
        adv / upem,
        0.0,
        0.0,
        0.0,
        range,
        None,
    )
}

fn char_advance(c: char, face: &ttf_parser::Face<'_>, font_size: f32, upem: f32) -> f32 {
    let gid = face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
    let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f32;
    adv / upem * font_size
}

// --- WASM C ABI -----------------------------------------------------------
//
// Exposed to the Cloudflare Worker host. The host:
//   1. calls `awd_alloc(len)` to get a pointer for the docx bytes,
//   2. writes the bytes there, then calls `awd_convert(ptr, len)`,
//   3. reads `awd_out_ptr()` / `awd_out_len()`,
//   4. frees with `awd_free(ptr, len)` and `awd_out_free()`.
//
// This is exposed unconditionally so the linker keeps the symbols and LTO
// retains the implementation crate on native too (cheap; doesn't hurt).

use std::cell::RefCell;

thread_local! {
    static OUTPUT: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// # Safety
/// `size` must be > 0. Returned pointer points to `size` uninitialized bytes.
#[no_mangle]
pub unsafe extern "C" fn awd_alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must have been returned from `awd_alloc(size)`.
#[no_mangle]
pub unsafe extern "C" fn awd_free(ptr: *mut u8, size: usize) {
    if !ptr.is_null() && size != 0 {
        drop(Vec::from_raw_parts(ptr, 0, size));
    }
}

/// Convert. Returns 0 on success, non-zero on error.
/// # Safety
/// `ptr` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn awd_convert(ptr: *const u8, len: usize) -> i32 {
    let input = std::slice::from_raw_parts(ptr, len);
    match convert(input) {
        Ok(pdf) => {
            OUTPUT.with(|o| *o.borrow_mut() = Some(pdf));
            0
        }
        Err(_) => 1,
    }
}

#[no_mangle]
pub extern "C" fn awd_out_ptr() -> *const u8 {
    OUTPUT.with(|o| {
        o.borrow()
            .as_ref()
            .map(|v| v.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn awd_out_len() -> usize {
    OUTPUT.with(|o| o.borrow().as_ref().map(|v| v.len()).unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn awd_out_free() {
    OUTPUT.with(|o| *o.borrow_mut() = None);
}

// --- wasm32 entry: a tiny custom getrandom stub so we don't pull JS bindings.
// krilla doesn't actually require randomness (PDF object IDs are sequential)
// but a few transitive deps might. Return a constant; if anything actually
// uses it, output won't be deterministic-safe but it's a PoC.
#[cfg(target_arch = "wasm32")]
mod wasm_random {
    use getrandom::register_custom_getrandom;
    fn stub(buf: &mut [u8]) -> Result<(), getrandom::Error> {
        for b in buf.iter_mut() {
            *b = 0x42;
        }
        Ok(())
    }
    register_custom_getrandom!(stub);
}

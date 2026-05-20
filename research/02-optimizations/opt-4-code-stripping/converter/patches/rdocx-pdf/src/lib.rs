//! PDF renderer for rdocx layout output.
//!
//! Converts `LayoutResult` (positioned page frames with glyph runs, lines,
//! rectangles, and images) into a PDF document.
//!
//! opt-4: the `raster`/PNG rendering paths and the `tiny-skia` dep are
//! stripped here. We only call `render_to_pdf`.

mod font;
mod image;
mod writer;

use rdocx_layout::LayoutResult;

/// Render a laid-out document to PDF bytes.
pub fn render_to_pdf(layout: &LayoutResult) -> Vec<u8> {
    writer::write_pdf(layout)
}

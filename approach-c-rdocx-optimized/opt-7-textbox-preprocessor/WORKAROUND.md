# Workaround — Textbox preprocessor

## What it does
A small Rust module (`converter/src/preprocess.rs`) opens the DOCX zip,
rewrites `word/document.xml`, and re-zips. The rewrite has four steps,
applied in order to the byte slice (no XML tree builds, no allocations
proportional to the parse depth):

1. **Strip `<mc:Fallback>` subtrees.** Word emits a Choice / Fallback pair
   that holds the same textbox content twice. Removing Fallback up front
   prevents us from lifting duplicates.
2. **Extract `<w:txbxContent>` inner content.** Concatenate every block's
   inner XML into a single buffer and erase the block from `document.xml`.
3. **Strip `<w:drawing>` and `<w:pict>` subtrees.** Once the textbox content
   has been extracted, the surviving drawing/pict containers are empty
   shells. Removing them keeps the rewrite tidy and avoids confusing the
   rdocx parser with half-graphics.
4. **Inject the extracted paragraphs** at the end of `<w:body>` — just
   before the closing `<w:sectPr>` if present, otherwise just before
   `</w:body>`.

The rewrite uses a small bespoke scanner (`find_open_tag`,
`find_matching_close`) that handles tag nesting and self-closing forms. We
do NOT depend on `quick-xml`'s tree builders for the body of the work; the
operation is byte-level and ~200 lines of Rust.

If no textboxes are found (`lifted_count == 0`) the preprocessor returns
the input bytes verbatim, so the function is safe to call on every DOCX.

## Public API
```rust
fn preprocess_textboxes(docx_bytes: &[u8]) -> Vec<u8>;

pub fn convert(docx_bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let preprocessed = preprocess::preprocess_textboxes(docx_bytes);
    let doc = rdocx::Document::from_bytes(&preprocessed)?;
    doc.to_pdf_with_fonts(&fonts)
}
```

Two new crates: `zip` (already present transitively via rdocx-opc) and
`quick-xml` (already transitively pulled by rdocx-oxml). Net Cargo.toml
change is two extra dependency lines, both pinning to versions already in
the dep graph.

## What is preserved
- All other DOCX parts (`word/styles.xml`, media, headers, relationships)
  are copied verbatim. Only `word/document.xml` is rewritten.
- The compression method of every entry is preserved exactly.
- Body paragraphs that were *not* inside a textbox are kept in their
  original position and order.

## What is lost
- **Sidebar / floating positioning.** Lifted paragraphs end up at the tail
  of the body. For text-extraction and AI-ingest pipelines this is fine.
- **Drawing shape geometry.** Once we lift the text, we drop the drawing
  wrapper, so the rounded rectangle / shape that hosted the text is no
  longer rendered.
- **Tables inside textboxes** (rare). We lift only `<w:p>` children of
  `<w:txbxContent>`. A future change could include `<w:tbl>` siblings.
- **Empty textboxes.** A textbox with no `<w:p>` content (pure decoration)
  is silently dropped along with its drawing wrapper.

## Failure modes
The preprocessor is defensive: any error during zip read, XML scan, or
re-zip is caught and the function returns the original bytes unchanged.
This means a malformed DOCX can never *worsen* the rdocx output through
preprocessing — at worst it stays as-is.

## Test coverage
`cargo test --release --lib preprocess` runs seven unit tests:

- `strip_simple_subtree` — non-nested removal
- `strip_nested_same_name` — nested same-name elements collapse correctly
- `strip_self_closing` — self-closing tags handled
- `extract_inner` — inner content captured across multiple blocks
- `inject_before_sectpr` — payload lands before `<w:sectPr>` not after
- `inject_no_sectpr` — graceful fallback when there's no `<w:sectPr>`
- `rewrite_real_shape` — full Choice/Fallback/drawing/pict scenario

All seven pass.

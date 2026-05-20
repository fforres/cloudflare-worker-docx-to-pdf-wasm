# Scorecard: office2pdf-native

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 1.00 | 0.00 | 0.0 | 223 |
| tier2 | 10 | 10 | 0.99 | 0.10 | 0.1 | 144 |
| tier3 | 10 | 9 | 0.91 | 0.00 | 0.4 | 136 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20560B time=556ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20856B time=116ms
- [tier1] **links.docx** — recall=1.0 pages=1/1 imgs=0/0 size=14094B time=125ms
- [tier1] **lists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13134B time=207ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=12310B time=110ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10889B time=111ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5119B time=112ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=10025B time=210ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=13658B time=215ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10576B time=109ms
- [tier2] **headers.docx** — recall=1.0 pages=1/1 imgs=0/0 size=18479B time=113ms
- [tier2] **inline_images.docx** — recall=1.0 pages=1/1 imgs=1/2 size=15028B time=122ms
- [tier2] **lists_level_override.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20108B time=214ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9065B time=115ms
- [tier2] **tables.docx** — recall=1.0 pages=1/1 imgs=0/0 size=15813B time=115ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=2743B time=111ms
- [tier3] **deep-table-cell.docx** — FAIL: `Error: converting "/Users/fforres/GITHUB/skyward/wasm-docx-to-pdf/fixtures/tier3/deep-table-cell.docx": parse error: Failed to parse DOCX (docx-rs): Failed to read from zip.`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=7670B time=224ms
- [tier3] **footnotes.docx** — recall=0.75 pages=1/1 imgs=0/0 size=7673B time=114ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=98992B time=113ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.909 pages=1/1 imgs=0/1 size=10075B time=112ms
- [tier3] **notes.docx** — recall=0.812 pages=1/1 imgs=0/0 size=12319B time=110ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10082B time=212ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=7014B time=117ms
- [tier3] **track_changes_insertion.docx** — recall=0.75 pages=1/1 imgs=0/0 size=6658B time=114ms

# Scorecard: opt-9a-security-toy

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 1.00 | 0.00 | 0.0 | 77 |
| tier2 | 10 | 10 | 0.99 | 0.10 | 0.0 | 63 |
| tier3 | 10 | 9 | 0.89 | 0.00 | 0.4 | 72 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13715B time=66ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13220B time=61ms
- [tier1] **links.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9618B time=66ms
- [tier1] **lists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=11515B time=85ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=7457B time=109ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9157B time=65ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=2795B time=58ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=9038B time=60ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=8078B time=61ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=6466B time=62ms
- [tier2] **headers.docx** — recall=1.0 pages=1/1 imgs=0/0 size=14469B time=69ms
- [tier2] **inline_images.docx** — recall=1.0 pages=1/1 imgs=2/2 size=8457B time=61ms
- [tier2] **lists_level_override.docx** — recall=0.946 pages=1/1 imgs=0/0 size=14174B time=66ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5595B time=62ms
- [tier2] **tables.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9572B time=68ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=717B time=63ms
- [tier3] **deep-table-cell.docx** — FAIL: `convert_wasm failed: docx read: preprocess: document.xml nesting depth 4097 exceeds MAX_XML_DEPTH (4096)`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=4325B time=104ms
- [tier3] **footnotes.docx** — recall=0.875 pages=1/1 imgs=0/0 size=5715B time=66ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=190373B time=72ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.909 pages=1/1 imgs=0/1 size=5952B time=67ms
- [tier3] **notes.docx** — recall=0.438 pages=1/1 imgs=0/0 size=8008B time=71ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9675B time=66ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=3908B time=74ms
- [tier3] **track_changes_insertion.docx** — recall=0.75 pages=1/1 imgs=0/0 size=3651B time=61ms

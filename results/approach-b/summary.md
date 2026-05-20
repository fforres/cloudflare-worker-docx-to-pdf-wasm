# Scorecard: approach-b

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 1.00 | 0.00 | 0.0 | 409 |
| tier2 | 10 | 10 | 0.99 | 0.10 | 0.1 | 1232 |
| tier3 | 10 | 9 | 0.91 | 0.00 | 0.4 | 482 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20560B time=186ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20856B time=255ms
- [tier1] **links.docx** — recall=1.0 pages=1/1 imgs=0/0 size=14094B time=220ms
- [tier1] **lists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13134B time=826ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=12310B time=560ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10889B time=553ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5119B time=587ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=10025B time=1604ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=13658B time=2825ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10576B time=1559ms
- [tier2] **headers.docx** — recall=1.0 pages=1/1 imgs=0/0 size=18479B time=1377ms
- [tier2] **inline_images.docx** — recall=1.0 pages=1/1 imgs=1/2 size=15028B time=1486ms
- [tier2] **lists_level_override.docx** — recall=1.0 pages=1/1 imgs=0/0 size=20108B time=1374ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9065B time=453ms
- [tier2] **tables.docx** — recall=1.0 pages=1/1 imgs=0/0 size=15813B time=498ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=2743B time=430ms
- [tier3] **deep-table-cell.docx** — FAIL: `error: convert: parse error: Failed to parse DOCX (docx-rs): Table nesting depth exceeded maximum limit.`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=7670B time=806ms
- [tier3] **footnotes.docx** — recall=0.75 pages=1/1 imgs=0/0 size=7673B time=349ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=98992B time=383ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.909 pages=1/1 imgs=0/1 size=10075B time=283ms
- [tier3] **notes.docx** — recall=0.812 pages=1/1 imgs=0/0 size=12319B time=389ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10082B time=901ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=7014B time=429ms
- [tier3] **track_changes_insertion.docx** — recall=0.75 pages=1/1 imgs=0/0 size=6658B time=369ms

# Scorecard: rdocx-wasm

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 0.97 | 0.00 | 0.0 | 48 |
| tier2 | 10 | 10 | 0.94 | 0.10 | 0.0 | 49 |
| tier3 | 10 | 9 | 0.77 | 0.00 | 0.4 | 47 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=1.0 pages=1/1 imgs=0/0 size=17645B time=48ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13509B time=47ms
- [tier1] **links.docx** — recall=0.842 pages=1/1 imgs=0/0 size=13360B time=47ms
- [tier1] **lists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=19318B time=49ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10684B time=48ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=17004B time=50ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5045B time=46ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=16355B time=48ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=12442B time=47ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=9081B time=46ms
- [tier2] **headers.docx** — recall=0.966 pages=1/1 imgs=0/0 size=22856B time=49ms
- [tier2] **inline_images.docx** — recall=1.0 pages=1/1 imgs=2/2 size=8611B time=56ms
- [tier2] **lists_level_override.docx** — recall=0.467 pages=1/1 imgs=0/0 size=26434B time=55ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10972B time=45ms
- [tier2] **tables.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13015B time=47ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=717B time=39ms
- [tier3] **deep-table-cell.docx** — FAIL: `m-function[5182]:0x149190`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=4519B time=45ms
- [tier3] **footnotes.docx** — recall=0.875 pages=1/1 imgs=0/0 size=10869B time=46ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=190373B time=53ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.0 pages=1/1 imgs=0/1 size=646B time=40ms
- [tier3] **notes.docx** — recall=0.438 pages=1/1 imgs=0/0 size=12727B time=49ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=19049B time=56ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=6933B time=48ms
- [tier3] **track_changes_insertion.docx** — recall=0.625 pages=1/1 imgs=0/0 size=6793B time=46ms

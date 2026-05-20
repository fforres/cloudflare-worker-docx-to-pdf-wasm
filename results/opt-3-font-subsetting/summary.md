# Scorecard: opt-3-font-subsetting

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 0.97 | 0.00 | 0.0 | 45 |
| tier2 | 10 | 10 | 0.94 | 0.10 | 0.0 | 46 |
| tier3 | 10 | 9 | 0.77 | 0.00 | 0.4 | 46 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=1.0 pages=1/1 imgs=0/0 size=14158B time=46ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=12597B time=45ms
- [tier1] **links.docx** — recall=0.842 pages=1/1 imgs=0/0 size=10232B time=46ms
- [tier1] **lists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=11857B time=47ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=7774B time=43ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10325B time=58ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=2958B time=40ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=10369B time=44ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=9076B time=45ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=6561B time=43ms
- [tier2] **headers.docx** — recall=0.966 pages=1/1 imgs=0/0 size=15481B time=44ms
- [tier2] **inline_images.docx** — recall=1.0 pages=1/1 imgs=2/2 size=8379B time=45ms
- [tier2] **lists_level_override.docx** — recall=0.467 pages=1/1 imgs=0/0 size=16623B time=47ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5897B time=44ms
- [tier2] **tables.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10088B time=45ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=717B time=39ms
- [tier3] **deep-table-cell.docx** — FAIL: `m-function[5181]:0x149470`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=4277B time=53ms
- [tier3] **footnotes.docx** — recall=0.875 pages=1/1 imgs=0/0 size=6338B time=43ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=190373B time=50ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.0 pages=1/1 imgs=0/1 size=646B time=38ms
- [tier3] **notes.docx** — recall=0.438 pages=1/1 imgs=0/0 size=8303B time=51ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10699B time=48ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=4400B time=49ms
- [tier3] **track_changes_insertion.docx** — recall=0.625 pages=1/1 imgs=0/0 size=4204B time=44ms

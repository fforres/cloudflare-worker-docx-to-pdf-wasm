# Scorecard: opt-4-code-stripping

| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |
|------|-------|----|--------------|--------------|-------------|--------|
| tier1 | 5 | 5 | 0.91 | 0.00 | 0.0 | 63 |
| tier2 | 10 | 10 | 0.91 | 0.10 | 0.0 | 110 |
| tier3 | 10 | 9 | 0.77 | 0.00 | 0.4 | 117 |

## Per-document results

- [tier1] **block_quotes.docx** — recall=0.98 pages=1/1 imgs=0/0 size=24478B time=48ms
- [tier1] **inline_formatting.docx** — recall=1.0 pages=1/1 imgs=0/0 size=24873B time=105ms
- [tier1] **links.docx** — recall=0.737 pages=1/1 imgs=0/0 size=16961B time=65ms
- [tier1] **lists.docx** — recall=0.85 pages=1/1 imgs=0/0 size=22701B time=52ms
- [tier1] **test.docx** — recall=1.0 pages=1/1 imgs=0/0 size=13635B time=46ms
- [tier2] **ComplexNumberedLists.docx** — recall=1.0 pages=1/1 imgs=0/0 size=16718B time=155ms
- [tier2] **EmptyDocumentWithHeaderFooter.docx** — recall=1.0 pages=1/1 imgs=0/0 size=5045B time=89ms
- [tier2] **SampleDoc.docx** — recall=1.0 pages=1/2 imgs=0/0 size=16351B time=122ms
- [tier2] **SimpleHeadThreeColFoot.docx** — recall=0.939 pages=1/2 imgs=0/0 size=14558B time=81ms
- [tier2] **Styles.docx** — recall=1.0 pages=1/1 imgs=0/0 size=11992B time=90ms
- [tier2] **headers.docx** — recall=0.966 pages=1/1 imgs=0/0 size=26091B time=53ms
- [tier2] **inline_images.docx** — recall=0.778 pages=1/1 imgs=2/2 size=11783B time=49ms
- [tier2] **lists_level_override.docx** — recall=0.467 pages=1/1 imgs=0/0 size=26020B time=187ms
- [tier2] **numbered_header.docx** — recall=1.0 pages=1/1 imgs=0/0 size=10558B time=98ms
- [tier2] **tables.docx** — recall=0.973 pages=1/1 imgs=0/0 size=17092B time=172ms
- [tier3] **TestTableColumns.docx** — recall=1.0 pages=1/1 imgs=0/0 size=717B time=75ms
- [tier3] **deep-table-cell.docx** — FAIL: `m-function[2373]:0x10b38d`
- [tier3] **drop_cap.docx** — recall=1.0 pages=1/1 imgs=0/0 size=7637B time=88ms
- [tier3] **footnotes.docx** — recall=0.875 pages=1/1 imgs=0/0 size=10867B time=75ms
- [tier3] **having-images.docx** — recall=1.0 pages=1/1 imgs=7/9 size=190373B time=110ms
- [tier3] **image_with_textbox_caption.docx** — recall=0.0 pages=1/1 imgs=0/1 size=646B time=44ms
- [tier3] **notes.docx** — recall=0.438 pages=1/1 imgs=0/0 size=15409B time=95ms
- [tier3] **table_header_rowspan.docx** — recall=1.0 pages=1/1 imgs=0/0 size=19050B time=158ms
- [tier3] **textbox_image.docx** — recall=1.0 pages=1/1 imgs=0/1 size=6935B time=284ms
- [tier3] **track_changes_insertion.docx** — recall=0.625 pages=1/1 imgs=0/0 size=6793B time=127ms

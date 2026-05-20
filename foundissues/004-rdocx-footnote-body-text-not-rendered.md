---
slug: rdocx-footnote-body-text-not-rendered
severity: medium
status: confirmed
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-oxml + rdocx-layout 0.1.2)
affects-versions:
  - rdocx-oxml 0.1.2
  - rdocx-layout 0.1.2
discovered-on: 2026-05-20
project-impact: |
  Multiple T3 toy-corpus docs (notes.docx recall 0.44, footnotes.docx recall 0.88,
  track_changes_insertion.docx 0.62). Real-world corpus less affected because
  documents in our 25-doc sample don't use footnotes heavily. Still a content-
  extraction gap for academic / legal / technical-spec documents that do.
workaround-in-this-repo: none
repro-available: yes
repro-fixture: fixtures/tier3/notes.docx
---

# rdocx-oxml does not render footnote / endnote body text into the PDF output

## Summary
DOCX stores footnote/endnote definitions in separate parts (`word/footnotes.xml`, `word/endnotes.xml`) referenced by `<w:footnoteReference>` / `<w:endnoteReference>` runs in the body. rdocx-oxml's body parser visits the references but doesn't fetch+emit the corresponding footnote body text. The rendered PDF has the reference superscript marker but no footnote area at the bottom of the page, and the footnote body text never reaches the text layer.

## Related to issue 002
Same architectural shape: the body walker doesn't follow links into separate-part containers. Issue 002 is about `<w:txbxContent>` (in-document floating frames). This issue is about `footnotes.xml` / `endnotes.xml` (separate OPC parts).

## Steps to reproduce
1. Input: `fixtures/tier3/notes.docx` (Pandoc's test fixture) or `fixtures/tier3/footnotes.docx` (Apache POI's).
2. Render via rdocx.
3. `pdftotext` output → references appear in the body (e.g. small numbers next to words) but the footnote text itself is absent.

## Suggested upstream fix
During `rdocx-oxml`'s body parse, when emitting a `Run` that contains a footnote/endnote reference, also enqueue the contents of the corresponding `<w:footnote w:id="...">` from `footnotes.xml` for downstream rendering. `rdocx-layout` then needs a target for placing those — bottom-of-page (footnote) or end-of-document (endnote).

## Repro artifacts in this repo
- `fixtures/tier3/notes.docx`, `fixtures/tier3/footnotes.docx`
- `reference-pdfs/tier3/notes.pdf` (LibreOffice — full footnotes)
- `results/opt-5-complex-corpus/...` (no footnote bodies in rdocx outputs)

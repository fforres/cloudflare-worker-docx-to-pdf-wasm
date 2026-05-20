---
slug: rdocx-textbox-content-not-extracted
severity: medium
status: confirmed
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-oxml 0.1.2)
affects-versions:
  - rdocx-oxml 0.1.2
  - rdocx-layout 0.1.2 (downstream impact)
discovered-on: 2026-05-20
project-impact: |
  1 / 25 documents in the complex corpus (UN SEEA policy brief) produces a PDF
  that has the correct page count but is empty of body text — all body text in
  that document lives in <w:txbxContent> inside <mc:AlternateContent>. Several
  more documents in the corpus had partial recall loss likely traceable to the
  same gap (sidebar widgets, image captions, pull-quotes).
workaround-in-this-repo: |
  None yet. Approach B (Typst) handles this correctly natively (0.89 recall vs
  rdocx's 0.00 on un_seea_policy_brief), so a Typst-based fallback for
  sidebar-heavy docs is one option. Otherwise: pre-process the DOCX XML to
  inline w:txbxContent into the main story flow before parsing.
repro-available: yes
repro-fixture: fixtures/complex/un_seea_policy_brief.docx
---

# rdocx-oxml does not extract body text from `<w:txbxContent>` (text boxes / sidebars / pull quotes)

## Summary
DOCX documents commonly place body text inside floating text boxes — `<w:txbxContent>` nested inside `<mc:AlternateContent>` or `<v:textbox>`. Examples: magazine-layout briefs, sidebar callouts, image captions positioned by frame, pull quotes. rdocx-oxml's body-parsing path does not visit these subtrees, so their text never reaches `rdocx-layout` and they don't appear in the rendered PDF.

The rendered PDF has the correct number of pages and any header/footer text, but the central body of those pages is blank.

## Steps to reproduce
1. Input: `fixtures/complex/un_seea_policy_brief.docx` (a 2-page UN policy brief, magazine layout with text in left/right sidebars and a centered figure).
2. Render to PDF via rdocx (`Document::from_bytes` → `to_pdf`).
3. `pdftotext` the output: returns essentially the title only. The substantive policy text in the sidebars is gone.
4. Compare to LibreOffice headless reference (`soffice --convert-to pdf`) — that one extracts everything.

## What we believe is happening
rdocx-oxml walks `w:body/w:p` and `w:body/w:tbl`. It does not descend into:
- `<mc:AlternateContent>/<mc:Choice>/<w:drawing>/.../<wps:txbx>/<w:txbxContent>/<w:p>` (modern Word text box)
- `<w:r>/<w:pict>/<v:shape>/<v:textbox>/<w:txbxContent>` (legacy VML text box)
- `<w:framePr>` paragraphs (rarer)

All of these are valid containers for body-text paragraphs.

## Possible upstream fix
Recursive walk during body parse: whenever an element type that can host text-box content is encountered, descend into the `<w:txbxContent>` and emit the contained `<w:p>` elements into the main flow (ideally with an anchor / floating hint, but for content-extraction purposes inline placement is fine).

## Local workaround idea
Pre-process the DOCX XML inside the worker before handing it to rdocx:

```python
# pseudocode
parse document.xml
for elem in iterall("//w:txbxContent"):
    move elem's <w:p> children to the end of <w:body>
    delete the original w:drawing / v:shape wrapper
write modified document.xml back into a new zip
```

This dumps textbox text at the end of the document — fidelity-poor but content-complete. Could be useful for the "extract content for AI analysis" pipeline. **Note: investigation of this workaround is one of the open tasks in this project — see `foundissues/004-rdocx-footnote-body-text-not-rendered.md` for the related gap.**

## Repro artifacts in this repo
- `fixtures/complex/un_seea_policy_brief.docx`
- `reference-pdfs/complex/un_seea_policy_brief.pdf` (LibreOffice — content extracts fine)
- `results/opt-5-complex-corpus/complex/un_seea_policy_brief.pdf` (rdocx — empty body)
- `results/approach-b-complex/complex/un_seea_policy_brief.pdf` (Typst — extracts the sidebar text, 0.89 recall)

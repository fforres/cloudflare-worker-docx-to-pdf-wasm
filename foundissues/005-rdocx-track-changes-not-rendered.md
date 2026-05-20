---
slug: rdocx-track-changes-not-rendered
severity: low
status: confirmed-scope-decision
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-oxml 0.1.2)
affects-versions:
  - rdocx-oxml 0.1.2
discovered-on: 2026-05-20
project-impact: |
  fixtures/tier3/track_changes_insertion.docx in our toy corpus has recall 0.62
  because inserted text (marked <w:ins>) is treated as ordinary content but
  some related markup may be skipped. Zero docs in the 25-doc real-world corpus
  are heavily track-changes-revised. Very low project impact in practice unless
  the worker is called by a workflow that uploads in-review documents.
workaround-in-this-repo: none
repro-available: yes
repro-fixture: fixtures/tier3/track_changes_insertion.docx
---

# rdocx does not render track-changes accept/reject visualisation; output represents the "accepted" view inconsistently

## Summary
Word's track-changes feature wraps inserted text in `<w:ins>` and deleted text in `<w:del>`. The PDF that LibreOffice produces from a track-changes document by default shows the changes (insertions underlined and coloured, deletions struck-through), while the "accept all changes" view just renders the post-revision text. rdocx does neither cleanly — `<w:ins>` content is inlined into the body but related formatting / colour cues are dropped, and `<w:del>` is sometimes included as plain text and sometimes omitted depending on the run structure.

The result on `track_changes_insertion.docx` is recall 0.62 — the doc has both inserted and deleted text and we keep some and lose some.

## Steps to reproduce
1. Input: `fixtures/tier3/track_changes_insertion.docx` (Pandoc test fixture).
2. Render via rdocx.
3. `pdftotext`: missing some sentences that the LibreOffice reference includes.

## Suggested upstream behaviour
At minimum: provide a `Document::accept_all_changes()` / `Document::reject_all_changes()` toggle that normalises the document into one of the two well-defined views before rendering. The current behaviour is between the two and is hard for callers to reason about.

## Repro artifacts in this repo
- `fixtures/tier3/track_changes_insertion.docx`
- `reference-pdfs/tier3/track_changes_insertion.pdf` (LibreOffice "show changes" view)
- `results/opt-5-complex-corpus/...` (rdocx — partial content loss)

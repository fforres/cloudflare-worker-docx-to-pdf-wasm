---
slug: caladea-suspected-cmap-bug
severity: unconfirmed-but-likely
status: hypothesis
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-pdf 0.1.2)
affects-versions:
  - rdocx-pdf 0.1.2 (likely same code path as issue 001)
  - rdocx-layout 0.1.2 (bundled-fonts feature)
discovered-on: 2026-05-20
project-impact: |
  Cannot directly attribute project impact yet. Caladea is the Cambria
  metric-compatible replacement and ships under rdocx-layout's bundled-fonts
  alongside Carlito. The architecture of Caladea's font file (Carolina Sergi's
  upstream) is closely modeled on Carlito. If issue 001's bug is in
  rdocx-pdf's CMap subset/encoding path and is sensitive to specific font
  table layouts that Carlito and Caladea share, Caladea will trip the same
  bug when rdocx falls back to it for Cambria-requesting documents.
workaround-in-this-repo: |
  Same as issue 001 — opt-5-complex-corpus aliases Cambria → LiberationSerif
  via to_pdf_with_fonts, bypassing Caladea entirely. So the bug is masked
  by our workaround even if it exists.
repro-available: not-yet
repro-fixture: "would need a Cambria-only document that does NOT use Calibri Light, to isolate the trigger"
---

# Suspected: rdocx-pdf produces the same scrambled ToUnicode CMap with bundled Caladea as with Carlito (issue 001)

## Summary
Issue 001 documents a confirmed bug in `rdocx-pdf` 0.1.2 where rendering with the bundled **Carlito** font produces a scrambled ToUnicode CMap. Caladea — the Cambria metric-compatible replacement also shipped under `rdocx-layout`'s `bundled-fonts` feature — shares architectural lineage and font-table layout with Carlito. We have not confirmed Caladea exhibits the same bug, but the prior probability is high.

## Why this is unconfirmed
Our 25-doc complex corpus contained Cambria-requesting documents (Cambria is one of MS Office's default heading fonts), but every one of them ALSO contained Calibri / Calibri-Light bodies that triggered the Carlito bug first. The corpus didn't have a clean "Cambria-only, no Calibri" repro.

## How to confirm
Create a minimal DOCX whose every run requests Cambria specifically (no Calibri family, no other fonts). Render with rdocx + bundled-fonts on. Compare PDF text layer to LibreOffice reference.

If text is scrambled in the same architectural way as Carlito (visually-correct shapes, wrong codepoints), the bug is confirmed and the fix path is the same as issue 001.

## What we'd write upstream (if/when confirmed)
Roll into issue 001 — same bug, broader trigger surface than initially documented.

## Repro artifacts in this repo
None yet. TODO: synthesise a single-page Cambria-only DOCX as `fixtures/tier3/cambria-only-cmap-test.docx` and run it through approach-c-rdocx (full fonts, no bundled-fonts override) to confirm or refute.

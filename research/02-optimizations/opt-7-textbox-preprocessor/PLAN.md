# opt-7 — Textbox preprocessor on top of opt-5

## Hypothesis
rdocx-oxml 0.1.2 drops everything inside `<w:txbxContent>` (DrawingML
`<wps:txbx>` or VML `<v:textbox>` containers). The
`fixtures/complex/un_seea_policy_brief.docx` document parks 100 % of its body
text in those containers, so opt-5 scores recall 0.00.

If we lift the `<w:p>` children of each `<w:txbxContent>` and inject them as
top-level paragraphs in `<w:body>` *before* handing the DOCX to rdocx, the
parser should see them as ordinary body paragraphs and emit them into the PDF.

## Strategy
1. Wrap opt-5's `convert()` with a `preprocess_textboxes()` step.
2. Preprocessor:
   - Open the DOCX as a zip.
   - Read `word/document.xml`.
   - Locate every `<w:txbxContent>...</w:txbxContent>` block, grab its inner
     `<w:p>` children.
   - Locate every `<mc:Fallback>...</mc:Fallback>` and drop it (avoids
     duplicate paragraphs — Word emits Choice + Fallback pairs).
   - Strip every `<w:drawing>...</w:drawing>` wrapper (we already grabbed the
     text; we don't want the empty container or its anchored shape).
   - Strip every `<w:pict>...</w:pict>` wrapper (legacy VML containers).
   - Append the collected paragraphs to the very end of `<w:body>` (before
     `<w:sectPr>` if present).
   - Re-zip with the modified `document.xml`.
3. Existing rdocx pipeline runs unmodified on the rewritten bytes.

## Acceptable scope reductions
- Position fidelity is lost — paragraphs end up at the document tail. Fine for
  text-extraction; we are not aiming for layout parity here.
- We handle modern (DrawingML `mc:AlternateContent`) and legacy (`v:textbox`)
  containers uniformly by scanning for the universal `<w:txbxContent>` tag
  name.
- We do NOT preserve textbox-internal tables. Only direct `<w:p>` children
  inside `<w:txbxContent>` are lifted. (Tables inside textboxes are rare;
  acceptable gap.)

## Validation
- Native build, run un_seea_policy_brief.docx through opt-7, assert
  `pdftotext` finds the missing strings ("Executive Summary", "Background",
  "Policy Recommendations", "References", etc.).
- Hard pass: text recall ≥ 0.60 on un_seea_policy_brief (currently 0.00).
- Run full opt-5 complex corpus through opt-7 and confirm no other doc
  regresses by more than ~2 percentage points.
- Worker test deferred unless time permits.

## Out of scope
- WASM build verification — only do if native works and time remains.
- Footnotes — different rdocx gap, tracked in foundissues/004.
- Fixing rdocx upstream — workaround only.

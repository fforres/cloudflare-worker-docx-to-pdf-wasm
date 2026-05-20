# Investigation — Why rdocx-oxml 0.1.2 drops textbox content

## tl;dr
`<w:txbxContent>` paragraphs never reach `BodyContent::Paragraph`. The body
parser captures them as opaque `RawXml` (preserved for round-trip) but they
are invisible to text extraction and to `rdocx-layout`/`rdocx-pdf`. The
preprocessor in this folder rewrites `word/document.xml` to lift those
paragraphs into the body before parsing.

## The document — un_seea_policy_brief.docx
Structure of the 115 KB `word/document.xml`:

| Where | Count of `<w:p>` |
|-------|------------------|
| Direct children of `<w:body>` | 34 (all empty wrappers — see below) |
| Inside `<w:txbxContent>` blocks | 22 (11 unique × Choice/Fallback pair) |
| Outside textboxes (substantive runs with `<w:t>`) | **0** |

Every body paragraph is a wrapper of the shape:

```
<w:p>
  <w:r>
    <w:rPr><w:noProof/></w:rPr>
    <mc:AlternateContent>
      <mc:Choice Requires="wps">
        <w:drawing>
          <wp:anchor …>
            <a:graphic>
              <a:graphicData uri=".../wordprocessingShape">
                <wps:wsp>
                  <wps:txbx>
                    <w:txbxContent>
                      <w:p>… real body paragraph …</w:p>
                    </w:txbxContent>
                  </wps:txbx>
                </wps:wsp>
              </a:graphicData>
            </a:graphic>
          </wp:anchor>
        </w:drawing>
      </mc:Choice>
      <mc:Fallback>
        <w:pict><v:shape><v:textbox>
          <w:txbxContent><w:p>… same paragraph, VML mirror …</w:p></w:txbxContent>
        </v:textbox></v:shape></w:pict>
      </mc:Fallback>
    </mc:AlternateContent>
  </w:r>
</w:p>
```

So a strict body-children walker sees 34 paragraphs that all extract to `""`.

## rdocx's parser path
Code references are to `rdocx-oxml-0.1.2` in the local cargo cache.

### `CT_Body::from_xml` (`document.rs:587`)
Loops through `<w:body>` children. Only `<w:p>` → `CT_P::from_xml` and
`<w:tbl>` → `CT_Tbl::from_xml` produce structured content; everything else
goes into `BodyContent::RawXml` (round-trippable but text-invisible).

In our case all 34 children *are* `<w:p>`, so they go through `CT_P`.

### `CT_P::from_xml` (`text.rs:304`)
Recognised children: `<w:pPr>`, `<w:r>`, `<w:hyperlink>`, `<w:fldSimple>`.
Everything else (incl. `<mc:AlternateContent>` if it appeared at paragraph
level) lands in `CT_P::extra_xml`.

### `CT_R::from_xml` (`text.rs:104`)
Recognised children: `<w:rPr>`, `<w:t>`, `<w:drawing>` plus empty
`<w:tab>`/`<w:br>`/`<w:footnoteReference>`/`<w:endnoteReference>`.
**`<mc:AlternateContent>` is not recognised**, so the whole subtree is
captured via `capture_element` and stored in `CT_R::extra_xml`. The content
is preserved but never traversed for text.

So `<mc:AlternateContent>` blocks always go opaque at the run level. There
is no path from `<mc:AlternateContent>` to `RunContent::Text`.

### What about runs that contain a bare `<w:drawing>`?
The drawing parser (`drawing.rs:675`) does descend into a `<w:drawing>`, but
only to look for `<wp:inline>` (image) or `<wp:anchor>` (anchored image).
`<wp:anchor>` parses positioning + `<a:blip>` (image embed id). Inside the
graphic the only recognised children are positionH / positionV / blip /
docPr — `<a:graphic>`, `<a:graphicData>`, `<wps:wsp>`, `<wps:txbx>`,
`<w:txbxContent>` all fall through with no handler. No `<w:p>` is ever
emitted from inside a drawing.

### Empirical confirmation
Built opt-5 (which uses the standard `rdocx::Document::from_bytes` → `to_pdf`
path) on the SEEA fixture: recall 0.000 — the rendered PDF contains zero
body tokens. After the opt-7 preprocessor it recovers to 0.929 (text-only;
the only missing tokens are the page numbers "1" and "2").

## Why the workaround is feasible
Because `<w:txbxContent>` is just a wrapper around `<w:p>` and `<w:tbl>`
elements that rdocx already knows how to parse, lifting them into the
top-level body is a one-pass byte rewrite — no real XML re-modelling needed.
The only risk is duplicating paragraphs: `<mc:Fallback>` mirrors the
`<mc:Choice>` content, so we strip Fallbacks first.

## What about other documents?
Among the other 24 docs in the complex corpus, three had small amounts of
textbox content (cdc_ngs_validation*, who_pqs_lab); their recall already
benefits slightly from opt-7 (+0.1 to +0.7 percentage points). The rest
have no textboxes and the preprocessor is a no-op (returns the input bytes
unchanged when `lifted_count == 0`).

## Remaining gaps (not addressed by opt-7)
1. **Footnote body text** — tracked separately in
   `foundissues/004-rdocx-footnote-body-text-not-rendered.md`. Different
   subsystem (`footnotes.xml`); preprocessor would need a different
   rewrite. Out of scope here.
2. **Tables inside textboxes** — we lift only `<w:p>` children, not
   `<w:tbl>`. In practice the inner content of `<w:txbxContent>` is almost
   always paragraphs; a future iteration could lift tables too with one
   extra line.
3. **Layout fidelity** — lifted paragraphs land at the end of the document
   (just before `<w:sectPr>`). They are extractable but lose their original
   sidebar positioning. For "search / AI ingest" pipelines this is fine.
4. **Glyph cmap drift** — pre-existing rdocx-pdf 0.1.2 issue affecting docs
   that heavily use the "fi" / "ffi" ligature glyphs. Unrelated to
   textboxes; visible in nasa_business_plan ("conflict" → "confict") on
   both opt-5 and opt-7.

---
slug: rdocx-deep-nested-tables-fail
severity: low
status: confirmed-deliberate-guard
suggested-upstream-target: https://github.com/tensorbee/rdocx/issues  (rdocx-oxml or rdocx-layout 0.1.2)
affects-versions:
  - rdocx 0.1.2 (probably the docx-rs-style table parser inherited via rdocx-oxml)
discovered-on: 2026-05-20
project-impact: |
  1 / 25 documents in the toy corpus (deep-table-cell.docx) fails to convert.
  All three approaches (A, B, C) fail on this document with the same root cause,
  so it's not specific to opt-5. Practically: documents with >N levels of nested
  tables produce HTTP 500 from the worker. No data loss; clean failure.
workaround-in-this-repo: |
  None. The worker returns 500 with the rdocx error message; the isolate stays
  healthy thanks to per-request WASM instantiation. Callers can detect and fall
  back to a different conversion pipeline.
repro-available: yes
repro-fixture: fixtures/tier3/deep-table-cell.docx
---

# rdocx (and approach B's docx-rs) reject deeply nested tables with a hard-coded depth limit

## Summary
`fixtures/tier3/deep-table-cell.docx` from Apache POI's test corpus has tables nested several levels deep (`<w:tbl>` inside a `<w:tc>` inside another `<w:tbl>` ...). Both rdocx 0.1.2 and the docx-rs version used by approach B reject it with `Table nesting depth exceeded maximum limit`. This appears to be a deliberate guard (against pathological / malicious input that could cause stack overflow) but is set conservatively enough to reject documents that real-world Word produces and LibreOffice handles.

## Steps to reproduce
1. Input: `fixtures/tier3/deep-table-cell.docx` (from Apache POI test-data).
2. Render via rdocx → returns `Err(Read(Opc(...) | DocumentParse("Table nesting depth exceeded maximum limit")))` (exact message form depends on which subcrate raises it).
3. LibreOffice headless renders the same file without issue.

## Why this is low-priority for us
- 1 test fixture out of 25 in toy corpus. Zero documents in the 25-doc real-world corpus exhibit this.
- Failure is clean: 500 from the worker, no isolate poisoning, valid error message.
- Trivial for a caller to fall back.

## Suggested upstream fix
Either:
- Make the depth limit configurable (e.g. a builder option on `Document::open` / `Document::from_bytes`), defaulting to current value but allowing power users to bump it.
- Replace the depth count with a "memory budget" guard — current `usize` counter is cheap and DoS-safe; only documents that *actually* hit pathological memory would fail.

## Repro artifacts in this repo
- `fixtures/tier3/deep-table-cell.docx`
- `results/opt-5-complex-corpus/` — would log this as the only T3 failure if the harness ran tier3

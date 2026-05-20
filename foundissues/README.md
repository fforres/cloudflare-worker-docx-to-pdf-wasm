# Found issues

A backlog of bugs / limitations / behavioural quirks discovered while building this project. Not filed upstream — explore individually and decide which deserve a real bug report.

Each file follows the same shape:
- YAML front-matter with metadata (severity, status, affected versions, repro, suggested upstream target).
- The body is written **as if it were the upstream issue** — i.e. paste it into a GitHub issue with minor edits and it's ready to file.

| # | Slug | Severity | Project impact |
|---|---|---|---|
| 001 | [rdocx-pdf-carlito-tounicode-cmap](001-rdocx-pdf-carlito-tounicode-cmap.md) | **HIGH** | Drove opt-5; before fix, scrambled text on 32% of real DOCX |
| 002 | [rdocx-textbox-content-not-extracted](002-rdocx-textbox-content-not-extracted.md) | MEDIUM | 1/25 doc in complex corpus produces empty PDF (UN SEEA brief) |
| 003 | [rdocx-deep-nested-tables-fail](003-rdocx-deep-nested-tables-fail.md) | LOW | 1 test fixture; deliberate guard |
| 004 | [rdocx-footnote-body-text-not-rendered](004-rdocx-footnote-body-text-not-rendered.md) | MEDIUM | T3 corpus outliers (recall 0.44–0.88) |
| 005 | [rdocx-track-changes-not-rendered](005-rdocx-track-changes-not-rendered.md) | LOW | Scope decision; documents with track changes lose those bits |
| 006 | [rdocx-opc-zip-features-break-wasm](006-rdocx-opc-zip-features-break-wasm.md) | LOW | Workaround in place via local Cargo patch |
| 007 | [caladea-suspected-cmap-bug](007-caladea-suspected-cmap-bug.md) | UNCONFIRMED | Same architecture as issue 001; not yet reproduced |

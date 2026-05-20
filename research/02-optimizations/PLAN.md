# Approach C — optimization variants

## Baseline (parent: `../approach-c-rdocx/`)
- 8.66 MiB raw WASM
- **4.04 MiB gzipped** (CF Workers wire size; 10 MiB ceiling)
- 22 TTF fonts (~6.8 MiB raw) dominate the bundle
- Code+deps: ~0.65 MiB gzipped (proven by no-bundled-fonts build)
- T1 recall 0.97 / T2 0.94 / T3 0.77; 5–7 ms warm worker conversion

## Where the budget goes
Fonts are ~85 % of the gzipped bundle. Code is ~15 %. **Fonts are the primary target.**

## Variants (4 in parallel)

| ID | Strategy | Bundle target | Risk |
|---|---|---|---|
| opt-1 | Bundle only Carlito Reg/Bold + Liberation Serif Reg/Bold (4 fonts) | ~1.0–1.5 MiB gz | Low |
| opt-2 | Zero fonts in WASM; fetch from R2 at top-level init | ~0.7 MiB WASM + fonts in R2 | Med (R2 wiring) |
| opt-3 | Build-time font subsetting to Latin codepoints (keep 22 families) | ~1.5–2.5 MiB gz | Med (build.rs) |
| opt-4 | Code stripping: drop rdocx-html, regex, image codecs | -100–300 KB; **stacks** | Low |

opt-1 + opt-4 likely produces the smallest realistic shippable bundle (~800 KB–1.2 MiB gz). opt-2 produces the smallest *WASM* but requires R2.

## Pass criteria for each variant (same as parent project)
- Native build OK
- WASM compiles for `wasm32-unknown-unknown`
- Compressed `.wasm` ≤ 10 MiB
- T1 recall ≥ 0.85, ≥ 5/5 OK
- T2 ≥ 6/10 OK
- Worker import-and-convert test passes (`wrangler dev` + curl)

## Folder convention (each variant)
```
opt-N-<name>/
├── PLAN.md                 # variant-specific plan
├── converter/              # forked Cargo project
│   ├── Cargo.toml
│   ├── src/lib.rs + bin.rs
│   └── patches/            # any rdocx-* patches
├── worker/                 # CF Worker importing this variant's WASM
│   ├── wrangler.toml
│   ├── package.json
│   └── src/{worker.js, converter.wasm}
└── RESULTS.md              # measured size, scorecard, worker test output
```

Each variant uses a unique wrangler port so tests don't collide:
- opt-1: 8788
- opt-2: 8789
- opt-3: 8790
- opt-4: 8791

## Fidelity expectations
- opt-1: minor regression possible on docs that reference non-Carlito/Liberation fonts (Cambria, Courier, etc.). Carlito is the Calibri replacement, Liberation Serif covers Times. Acceptable per project priorities ("content > font fidelity").
- opt-2: identical or better fidelity (more fonts available, just not embedded).
- opt-3: identical for Latin-only docs; degraded for any CJK/Cyrillic/Greek/Arabic content.
- opt-4: identical (only changing code paths we don't use).

## Out of scope here
- Combined variants (opt-1+opt-4 etc.) — decide after measuring each in isolation.
- Fidelity improvements (track changes, textbox text, footnotes) — separate work.
- Cold-start optimization (`wasm-opt --converge`, alternative compressors) — apply uniformly later.

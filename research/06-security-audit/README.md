# 06 — Security audit

External audit of opt-9a (`research/02-optimizations/opt-9a-multi-format/`) and the two example workers (`examples/cloudflare-worker/`, `examples/rust-worker/`). Six issues identified; all six verified against the current code and fixed.

- [`SECURITY-AUDIT-FIXES.md`](SECURITY-AUDIT-FIXES.md) — the per-issue writeup with fix sketches, unit tests added, and regression results.

## Headline

| Layer | Threat | Mitigation |
|---|---|---|
| Workers (JS + Rust) | Oversized body DoS | `MAX_BODY_BYTES = 32 MiB`, returns **413** before WASM is invoked |
| `lib.rs` entry | Same, defense-in-depth | `MAX_INPUT_BYTES = 32 MiB`, returns `ConvertError::Read` |
| `preprocess.rs` zip read | Zip-bomb (lying uncompressed size) | `MAX_PART_BYTES = 32 MiB` + bounded `Read::take` |
| `preprocess.rs` XML | Deep-nesting attack against `rdocx-oxml` | `MAX_XML_DEPTH = 4096` pre-check |
| `preprocess.rs` XML scanner | Tag injection / truncation via CDATA | Unified CDATA-aware `skip_special_section` |
| `lib.rs` WASM ABI | Latent UB in `alloc`/`dealloc` | Explicit `Layout::array::<u8>(size)` via `std::alloc` |
| `lib.rs` WASM ABI | `last_error_ptr` lifetime footgun | Documented contract |

## Net cost

- Bundle: 1.03 → 1.04 MiB gzipped (+1 KB)
- 18 unit tests pass (8 new)
- Corpus regression: 0 changes — toy 1.00/0.99/0.89, real-world 0.98 (matches pre-audit numbers exactly)

The new defenses run in O(n) time and O(1) memory beyond the input, with measured throughput >250 MB/s — they pay for themselves on the first textbox-free document.

## Reproducing the regression test

```bash
cd research/02-optimizations/opt-9a-multi-format/converter
cargo test --release
# 18 tests, 0 failed

cd /Users/fforres/GITHUB/skyward/wasm-docx-to-pdf
python3 research/harness/score.py        opt-9a-sec  research/02-optimizations/opt-9a-multi-format/wasm-runner.mjs
python3 research/harness/score_complex.py opt-9a-sec-complex  research/02-optimizations/opt-9a-multi-format/wasm-runner.mjs
```

# Phase 1 — Feasibility report: pure-Rust Cloudflare Worker

Date: 2026-05-20.
Target: a workers-rs Worker that calls the existing `approach_c_rdocx_opt8`
(rdocx-based) crate directly, compiled to a single WASM module that handles
both the HTTP routing and the DOCX→PDF conversion.

## 1. Current state of workers-rs (2026)

- crates.io: `worker = "0.8.3"` — latest release **2026-05-09** (10 days ago).
- Repo: <https://github.com/cloudflare/workers-rs>; first-party Cloudflare project,
  active development through 2025 and 2026.
- Companion crate `worker-macros = "0.8"` (re-exported via `worker::*`).
- Docs: <https://developers.cloudflare.com/workers/languages/rust/> and
  <https://docs.rs/worker/latest/worker/>.

Verdict: production-ready, actively maintained, fits our `cpu_ms = 30_000`
paid-plan footprint.

## 2. How it's built

- A workers-rs project is compiled by `worker-build` (currently `0.8.x`),
  which the template installs on demand via `cargo install`. It:
  1. Runs `cargo build --release --target wasm32-unknown-unknown`.
  2. Drives `wasm-bindgen` against the resulting `.wasm` to emit a JS glue
     module (`shim.mjs` / `index.js`) plus the trimmed WASM.
  3. Optionally runs `wasm-opt` for size.
- `wrangler` itself does **not** natively understand a workers-rs crate; the
  pattern is to set `[build] command = "worker-build --release"` and point
  `main = "build/worker/shim.mjs"` (or `"build/index.js"` on newer templates;
  the actual filename is what `worker-build` writes — we'll verify when we run
  it).
- wasm-bindgen is required for the `#[event(fetch)]` entry-point macro; it is
  **not** required of transitive dependencies.

## 3. Bundle size expectation

- A hello-world workers-rs worker post-wasm-bindgen + wasm-opt typically lands
  around **150–250 KiB raw / 80–130 KiB gzipped** (the workers-rs README and
  third-party benchmarks line up on that range; an empty `Response::ok` weighs
  almost nothing once `lto`, `opt-level = "z"` and `strip = true` are on).
- Our existing JS worker bundle is ~1 MiB gzipped, of which essentially all is
  the DOCX→PDF WASM (`docx-to-pdf.wasm`). The workers-rs shim cost on top of
  that is ~100 KiB gz — small relative to the converter itself.

## 4. Can workers-rs import a plain Rust library targeting wasm32-unknown-unknown?

**Yes.** The constraint documented in the workers-rs README is that *every
crate in the dependency tree must compile to* `wasm32-unknown-unknown` — i.e.
no `cc`/native deps, no missing `std` shims, no syscall-bound libraries. It is
**not** required for every dependency to itself use `wasm-bindgen`.

Our `approach_c_rdocx_opt8` crate:
- Compiles cleanly to `wasm32-unknown-unknown` today (it already produces
  `converter.wasm` for the JS worker via a hand-rolled ABI; we just won't use
  that ABI — workers-rs will call `convert()` directly).
- Has only pure-Rust transitive deps (`rdocx`, `rdocx-layout`, `quick-xml`,
  `zip` with `deflate-flate2`). All compile on `wasm32-unknown-unknown`.
- Has a `build.rs` that pre-subsets fonts — runs on the host, not in WASM, so
  the build target is irrelevant.
- Uses a `[patch.crates-io]` for `rdocx-opc`. **That patch must be repeated in
  the binary crate's `Cargo.toml`** (cargo only honors `[patch]` at the
  workspace root or the crate being built). The instructions call this out.

The path-dependency model is straightforward:

```toml
[dependencies]
approach_c_rdocx_opt8 = { path = "../../research/02-optimizations/opt-8-textbox-preprocessor-subset/converter", default-features = false }

[patch.crates-io]
rdocx-opc = { path = "../../research/02-optimizations/opt-8-textbox-preprocessor-subset/converter/patches/rdocx-opc" }
```

We disable the upstream `native` default feature because we don't need the
`approach_c_rdocx_opt8` `[[bin]]` to compile on the wasm target.

## 5. WASM bundle size budget

| Component                         | Estimate (gzipped) |
|-----------------------------------|--------------------|
| workers-rs shim + wasm-bindgen JS | ~50–100 KiB        |
| Empty Rust worker WASM            | ~80–130 KiB        |
| Our opt-8 converter logic + fonts | ~900 KiB           |
| **Total**                         | **~1.0–1.1 MiB**   |

Cloudflare Workers paid-plan ceiling is **10 MiB gzipped**, so we are at ~11 %.
Plenty of headroom even with opt-9a's HTML/Markdown converters bolted on.

## 6. Caveats

- **panic strategy.** workers-rs templates default to `panic = "abort"`, but
  the opt-8 crate uses `panic = "unwind"` so that `catch_unwind` works in the
  WASM ABI. workers-rs 0.8 supports unwind via `worker-build --panic-unwind`
  (and a matching `[profile.release] panic = "unwind"` in the binary crate).
  We'll use unwind for parity with the existing converter — that means our PDF
  errors stay recoverable, and a runtime trap surfaces as a JS exception rather
  than killing the isolate. (`catch_unwind` from inside `convert()` is still a
  no-op in our path because we call `convert()` directly, not through the
  hand-rolled WASM ABI — workers-rs's own `#[event(fetch)]` macro wraps the
  handler in something that surfaces panics as 500s.)
- **wasm-bindgen + LTO.** The opt-8 crate enables `lto = "fat"`,
  `codegen-units = 1`, `strip = true`. These compose with workers-rs without
  trouble; `wasm-bindgen` runs after the linker on the already-LTO'd binary.
- **`getrandom` / `instant` / system time.** The Workers JS runtime exposes
  `crypto` and `Date.now` but not the WASI clock. Neither `rdocx` nor `zip`
  nor `quick-xml` need a randomness source for normal conversion. If a
  transitive dep tries to call `time::SystemTime::now()` on the wasm target
  without a backend, it'll panic at runtime. We've already shipped opt-8 via
  the JS worker today, so we know this path is clean — no extra patches
  required.
- **First-byte cold start.** workers-rs cold starts are dominated by
  WebAssembly compilation. A ~5 MiB raw WASM module compiles in 30–80 ms on
  Workers' V8, well under the 400 ms startup budget.
- **wrangler workspace integration.** wrangler does *not* read pnpm workspaces
  for Rust crates; it just runs the `[build] command`. That means our new
  package only needs `package.json` for dev-tooling (wrangler binary), not for
  source linking.

## Verdict

**Proceed to Phase 2.** workers-rs 0.8.3 is in good shape, our existing opt-8
library is already wasm32-compatible without modification, and the size and
panic-strategy story all line up. The only operational risk is the first
`worker-build` invocation taking a long time on a cold `cargo install`; we'll
budget for that.

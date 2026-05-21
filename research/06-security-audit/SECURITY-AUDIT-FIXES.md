# Security audit response — opt-9a hardening

A pre-publication audit of `opt-9a-multi-format/converter/` and the two example workers surfaced six issues. All six were verified against the current code and fixed in this round. No corpus regression; bundle grew by ~1 KB gzipped.

## Summary

| # | Severity (audit) | Status | File(s) touched |
|---|---|---|---|
| 1 | DoS — wasted CPU on textbox-free docs | **fixed** | `preprocess.rs` |
| 2 | Injection — CDATA-smuggled fake tags | **fixed** | `preprocess.rs` |
| 3 | DoS — zip-bomb-shaped uncompressed size | **fixed** | `preprocess.rs`, `lib.rs`, both workers |
| 4 | DoS — deeply-nested XML against `rdocx-oxml` | **fixed** | `preprocess.rs` |
| 5 | Latent UB in WASM `alloc`/`dealloc` ABI | **fixed** | `lib.rs` |
| 6 | Footgun — `last_error_ptr` lifetime contract | **documented** | `lib.rs` |

Bundle size: **1.04 MiB gz** (was 1.03 MiB before fixes — +1 KB).
Toy corpus: T1 1.00 / T2 0.99 / T3 0.89 (unchanged).
Real-world corpus: 0.98 avg recall over 25 docs (unchanged).
18 unit tests pass, including 8 new tests covering the security regressions.

---

## Findings + fixes (in audit order)

### 1. Fast-path early-out in `rewrite_document_xml`

**Audit**: five sequential full-buffer copies (strip Fallback → extract txbx → strip drawing → strip pict → inject) ran before the `lifted_count == 0` early-out at the caller, so a textbox-free document still paid the full preprocessing cost — and a malicious input whose `document.xml` decompresses to 100 MB of `<w:p/>` tags peaked at 12.9 GB RSS / 28.5 s wallclock on native.

**Fix**: hoist a single `find_subslice(xml, b"<w:txbxContent")` check to the **top** of `rewrite_document_xml`. If the marker isn't present, return immediately with `lifted_count: 0` and an empty `xml`. The caller already returns the original bytes in that case; we just stop paying for the copies that produced nothing.

```rust
// Top of rewrite_document_xml — runs in ~250–650 MB/s on the input
if find_subslice(xml, b"<w:txbxContent").is_none() {
    return RewriteResult { xml: Vec::new(), lifted_count: 0 };
}
```

Verified by new unit test `fast_path_no_textboxes`. Affects 22 / 25 documents in the production real-world corpus.

### 2. CDATA-aware scanner

**Audit**: `find_open_tag` and `find_matching_close` correctly skipped `<!-- … -->` but not `<![CDATA[ … ]]>`. Two exploitable shapes:
- A CDATA section containing a fake `</w:txbxContent>` truncates a real textbox extraction, silently dropping content.
- A CDATA section containing a complete fake `<w:txbxContent> … </w:txbxContent>` pair gets lifted into the body, injecting attacker-controlled text into the rendered output.

**Fix**: introduce a single `skip_special_section(xml, i)` helper that handles CDATA, comments, `<!DOCTYPE …>` declarations, and `<?…?>` processing instructions uniformly. Both scanners consult it on every `<` they encounter.

```rust
fn skip_special_section(xml: &[u8], i: usize) -> Option<usize> {
    if starts_with(&xml[i..], b"<![CDATA[") {
        // jump past matching ]]>
        ...
    }
    if starts_with(&xml[i..], b"<!--") { ... }
    if xml[i + 1] == b'!' { ... }   // <!DOCTYPE …>
    if xml[i + 1] == b'?' { ... }   // <?xml … ?>
    None
}
```

New unit tests cover both attack shapes:
- `cdata_inside_does_not_match_open_tag` — a CDATA-embedded fake `<w:txbxContent>` must not lift "EVIL" into the body.
- `cdata_inside_does_not_break_matching_close` — a CDATA-embedded fake `</w:txbxContent>` must not truncate a real textbox extraction.

### 3. Zip-bomb-shaped uncompressed size + worker body cap

**Audit**: `Vec::with_capacity(f.size() as usize)` honored the ZIP central-directory's uncompressed-size field unconditionally. A ~1 KB DOCX can legally declare `u64::MAX` and trigger an instance-killing OOM trap before any conversion work begins.

**Fix (three layers, defense-in-depth)**:

1. **Preprocessor**: clamp the `with_capacity` *hint* to `MAX_PART_BYTES = 32 MiB` and bound the actual read with `Read::take(MAX_PART_BYTES + 1)`. If the post-read buffer exceeds the limit, return `PreprocessError::Reject` (hard reject — do NOT pass to rdocx).
2. **Library entry**: `convert_to_pdf` / `to_html` / `to_markdown` now check `docx_bytes.len() > MAX_INPUT_BYTES (32 MiB)` and return `ConvertError::Read` before the preprocessor is even invoked. So a 1 GB request never touches `Vec::with_capacity` in our code.
3. **Workers**: both `examples/cloudflare-worker/src/worker.js` and `examples/rust-worker/src/lib.rs` reject bodies over `MAX_BODY_BYTES = 32 MiB` with HTTP **413 Payload Too Large**, using the `Content-Length` header for an early reject when the client provides one.

Verified via curl: `dd if=/dev/urandom bs=1M count=35 | curl -X POST --data-binary @-` → `413` from both workers, no WASM execution.

### 4. XML depth budget against `rdocx-oxml`

**Audit**: a `document.xml` with 100,000 nested levels caused `rdocx-oxml` / `rdocx-pdf` to peak at ~1.6 GB RSS. The preprocessor's own scanner handles such input in 378 µs, but it didn't protect rdocx — the next stage in the pipeline.

**Fix**: add `check_xml_depth(xml, MAX_XML_DEPTH)` as a single linear pass over `document.xml`, called from `try_preprocess` before `rewrite_document_xml`. Tracks element depth with `<…>` open / `</…>` close, accounting for self-closing tags, attribute quoting, CDATA, comments, and PIs. If observed depth ever exceeds the budget (default `4096`), returns `PreprocessError::Reject` — the input never reaches rdocx.

`MAX_XML_DEPTH = 4096` is deliberately generous; real Word documents do not exceed ~30 levels of nesting even with table-in-table-in-table. The check is O(n) time, O(1) memory, and runs at ~250–650 MB/s — negligible cost per request.

New unit tests:
- `depth_check_accepts_normal_docs`
- `depth_check_rejects_pathological_nesting`
- `depth_check_ignores_cdata_and_comments`
- `depth_check_treats_self_closing_correctly`

### 5. Replace `Vec`-based ABI allocator with `std::alloc + Layout`

**Audit**: `alloc` used `Vec::<u8>::with_capacity(size)` + `core::mem::forget` and `dealloc` used `Vec::from_raw_parts(ptr, 0, size)`. The standard library explicitly says `Vec::with_capacity(n)` is *permitted* to allocate more than `n`, while `Vec::from_raw_parts` requires the capacity to *exactly match* what was allocated. Sound in practice today only because `RawVec` doesn't round up for `T = u8` — but latent UB by the book.

**Fix**: rewrite both exports against `std::alloc::{alloc, dealloc}` with an explicit `Layout::array::<u8>(size)`. The layout is fully determined by `size`, so alloc and dealloc trivially match. Plus the trivial null-pointer / zero-size guards.

```rust
fn layout_for(size: usize) -> Option<Layout> {
    if size == 0 { return None; }
    Layout::array::<u8>(size).ok()
}

pub extern "C" fn alloc(size: usize) -> *mut u8 {
    match layout_for(size) {
        Some(layout) => unsafe { raw_alloc(layout) },
        None => core::ptr::null_mut(),
    }
}

pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    if ptr.is_null() { return; }
    if let Some(layout) = layout_for(size) {
        unsafe { raw_dealloc(ptr, layout) };
    }
}
```

The output side (where `run_convert` returns a packed `(ptr, len)`) uses `Vec::into_boxed_slice()` to shrink to exact length before forgetting — so `Layout::array::<u8>(out_len)` in `dealloc` matches the box's actual allocation.

### 6. Document the `last_error_ptr` lifetime contract

**Audit**: `last_error_ptr` returns a raw pointer into the `thread_local!` `Vec<u8>` backing `LAST_ERROR`. Any subsequent `convert_*_wasm` or `alloc` call may reallocate that buffer, invalidating the pointer. Safe-by-coincidence in both current callers but a footgun for future maintainers.

**Fix (documentation)**: add a detailed safety section to `last_error_ptr` and `last_error_len` spelling out exactly which calls invalidate the pointer. The `LAST_ERROR` `thread_local!` block itself also gets a comment cross-referencing the contract:

```rust
/// Pointer to the last error message UTF-8 bytes.
///
/// # Pointer lifetime contract
/// The returned pointer is valid **only until the next call to any of**:
/// `convert_wasm`, `convert_html_wasm`, `convert_md_wasm`, `alloc`, or
/// `dealloc`. Callers MUST copy the message into their own buffer before
/// invoking any of those — the underlying buffer is a `thread_local!`
/// `Vec<u8>` that gets overwritten in place on every conversion call.
```

Note: the audit suggested an API-shaped fix (`take_last_error_into(out_ptr, out_cap) -> usize`) which would eliminate the footgun entirely. That's a future ABI change — for now both callers (JS worker, Rust worker) consume the error inline before any further conversion, so we accept the documentation-only fix.

---

## Verification

```
cargo test --release          → 18 passed, 0 failed
  (8 new: fast_path_no_textboxes, cdata_inside_does_not_match_open_tag,
   cdata_inside_does_not_break_matching_close, comments_still_skipped,
   depth_check_accepts_normal_docs, depth_check_rejects_pathological_nesting,
   depth_check_ignores_cdata_and_comments, depth_check_treats_self_closing_correctly,
   skip_cdata, skip_comment, skip_pi)

corpus regression (toy, 25 docs):       1.00 / 0.99 / 0.89   (unchanged)
corpus regression (real-world, 25 docs): 0.98 avg            (unchanged)

bundle size:  1.04 MiB gz   (was 1.03 MiB → +1 KB)

live worker smoke:
  POST /convert (cdc_ngs_validation.docx)  → 200, "Next Generation Sequencing"  ✓
  POST /convert/html                        → 200, valid HTML doc                ✓
  POST /convert (garbage body)              → 422 ConvertError                   ✓
  POST /convert (35 MiB random body)        → 413 Payload Too Large              ✓
```

The 35 MiB → 413 path exercises the new defense-in-depth: the worker layer rejects before any bytes hit WASM memory.

## Constants summary (knobs)

| Name | Value | Where |
|---|---|---|
| `MAX_INPUT_BYTES` | 32 MiB | `lib.rs` — total DOCX input cap |
| `MAX_PART_BYTES` | 32 MiB | `preprocess.rs` — per-zip-part cap |
| `MAX_XML_DEPTH` | 4096 | `preprocess.rs` — `document.xml` nesting depth cap |
| `MAX_BODY_BYTES` | 32 MiB | both workers — HTTP body cap (returns 413) |

Bump in concert if you ever need to handle very large enterprise documents — but the current values are well above the largest production fixture (2 MB / 468-page NIST SP 800-53), and the per-part / per-input caps protect linear memory more effectively than raising them would benefit any legitimate document.

//! DOCX preprocessor that lifts `<w:txbxContent>` paragraphs into the main
//! body. Works around the rdocx-oxml 0.1.2 gap where textbox content is never
//! emitted as `BodyContent::Paragraph` and therefore never reaches the PDF.
//!
//! Algorithm:
//!   1. Open the DOCX zip, read `word/document.xml` as bytes (with size guards
//!      against zip-bomb-shaped inputs).
//!   2. Run a depth budget pre-check on `document.xml`. Documents whose XML
//!      nests deeper than [`MAX_XML_DEPTH`] are rejected: they would trigger
//!      pathological memory in `rdocx-oxml` (see foundissues/003 territory).
//!   3. Fast-path: if `document.xml` contains no `<w:txbxContent` at all,
//!      return the original bytes unchanged — skip the 5 full-buffer copies
//!      below.
//!   4. Otherwise:
//!       a. Delete every `<mc:Fallback>...</mc:Fallback>` subtree (Word emits
//!          a Choice + Fallback pair around the same textbox; we keep the
//!          Choice side only).
//!       b. Collect the inner XML of every `<w:txbxContent>...</w:txbxContent>`
//!          block — those are the paragraphs that rdocx silently drops.
//!       c. Delete every `<w:drawing>...</w:drawing>` and
//!          `<w:pict>...</w:pict>` subtree — once the textbox text has been
//!          lifted we no longer need the empty drawing container.
//!       d. Inject the collected paragraphs at the end of `<w:body>` (just
//!          before the final `<w:sectPr>` if present, otherwise immediately
//!          before `</w:body>`).
//!   5. Re-zip every other part of the original DOCX verbatim, substituting
//!      the modified `document.xml`.
//!
//! Position fidelity is lost — lifted text ends up at the document tail — but
//! the content becomes extractable, which is the goal for AI / search
//! pipelines.
//!
//! ## Security
//! The scanner is CDATA-aware: `<![CDATA[ ... ]]>` sections are skipped
//! verbatim, so an attacker cannot inject fake `<w:txbxContent>` markers
//! into a CDATA section to force visible text to appear in the output, nor
//! truncate a legitimate textbox extraction. See unit tests
//! `cdata_inside_does_not_match_open_tag` and
//! `cdata_inside_does_not_break_matching_close` for the regression cases.
//!
//! Likewise, [`MAX_PART_BYTES`] and [`MAX_XML_DEPTH`] are hard upper bounds
//! that protect against zip-bomb and deep-nesting denial-of-service.

use std::io::{Cursor, Read, Write};

/// Maximum size (in bytes) of any single zip part we'll fully load into
/// memory. DOCX files in our 25-document real-world corpus top out around
/// 2 MB total; 32 MiB is a deliberately generous ceiling that protects
/// against malicious zip central directories that lie about uncompressed
/// sizes.
pub const MAX_PART_BYTES: usize = 32 * 1024 * 1024;

/// Maximum permitted XML element nesting depth in `document.xml`. Real Word
/// documents do not exceed ~30 levels even with table-in-table-in-table.
/// `rdocx-oxml` 0.1.2 has been observed to peak at ~1.6 GB RSS on a 100,000
/// level deep adversarial input — well past the Cloudflare Workers isolate
/// budget. 4,096 is generous for any legitimate document.
pub const MAX_XML_DEPTH: usize = 4096;

/// Top-level entry: take the original DOCX bytes, return rewritten bytes.
///
/// Error semantics:
/// - `Ok(bytes)` — either successfully preprocessed, or the input had no
///   textboxes and was returned unchanged. Either way, the bytes are safe
///   to hand to rdocx.
/// - `Err(message)` — the input was **rejected** for safety reasons
///   (oversized zip part, depth budget exceeded). The caller MUST NOT pass
///   the original bytes to rdocx in this case — that would defeat the
///   defense-in-depth purpose of this validation.
pub fn preprocess_textboxes(docx_bytes: &[u8]) -> Result<Vec<u8>, String> {
    match try_preprocess(docx_bytes) {
        Ok(out) => Ok(out),
        // Hard reject: validation failed in a DoS-shaped way.
        Err(PreprocessError::Reject(msg)) => Err(msg),
        // Soft failure (malformed zip etc.) — fall back to the original
        // bytes and let rdocx surface its own error.
        Err(PreprocessError::Soft(_)) => Ok(docx_bytes.to_vec()),
    }
}

#[derive(Debug)]
enum PreprocessError {
    Reject(String),
    Soft(String),
}

fn soft<E: std::fmt::Debug>(e: E) -> PreprocessError {
    PreprocessError::Soft(format!("{e:?}"))
}

fn try_preprocess(docx_bytes: &[u8]) -> Result<Vec<u8>, PreprocessError> {
    let mut zr = zip::ZipArchive::new(Cursor::new(docx_bytes)).map_err(soft)?;

    // Capture every entry verbatim. We'll later substitute document.xml.
    let mut entries: Vec<(String, Vec<u8>, zip::CompressionMethod)> = Vec::with_capacity(zr.len());
    let mut document_idx: Option<usize> = None;
    for i in 0..zr.len() {
        let mut f = zr.by_index(i).map_err(soft)?;
        let name = f.name().to_string();
        let method = f.compression();

        // Size guard 1: clamp the with_capacity *hint*. The zip central
        // directory's uncompressed-size field is attacker-controlled; a
        // ~1 KB DOCX can legally claim u64::MAX uncompressed bytes and
        // Vec::with_capacity would happily attempt to commit that much
        // linear memory before any conversion work begins.
        let claimed = f.size() as usize;
        let hint = claimed.min(MAX_PART_BYTES);
        let mut buf = Vec::with_capacity(hint);

        // Cap the actual read at MAX_PART_BYTES + 1 (the +1 lets us detect
        // "exactly at the limit" vs "exceeded the limit" deterministically).
        let mut limited = f.by_ref().take((MAX_PART_BYTES as u64) + 1);
        limited.read_to_end(&mut buf).map_err(soft)?;

        // Size guard 2: if the actual uncompressed stream exceeded our limit,
        // reject. We do not try to "best effort" past this — the document is
        // either malformed or hostile.
        if buf.len() > MAX_PART_BYTES {
            return Err(PreprocessError::Reject(format!(
                "zip part `{}` exceeds MAX_PART_BYTES ({} > {})",
                name,
                buf.len(),
                MAX_PART_BYTES
            )));
        }

        if name == "word/document.xml" {
            document_idx = Some(entries.len());
        }
        entries.push((name, buf, method));
    }

    let doc_idx = match document_idx {
        Some(i) => i,
        None => return Err(PreprocessError::Soft("word/document.xml missing".into())),
    };

    let original_xml = &entries[doc_idx].1;

    // Depth-budget pre-check: catches deeply-nested-XML attacks against
    // rdocx-oxml before the input ever reaches it.
    if let Err(d) = check_xml_depth(original_xml, MAX_XML_DEPTH) {
        return Err(PreprocessError::Reject(format!(
            "document.xml nesting depth {} exceeds MAX_XML_DEPTH ({})",
            d, MAX_XML_DEPTH
        )));
    }

    let rewritten = rewrite_document_xml(original_xml);

    // If we produced something with no detected textboxes, just return the
    // original DOCX bytes — no point re-zipping for nothing.
    if rewritten.lifted_count == 0 {
        return Ok(docx_bytes.to_vec());
    }

    entries[doc_idx].1 = rewritten.xml;

    // Re-zip.
    let mut out = Cursor::new(Vec::with_capacity(docx_bytes.len()));
    {
        let mut zw = zip::ZipWriter::new(&mut out);
        for (name, data, method) in &entries {
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
                .compression_method(*method)
                .large_file(false);
            zw.start_file(name, opts).map_err(soft)?;
            zw.write_all(data).map_err(soft)?;
        }
        zw.finish().map_err(soft)?;
    }
    Ok(out.into_inner())
}

/// Single linear pass over the XML counting element depth, skipping the
/// usual XML noise (declarations, comments, CDATA, processing instructions,
/// self-closing tags). Returns `Ok(())` if the document stays within
/// `limit`, or `Err(observed_max)` if depth ever exceeded the budget.
///
/// O(n) time, O(1) memory. ~250–650 MB/s on typical input.
fn check_xml_depth(xml: &[u8], limit: usize) -> Result<(), usize> {
    let n = xml.len();
    let mut i = 0usize;
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    while i < n {
        if xml[i] != b'<' {
            i += 1;
            continue;
        }
        // Try to skip XML "noise" (comments, CDATA, declarations, PIs) first.
        if let Some(next) = skip_special_section(xml, i) {
            i = next;
            continue;
        }
        // End tag?
        if i + 1 < n && xml[i + 1] == b'/' {
            depth = depth.saturating_sub(1);
            while i < n && xml[i] != b'>' {
                i += 1;
            }
            i += 1;
            continue;
        }
        // Open tag. Find '>' and check whether it's self-closing.
        let mut j = i + 1;
        let mut in_attr_quote: Option<u8> = None;
        while j < n {
            let c = xml[j];
            if let Some(q) = in_attr_quote {
                if c == q {
                    in_attr_quote = None;
                }
            } else if c == b'"' || c == b'\'' {
                in_attr_quote = Some(c);
            } else if c == b'>' {
                break;
            }
            j += 1;
        }
        if j >= n {
            // Truncated tag — stop counting, treat as well-formed up to here.
            return Ok(());
        }
        let self_closing = j > 0 && xml[j - 1] == b'/';
        if !self_closing {
            depth += 1;
            if depth > max_depth {
                max_depth = depth;
            }
            if depth > limit {
                return Err(depth);
            }
        }
        i = j + 1;
    }
    Ok(())
}

struct RewriteResult {
    xml: Vec<u8>,
    lifted_count: usize,
}

/// Pure-bytes rewrite of `document.xml`. Exposed for unit tests.
pub fn rewrite_document_xml_public(xml: &[u8]) -> Vec<u8> {
    rewrite_document_xml(xml).xml
}

fn rewrite_document_xml(xml: &[u8]) -> RewriteResult {
    // FAST PATH: most documents have no textboxes. Skip the five full-buffer
    // copies below if we can prove there isn't a single `<w:txbxContent`
    // marker anywhere. The marker check is a ~250–650 MB/s byte scan so it
    // pays for itself many times over on textbox-free inputs.
    if find_subslice(xml, b"<w:txbxContent").is_none() {
        return RewriteResult {
            xml: Vec::new(),
            lifted_count: 0,
        };
    }

    // Step 1: drop <mc:Fallback>...</mc:Fallback> first so we don't lift
    // duplicate paragraphs from Choice + Fallback pairs.
    let stripped_fallback = strip_subtrees(xml, &[b"mc:Fallback"]);

    // Step 2: extract <w:txbxContent>...</w:txbxContent> inner content.
    let mut lifted: Vec<u8> = Vec::new();
    let mut lifted_count = 0usize;
    let after_txbx_extract = extract_subtree_inner(
        &stripped_fallback,
        b"w:txbxContent",
        &mut lifted,
        &mut lifted_count,
    );

    // Step 3: drop now-empty <w:drawing> and <w:pict> wrappers.
    let stripped_drawings =
        strip_subtrees(&after_txbx_extract, &[b"w:drawing", b"w:pict"]);

    // Step 4: inject lifted paragraphs at end of <w:body>.
    let final_xml = inject_before_body_end(&stripped_drawings, &lifted);

    RewriteResult {
        xml: final_xml,
        lifted_count,
    }
}

/// Skip past a `<!-- comment -->`, `<![CDATA[ ... ]]>` block, `<?pi ... ?>`,
/// or `<!DOCTYPE ...>` declaration that *begins* at position `i`. Returns
/// the position immediately past the closing delimiter, or `None` if the
/// byte at `i` is not the start of such a section.
///
/// This is the single source of truth for "this byte range isn't real XML
/// markup" so that every scanner in this file behaves identically. In
/// particular it correctly skips CDATA — the previous implementation only
/// recognised comments, which allowed a crafted CDATA section to inject a
/// fake `<w:txbxContent>` literal into the rewrite output.
#[inline]
fn skip_special_section(xml: &[u8], i: usize) -> Option<usize> {
    let n = xml.len();
    if i + 1 >= n || xml[i] != b'<' {
        return None;
    }
    // <![CDATA[ ... ]]>
    if starts_with(&xml[i..], b"<![CDATA[") {
        let body_start = i + b"<![CDATA[".len();
        return match find_subslice(&xml[body_start..], b"]]>") {
            Some(off) => Some(body_start + off + b"]]>".len()),
            // Unterminated CDATA — treat as consuming the rest of the input.
            None => Some(n),
        };
    }
    // <!-- ... -->
    if starts_with(&xml[i..], b"<!--") {
        let body_start = i + 4;
        return match find_subslice(&xml[body_start..], b"-->") {
            Some(off) => Some(body_start + off + 3),
            None => Some(n),
        };
    }
    // <!DOCTYPE ...> or other <! declarations: scan to next '>'
    if xml[i + 1] == b'!' {
        let mut j = i;
        while j < n && xml[j] != b'>' {
            j += 1;
        }
        return Some(j.saturating_add(1).min(n));
    }
    // <?xml ... ?> or other PIs: scan to closing '?>'
    if xml[i + 1] == b'?' {
        let body_start = i + 2;
        return match find_subslice(&xml[body_start..], b"?>") {
            Some(off) => Some(body_start + off + 2),
            None => Some(n),
        };
    }
    None
}

/// Find the matching close tag for `<{name}` opened at `start` in `xml`,
/// accounting for nested same-name elements. `start` must point at the `<`
/// of the opening tag. Returns the byte index immediately past the matching
/// close tag (`</{name}>`).
///
/// CDATA-aware: `<![CDATA[ ... ]]>` contents are skipped verbatim, so a
/// literal `</name>` inside CDATA does not close the element.
fn find_matching_close(xml: &[u8], name: &[u8], start: usize) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = start;
    let open_prefix_a = make_prefix(b"<", name); // "<name"
    let close_prefix = make_close(name); // "</name>"
    let n = xml.len();
    while i < n {
        if xml[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip CDATA / comments / PIs before testing tag matches.
        if let Some(after) = skip_special_section(xml, i) {
            i = after;
            continue;
        }
        // </name>?
        if starts_with(&xml[i..], &close_prefix) {
            depth -= 1;
            i += close_prefix.len();
            if depth == 0 {
                return Some(i);
            }
            continue;
        }
        // <name ...> or <name> or <name/>?
        if starts_with(&xml[i..], &open_prefix_a) {
            // Confirm it's not a longer name (e.g. searching <w:p but tag is <w:pPr).
            let after = i + open_prefix_a.len();
            if after < n {
                let c = xml[after];
                if c == b' '
                    || c == b'>'
                    || c == b'/'
                    || c == b'\t'
                    || c == b'\n'
                    || c == b'\r'
                {
                    // Find end of tag, respecting attribute quoting.
                    let (end, self_closing) = find_open_tag_end(xml, after)?;
                    if !self_closing {
                        depth += 1;
                    }
                    i = end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    None
}

/// Given `xml[start..]` positioned just past a tag name (so the next byte is
/// either whitespace, `>`, or `/`), return `(index_of_closing_'>', self_closing)`.
/// Respects single- and double-quoted attribute values.
fn find_open_tag_end(xml: &[u8], start: usize) -> Option<(usize, bool)> {
    let n = xml.len();
    let mut j = start;
    let mut in_quote: Option<u8> = None;
    while j < n {
        let c = xml[j];
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            }
        } else if c == b'"' || c == b'\'' {
            in_quote = Some(c);
        } else if c == b'>' {
            let self_closing = j > 0 && xml[j - 1] == b'/';
            return Some((j, self_closing));
        }
        j += 1;
    }
    None
}

/// Find the opening tag `<{name}` (followed by whitespace, '>', or '/') in
/// `xml` starting at `from`, **skipping CDATA / comments / PIs**. Returns
/// `(start_of_lt, start_of_inner_after_open_tag, self_closing)`.
fn find_open_tag(xml: &[u8], name: &[u8], from: usize) -> Option<(usize, usize, bool)> {
    let prefix = make_prefix(b"<", name);
    let n = xml.len();
    let mut i = from;
    while i + prefix.len() <= n {
        if xml[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip XML noise before doing the literal-prefix match.
        if let Some(after) = skip_special_section(xml, i) {
            i = after;
            continue;
        }
        if starts_with(&xml[i..], &prefix) {
            let after_prefix = i + prefix.len();
            if after_prefix >= n {
                return None;
            }
            let c = xml[after_prefix];
            if c == b' '
                || c == b'>'
                || c == b'/'
                || c == b'\t'
                || c == b'\n'
                || c == b'\r'
            {
                let (j, self_closing) = find_open_tag_end(xml, after_prefix)?;
                let inner_start = j + 1;
                return Some((i, inner_start, self_closing));
            }
        }
        i += 1;
    }
    None
}

/// Remove every `<name>...</name>` subtree (and self-closing `<name/>` empties)
/// for each of the supplied names. Operates greedily from left to right;
/// nested same-name subtrees collapse with the outer one.
fn strip_subtrees(xml: &[u8], names: &[&[u8]]) -> Vec<u8> {
    let mut current = xml.to_vec();
    for &name in names {
        current = strip_one_subtree(&current, name);
    }
    current
}

fn strip_one_subtree(xml: &[u8], name: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(xml.len());
    let mut i = 0;
    while i < xml.len() {
        match find_open_tag(xml, name, i) {
            Some((lt, inner_start, self_closing)) => {
                out.extend_from_slice(&xml[i..lt]);
                if self_closing {
                    i = inner_start;
                } else {
                    match find_matching_close(xml, name, lt) {
                        Some(end) => i = end,
                        None => {
                            out.extend_from_slice(&xml[lt..]);
                            return out;
                        }
                    }
                }
            }
            None => {
                out.extend_from_slice(&xml[i..]);
                break;
            }
        }
    }
    out
}

/// Extract inner contents of every `<name>...</name>` subtree, appending them
/// to `out_inner`. Replace the original subtree in the returned XML with
/// nothing.
fn extract_subtree_inner(
    xml: &[u8],
    name: &[u8],
    out_inner: &mut Vec<u8>,
    out_count: &mut usize,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(xml.len());
    let mut i = 0;
    while i < xml.len() {
        match find_open_tag(xml, name, i) {
            Some((lt, inner_start, self_closing)) => {
                out.extend_from_slice(&xml[i..lt]);
                if self_closing {
                    i = inner_start;
                    continue;
                }
                match find_matching_close(xml, name, lt) {
                    Some(end) => {
                        let close_len = make_close(name).len();
                        let inner_end = end - close_len;
                        out_inner.extend_from_slice(&xml[inner_start..inner_end]);
                        *out_count += 1;
                        i = end;
                    }
                    None => {
                        out.extend_from_slice(&xml[lt..]);
                        return out;
                    }
                }
            }
            None => {
                out.extend_from_slice(&xml[i..]);
                break;
            }
        }
    }
    out
}

/// Inject `payload` immediately before `</w:body>`. If the body ends with
/// a `<w:sectPr>...</w:sectPr>` chunk, inject the payload before that chunk.
fn inject_before_body_end(xml: &[u8], payload: &[u8]) -> Vec<u8> {
    if payload.is_empty() {
        return xml.to_vec();
    }
    let close_body = b"</w:body>";
    let end = match find_subslice(xml, close_body) {
        Some(e) => e,
        None => return xml.to_vec(),
    };
    let body_open = match find_open_tag(xml, b"w:body", 0) {
        Some((_, after_open, _)) => after_open,
        None => 0,
    };
    let injection_point = match find_open_tag(xml, b"w:sectPr", body_open) {
        Some((lt, _, _)) if lt < end => lt,
        _ => end,
    };
    let mut out = Vec::with_capacity(xml.len() + payload.len() + 8);
    out.extend_from_slice(&xml[..injection_point]);
    out.extend_from_slice(payload);
    out.extend_from_slice(&xml[injection_point..]);
    out
}

#[inline]
fn make_prefix(lead: &[u8], name: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(lead.len() + name.len());
    v.extend_from_slice(lead);
    v.extend_from_slice(name);
    v
}

#[inline]
fn make_close(name: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(name.len() + 3);
    v.extend_from_slice(b"</");
    v.extend_from_slice(name);
    v.push(b'>');
    v
}

#[inline]
fn starts_with(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len() && &hay[..needle.len()] == needle
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let last = hay.len() - needle.len() + 1;
    let first = needle[0];
    let mut i = 0;
    while i < last {
        if hay[i] == first && &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(b: &[u8]) -> String {
        String::from_utf8(b.to_vec()).unwrap()
    }

    #[test]
    fn strip_simple_subtree() {
        let xml = b"<a><b>inner</b></a>";
        let out = strip_one_subtree(xml, b"b");
        assert_eq!(s(&out), "<a></a>");
    }

    #[test]
    fn strip_nested_same_name() {
        let xml = b"<root><x>1<x>2</x>3</x></root>";
        let out = strip_one_subtree(xml, b"x");
        assert_eq!(s(&out), "<root></root>");
    }

    #[test]
    fn strip_self_closing() {
        let xml = b"<a><img/><b>k</b></a>";
        let out = strip_one_subtree(xml, b"img");
        assert_eq!(s(&out), "<a><b>k</b></a>");
    }

    #[test]
    fn extract_inner() {
        let xml = b"<root><tc>aa<p>1</p></tc><tc><p>2</p></tc></root>";
        let mut inner = Vec::new();
        let mut count = 0;
        let out = extract_subtree_inner(xml, b"tc", &mut inner, &mut count);
        assert_eq!(s(&out), "<root></root>");
        assert_eq!(s(&inner), "aa<p>1</p><p>2</p>");
        assert_eq!(count, 2);
    }

    #[test]
    fn inject_before_sectpr() {
        let xml = b"<w:body><w:p/><w:sectPr></w:sectPr></w:body>";
        let payload = b"<w:p>LIFTED</w:p>";
        let out = inject_before_body_end(xml, payload);
        assert_eq!(
            s(&out),
            "<w:body><w:p/><w:p>LIFTED</w:p><w:sectPr></w:sectPr></w:body>"
        );
    }

    #[test]
    fn inject_no_sectpr() {
        let xml = b"<w:body><w:p/></w:body>";
        let out = inject_before_body_end(xml, b"<w:p>L</w:p>");
        assert_eq!(s(&out), "<w:body><w:p/><w:p>L</w:p></w:body>");
    }

    #[test]
    fn rewrite_real_shape() {
        let xml = br#"<w:body><w:p><w:r><mc:AlternateContent><mc:Choice><w:drawing><wps:txbx><w:txbxContent><w:p><w:r><w:t>Lifted</w:t></w:r></w:p></w:txbxContent></wps:txbx></w:drawing></mc:Choice><mc:Fallback><w:pict><v:textbox><w:txbxContent><w:p><w:r><w:t>Lifted</w:t></w:r></w:p></w:txbxContent></v:textbox></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p><w:sectPr></w:sectPr></w:body>"#;
        let result = rewrite_document_xml(xml);
        let out = String::from_utf8(result.xml).unwrap();
        assert_eq!(result.lifted_count, 1);
        assert!(out.contains("Lifted"), "lifted text not present: {out}");
        assert!(!out.contains("<w:drawing"), "drawing not stripped: {out}");
        assert!(!out.contains("<w:pict"), "pict not stripped: {out}");
        assert!(!out.contains("<w:txbxContent"), "txbxContent leftover: {out}");
    }

    // -- Fast path (issue 1) --

    #[test]
    fn fast_path_no_textboxes() {
        let xml = b"<w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body>";
        let r = rewrite_document_xml(xml);
        assert_eq!(r.lifted_count, 0);
        // Fast path returns an empty Vec — caller checks lifted_count first.
        assert!(r.xml.is_empty(), "fast path should not allocate a copy");
    }

    // -- CDATA awareness (issue 2) --

    #[test]
    fn cdata_inside_does_not_match_open_tag() {
        // Attacker hides a fake <w:txbxContent>…</w:txbxContent> inside CDATA.
        // If we matched it, an injected paragraph would appear in the output.
        let xml = b"<w:body><w:p><![CDATA[<w:txbxContent><w:p><w:r><w:t>EVIL</w:t></w:r></w:p></w:txbxContent>]]></w:p></w:body>";
        let r = rewrite_document_xml(xml);
        assert_eq!(r.lifted_count, 0, "must not lift content inside CDATA");
        // Fast path: no txbxContent token found *outside* CDATA, so the
        // marker scan returns None and we early-out. Both behaviours OK.
        if !r.xml.is_empty() {
            let out = String::from_utf8(r.xml).unwrap();
            assert!(
                !out.contains("EVIL") || out.contains("![CDATA["),
                "EVIL text must only appear inside its original CDATA, not lifted: {out}"
            );
        }
    }

    #[test]
    fn cdata_inside_does_not_break_matching_close() {
        // Real textbox followed by CDATA-embedded fake </w:txbxContent>.
        // Without the fix, the bogus close terminates the real one early
        // and the real lifted content is truncated.
        let xml = br#"<w:body><w:p><w:r><w:drawing><wps:txbx><w:txbxContent><w:p><w:r><w:t>REAL</w:t></w:r></w:p><![CDATA[</w:txbxContent>]]><w:p><w:r><w:t>AFTER</w:t></w:r></w:p></w:txbxContent></wps:txbx></w:drawing></w:r></w:p><w:sectPr/></w:body>"#;
        let r = rewrite_document_xml(xml);
        let out = String::from_utf8(r.xml).unwrap();
        assert_eq!(r.lifted_count, 1);
        assert!(out.contains("REAL"), "REAL must survive CDATA decoy: {out}");
        assert!(out.contains("AFTER"), "AFTER must survive CDATA decoy: {out}");
    }

    #[test]
    fn comments_still_skipped() {
        let xml = b"<root><!-- <w:txbxContent>fake</w:txbxContent> --><w:p/></root>";
        let r = rewrite_document_xml(xml);
        assert_eq!(r.lifted_count, 0, "fake textbox in comment must not lift");
    }

    // -- Depth budget (issue 4) --

    #[test]
    fn depth_check_accepts_normal_docs() {
        // Realistic depth: w:body > w:tbl > w:tr > w:tc > w:p > w:r > w:t
        let xml = b"<w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>ok</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body>";
        assert!(check_xml_depth(xml, MAX_XML_DEPTH).is_ok());
    }

    #[test]
    fn depth_check_rejects_pathological_nesting() {
        let mut xml = String::new();
        for _ in 0..(MAX_XML_DEPTH + 100) {
            xml.push_str("<x>");
        }
        // Don't bother closing them — the check rejects on opens.
        let r = check_xml_depth(xml.as_bytes(), MAX_XML_DEPTH);
        assert!(r.is_err(), "deep nesting should be rejected");
    }

    #[test]
    fn depth_check_ignores_cdata_and_comments() {
        // Lots of `<` characters but all inside CDATA — depth must stay at 0.
        let mut xml = String::from("<root><![CDATA[");
        for _ in 0..(MAX_XML_DEPTH + 100) {
            xml.push_str("<x");
        }
        xml.push_str("]]></root>");
        assert!(check_xml_depth(xml.as_bytes(), MAX_XML_DEPTH).is_ok());
    }

    #[test]
    fn depth_check_treats_self_closing_correctly() {
        let mut xml = String::from("<root>");
        for _ in 0..(MAX_XML_DEPTH + 100) {
            xml.push_str("<x/>");
        }
        xml.push_str("</root>");
        // Each <x/> is self-closing → depth never exceeds 2.
        assert!(check_xml_depth(xml.as_bytes(), MAX_XML_DEPTH).is_ok());
    }

    // -- skip_special_section coverage --

    #[test]
    fn skip_cdata() {
        let xml = b"a<![CDATA[<x>]]>b";
        // i=1 points at '<'
        let after = skip_special_section(xml, 1).unwrap();
        assert_eq!(&xml[after..], b"b");
    }

    #[test]
    fn skip_comment() {
        let xml = b"a<!--<x>-->b";
        let after = skip_special_section(xml, 1).unwrap();
        assert_eq!(&xml[after..], b"b");
    }

    #[test]
    fn skip_pi() {
        let xml = b"<?xml version='1'?>rest";
        let after = skip_special_section(xml, 0).unwrap();
        assert_eq!(&xml[after..], b"rest");
    }
}

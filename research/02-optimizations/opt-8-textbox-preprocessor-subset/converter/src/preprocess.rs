//! DOCX preprocessor that lifts `<w:txbxContent>` paragraphs into the main
//! body. Works around the rdocx-oxml 0.1.2 gap where textbox content is never
//! emitted as `BodyContent::Paragraph` and therefore never reaches the PDF.
//!
//! Algorithm:
//!   1. Open the DOCX zip, read `word/document.xml` as bytes.
//!   2. Delete every `<mc:Fallback>...</mc:Fallback>` subtree (Word emits a
//!      Choice + Fallback pair around the same textbox; we keep the Choice
//!      side only so we don't lift duplicates).
//!   3. Collect the inner XML of every `<w:txbxContent>...</w:txbxContent>`
//!      block. The inner content is always a sequence of `<w:p>` (and
//!      occasionally `<w:tbl>`) elements that rdocx already knows how to
//!      parse.
//!   4. Delete every `<w:drawing>...</w:drawing>` and `<w:pict>...</w:pict>`
//!      subtree — once the textbox text has been lifted we no longer need the
//!      empty drawing container, and removing it avoids confusing rdocx with
//!      half-parsed graphics.
//!   5. Inject the collected paragraphs at the end of `<w:body>` (just before
//!      the final `<w:sectPr>` if present, otherwise immediately before
//!      `</w:body>`).
//!   6. Re-zip every other part of the original DOCX verbatim, substituting
//!      our modified `document.xml`.
//!
//! Position fidelity is lost — lifted text ends up at the document tail —
//! but the content becomes extractable, which is the goal for AI / search
//! pipelines.

use std::io::{Cursor, Read, Write};

/// Top-level entry: take the original DOCX bytes, return rewritten bytes.
/// Falls back to returning the input unchanged on any error (best-effort).
pub fn preprocess_textboxes(docx_bytes: &[u8]) -> Vec<u8> {
    match try_preprocess(docx_bytes) {
        Ok(out) => out,
        Err(_) => docx_bytes.to_vec(),
    }
}

fn try_preprocess(docx_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut zr = zip::ZipArchive::new(Cursor::new(docx_bytes))?;

    // Capture every entry verbatim. We'll later substitute document.xml.
    let mut entries: Vec<(String, Vec<u8>, zip::CompressionMethod)> = Vec::with_capacity(zr.len());
    let mut document_idx: Option<usize> = None;
    for i in 0..zr.len() {
        let mut f = zr.by_index(i)?;
        let name = f.name().to_string();
        let method = f.compression();
        let mut buf = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut buf)?;
        if name == "word/document.xml" {
            document_idx = Some(entries.len());
        }
        entries.push((name, buf, method));
    }

    let doc_idx = match document_idx {
        Some(i) => i,
        None => return Err("word/document.xml missing".into()),
    };

    let original_xml = &entries[doc_idx].1;
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
            zw.start_file(name, opts)?;
            zw.write_all(data)?;
        }
        zw.finish()?;
    }
    Ok(out.into_inner())
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

/// Find the matching close tag for `<{name}` opened at `start` in `xml`,
/// accounting for nested same-name elements. `start` must point at the `<`
/// of the opening tag. Returns the byte index immediately past the matching
/// close tag (`</{name}>`).
///
/// Returns `None` if no balanced close can be found, or if `start` actually
/// pointed at a self-closing tag (which the caller should detect first).
fn find_matching_close(xml: &[u8], name: &[u8], start: usize) -> Option<usize> {
    // Walk forward looking for `<name ...>`, `<name>`, `</name>`. Honour
    // self-closing forms by skipping them (the caller should not have called
    // us in that case, but defend anyway).
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
                    // Find end of tag.
                    let mut j = after;
                    let mut self_closing = false;
                    while j < n && xml[j] != b'>' {
                        j += 1;
                    }
                    if j >= n {
                        return None;
                    }
                    if j > 0 && xml[j - 1] == b'/' {
                        self_closing = true;
                    }
                    if !self_closing {
                        depth += 1;
                    }
                    i = j + 1;
                    continue;
                }
            }
        }
        // Skip comments/CDATA/PI conservatively by hunting forward to '>'.
        if i + 1 < n && (xml[i + 1] == b'!' || xml[i + 1] == b'?') {
            // Find matching '-->' for comment or '>' otherwise.
            if starts_with(&xml[i..], b"<!--") {
                if let Some(end) = find_subslice(&xml[i + 4..], b"-->") {
                    i = i + 4 + end + 3;
                    continue;
                } else {
                    return None;
                }
            }
            // Generic: just hop to next '>'.
            while i < n && xml[i] != b'>' {
                i += 1;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    None
}

/// Find the opening tag `<{name}` (followed by whitespace, '>', or '/') in
/// `xml` starting at `from`. Returns `(start_of_lt, start_of_inner_after_open_tag, self_closing)`.
fn find_open_tag(xml: &[u8], name: &[u8], from: usize) -> Option<(usize, usize, bool)> {
    let prefix = make_prefix(b"<", name);
    let n = xml.len();
    let mut i = from;
    while i + prefix.len() < n {
        if let Some(off) = find_subslice(&xml[i..], &prefix) {
            let lt = i + off;
            let after_prefix = lt + prefix.len();
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
                // Find '>' that closes the open tag.
                let mut j = after_prefix;
                while j < n && xml[j] != b'>' {
                    j += 1;
                }
                if j >= n {
                    return None;
                }
                let self_closing = j > 0 && xml[j - 1] == b'/';
                let inner_start = j + 1;
                return Some((lt, inner_start, self_closing));
            } else {
                i = after_prefix;
            }
        } else {
            return None;
        }
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
                // Copy bytes before lt.
                out.extend_from_slice(&xml[i..lt]);
                if self_closing {
                    i = inner_start;
                } else {
                    match find_matching_close(xml, name, lt) {
                        Some(end) => i = end,
                        None => {
                            // Unbalanced — bail out and keep the rest verbatim.
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
/// nothing (the caller is going to strip the enclosing drawing/pict wrapper
/// anyway). Increment `out_count` once per extracted subtree.
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
                        // The closing tag we found ends at `end`. Inner content
                        // is xml[inner_start..end - len("</name>")]
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
    // Find </w:body>
    let close_body = b"</w:body>";
    let end = match find_subslice(xml, close_body) {
        Some(e) => e,
        None => return xml.to_vec(),
    };
    // Find sectPr open tag between body open and close.
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
        // Should have lifted exactly once (Fallback removed first).
        assert_eq!(result.lifted_count, 1);
        // Should contain the lifted text.
        assert!(out.contains("Lifted"), "lifted text not present: {out}");
        // Empty drawing/pict wrappers should be gone.
        assert!(!out.contains("<w:drawing"), "drawing not stripped: {out}");
        assert!(!out.contains("<w:pict"), "pict not stripped: {out}");
        assert!(!out.contains("<w:txbxContent"), "txbxContent leftover: {out}");
    }
}

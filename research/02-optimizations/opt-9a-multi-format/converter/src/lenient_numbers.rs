//! Rewrites numeric OOXML attributes that `rdocx-oxml` 0.1.2 parses with a
//! strict integer parse (`str::parse::<i32/u32/i64>`) into plain integers.
//!
//! Real-world writers emit decimals (`w:sz w:val="21.5"`), percentages
//! (`w:tcW w:w="50%"`) and `auto` where the schema allows integers, and a
//! single such value aborts the whole conversion with
//! `Oxml(ParseInt(InvalidDigit))`. Decimals are rounded; values that are not
//! numbers (percentages, `auto`, unit suffixes) have the attribute removed so
//! the reader falls back to its default for that property.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};

#[derive(Clone, Copy, PartialEq)]
enum Sign {
    Any,
    NonNegative,
}

/// Every `(attribute local name, sign)` the reader parses as a number, by
/// element local name.
fn numeric_attrs(element: &[u8]) -> &'static [(&'static str, Sign)] {
    match element {
        b"extent" | b"ext" => &[("cx", Sign::Any), ("cy", Sign::Any)],
        b"top" | b"left" | b"bottom" | b"right" | b"start" | b"end" | b"insideH" | b"insideV"
        | b"between" | b"bar" | b"tl2br" | b"tr2bl" => &[
            ("sz", Sign::NonNegative),
            ("space", Sign::NonNegative),
            ("w", Sign::Any),
        ],
        b"tab" => &[("pos", Sign::Any)],
        b"spacing" => &[
            ("before", Sign::Any),
            ("after", Sign::Any),
            ("line", Sign::Any),
            ("val", Sign::Any),
        ],
        b"ind" => &[
            ("left", Sign::Any),
            ("right", Sign::Any),
            ("firstLine", Sign::Any),
            ("hanging", Sign::Any),
            ("start", Sign::Any),
            ("end", Sign::Any),
        ],
        b"outlineLvl" | b"ilvl" | b"numId" | b"szCs" | b"sz" | b"gridSpan" | b"abstractNumId"
        | b"w" => &[("val", Sign::NonNegative)],
        b"position" | b"trHeight" => &[("val", Sign::Any)],
        b"tcW" | b"tblW" | b"gridCol" => &[("w", Sign::Any)],
        b"pgSz" => &[("w", Sign::Any), ("h", Sign::Any)],
        b"pgMar" => &[
            ("top", Sign::Any),
            ("right", Sign::Any),
            ("bottom", Sign::Any),
            ("left", Sign::Any),
            ("start", Sign::Any),
            ("end", Sign::Any),
            ("header", Sign::Any),
            ("footer", Sign::Any),
            ("gutter", Sign::Any),
        ],
        b"cols" => &[("num", Sign::NonNegative), ("space", Sign::Any)],
        b"col" => &[("w", Sign::Any), ("space", Sign::Any)],
        b"lvl" => &[("ilvl", Sign::NonNegative)],
        b"abstractNum" => &[("abstractNumId", Sign::NonNegative)],
        b"num" => &[("numId", Sign::NonNegative)],
        _ => &[],
    }
}

fn local_name(qname: &[u8]) -> &[u8] {
    match qname.iter().rposition(|&b| b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

enum Fix {
    Keep,
    Replace(String),
    Drop,
}

fn fix_value(raw: &str, sign: Sign) -> Fix {
    let trimmed = raw.trim();
    if let Ok(n) = trimmed.parse::<i64>() {
        return if sign == Sign::NonNegative && n < 0 {
            Fix::Drop
        } else if trimmed == raw {
            Fix::Keep
        } else {
            Fix::Replace(n.to_string())
        };
    }
    match trimmed.parse::<f64>() {
        Ok(f) if f.is_finite() && f.abs() < i64::MAX as f64 / 2.0 => {
            let rounded = f.round() as i64;
            if sign == Sign::NonNegative && rounded < 0 {
                Fix::Drop
            } else {
                Fix::Replace(rounded.to_string())
            }
        }
        _ => Fix::Drop,
    }
}

fn rewrite_tag(tag: &BytesStart) -> Option<BytesStart<'static>> {
    let attrs = numeric_attrs(local_name(tag.name().as_ref()));
    if attrs.is_empty() {
        return None;
    }
    let mut changed = false;
    let mut out = BytesStart::new(String::from_utf8_lossy(tag.name().as_ref()).into_owned());
    for attr in tag.attributes().with_checks(false).flatten() {
        let key = attr.key.as_ref();
        let sign = attrs
            .iter()
            .find(|(name, _)| name.as_bytes() == local_name(key))
            .map(|(_, sign)| *sign);
        let raw = String::from_utf8_lossy(&attr.value).into_owned();
        let key_owned = String::from_utf8_lossy(key).into_owned();
        match sign.map(|s| fix_value(&raw, s)) {
            None | Some(Fix::Keep) => out.push_attribute((key_owned.as_str(), raw.as_str())),
            Some(Fix::Replace(v)) => {
                changed = true;
                out.push_attribute((key_owned.as_str(), v.as_str()));
            }
            Some(Fix::Drop) => changed = true,
        }
    }
    changed.then_some(out)
}

/// Returns `Some(rewritten)` only when at least one attribute changed, so
/// documents with clean numbers are never re-serialized.
pub fn normalize_numeric_attributes(xml: &[u8]) -> Option<Vec<u8>> {
    let mut reader = Reader::from_reader(xml);
    let mut writer = Writer::new(Vec::with_capacity(xml.len()));
    let mut changed = false;
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(event) => event,
            Err(_) => return None,
        };
        let rewritten = match &event {
            Event::Start(tag) => rewrite_tag(tag).map(Event::Start),
            Event::Empty(tag) => rewrite_tag(tag).map(Event::Empty),
            _ => None,
        };
        changed |= rewritten.is_some();
        writer.write_event(rewritten.unwrap_or(event)).ok()?;
    }
    changed.then(|| writer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(xml: &str) -> String {
        String::from_utf8(normalize_numeric_attributes(xml.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn rounds_decimals() {
        assert_eq!(run(r#"<w:sz w:val="21.5"/>"#), r#"<w:sz w:val="22"/>"#);
    }

    #[test]
    fn drops_percentages_and_auto() {
        assert_eq!(
            run(r#"<w:tcW w:w="50%" w:type="pct"/>"#),
            r#"<w:tcW w:type="pct"/>"#
        );
        assert_eq!(run(r#"<w:gridCol w:w="auto"/>"#), r#"<w:gridCol/>"#);
    }

    #[test]
    fn drops_negative_values_for_unsigned_fields() {
        assert_eq!(run(r#"<w:sz w:val="-3"/>"#), r#"<w:sz/>"#);
        assert_eq!(
            run(r#"<w:position w:val="-2.5"/>"#),
            r#"<w:position w:val="-3"/>"#
        );
    }

    #[test]
    fn leaves_clean_documents_untouched() {
        assert!(normalize_numeric_attributes(br#"<w:sz w:val="22"/><w:p>hola</w:p>"#).is_none());
    }

    #[test]
    fn ignores_non_numeric_attributes_and_elements() {
        assert!(
            normalize_numeric_attributes(br#"<w:top w:val="single" w:color="auto"/>"#).is_none()
        );
    }
}

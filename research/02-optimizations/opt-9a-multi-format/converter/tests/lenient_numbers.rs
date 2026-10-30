use std::io::{Cursor, Write};

use approach_c_rdocx_opt9a::convert_to_pdf;

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn docx_with_body(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:body></w:document>"#
    );
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", RELS),
        ("word/document.xml", document.as_str()),
    ] {
        zw.start_file(name, opts).unwrap();
        zw.write_all(data.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn assert_converts(body: &str) {
    let pdf = convert_to_pdf(&docx_with_body(body)).unwrap_or_else(|e| panic!("{e}"));
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn baseline_integer_values_convert() {
    assert_converts(
        r#"<w:p><w:pPr><w:spacing w:before="120" w:after="120"/></w:pPr><w:r><w:rPr><w:sz w:val="22"/></w:rPr><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_paragraph_spacing_converts() {
    assert_converts(
        r#"<w:p><w:pPr><w:spacing w:before="120.5" w:after="60.0" w:line="276.5"/></w:pPr><w:r><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_indent_converts() {
    assert_converts(
        r#"<w:p><w:pPr><w:ind w:left="720.5" w:firstLine="360.25" w:hanging="1.5"/></w:pPr><w:r><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_font_size_converts() {
    assert_converts(
        r#"<w:p><w:r><w:rPr><w:sz w:val="21.5"/><w:szCs w:val="21.5"/></w:rPr><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_character_spacing_converts() {
    assert_converts(
        r#"<w:p><w:r><w:rPr><w:spacing w:val="1.5"/><w:w w:val="99.5"/><w:position w:val="-2.5"/></w:rPr><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_table_widths_convert() {
    assert_converts(
        r#"<w:tbl><w:tblPr><w:tblW w:w="5000.0" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="2500.5"/><w:gridCol w:w="2500.5"/></w:tblGrid><w:tr><w:trPr><w:trHeight w:val="300.5"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="2500.5" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="50%" w:type="pct"/></w:tcPr><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
}

#[test]
fn percentage_table_width_converts() {
    assert_converts(
        r#"<w:tbl><w:tblPr><w:tblW w:w="100%" w:type="pct"/></w:tblPr><w:tblGrid><w:gridCol w:w="auto"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
}

#[test]
fn decimal_borders_and_tabs_convert() {
    assert_converts(
        r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="4.5" w:space="1.5" w:color="auto"/></w:pBdr><w:tabs><w:tab w:val="left" w:pos="720.5"/></w:tabs></w:pPr><w:r><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_page_geometry_converts() {
    let document_body = r#"<w:p><w:r><w:t>hola</w:t></w:r></w:p>"#;
    let mut docx = docx_with_body(document_body);
    docx = replace_part(
        &docx,
        "w:w=\"12240\" w:h=\"15840\"",
        "w:w=\"12240.0\" w:h=\"15840.5\"",
    );
    let pdf = convert_to_pdf(&docx).unwrap_or_else(|e| panic!("{e}"));
    assert!(pdf.starts_with(b"%PDF"));
}

fn replace_part(docx: &[u8], from: &str, to: &str) -> Vec<u8> {
    use std::io::Read;
    let mut zr = zip::ZipArchive::new(Cursor::new(docx)).unwrap();
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for i in 0..zr.len() {
        let mut f = zr.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        zw.start_file(&name, opts).unwrap();
        zw.write_all(s.replace(from, to).as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

#[test]
fn decimal_numbering_ids_convert() {
    assert_converts(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0.0"/><w:numId w:val="1.0"/></w:numPr><w:outlineLvl w:val="1.0"/></w:pPr><w:r><w:t>hola</w:t></w:r></w:p>"#,
    );
}

#[test]
fn decimal_grid_span_converts() {
    assert_converts(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:gridSpan w:val="1.0"/></w:tcPr><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
}

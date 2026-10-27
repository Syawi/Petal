//! Searchable, paginated text PDFs with a system font and explicit Unicode mapping.
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, Stream,
};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, String>;
const LIMIT: usize = 16 * 1024 * 1024;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn decode(bytes: &[u8]) -> Result<String> {
    let doc = Document::load_mem(bytes).map_err(error)?;
    if doc.is_encrypted() {
        return Err("Protected PDF: the original was kept.".into());
    }
    // Refuse assets that a text-only conversion would discard, including scanned pages.
    for object in doc.objects.values() {
        let dict = match object {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        };
        if let Some(d) = dict {
            if d.has(b"AcroForm")
                || d.has(b"EmbeddedFiles")
                || d.has(b"RichMediaContent")
                || d.has(b"Annots")
                || d.has(b"JavaScript")
                || d.get(b"Subtype")
                    .and_then(Object::as_name)
                    .is_ok_and(|s| matches!(s, b"Image" | b"Form" | b"Widget"))
            {
                return Err(
                    "PDF contains images, forms or annotations. The original was kept.".into(),
                );
            }
        }
    }
    let pages = doc.get_pages();
    if pages.is_empty() || pages.len() > 2000 {
        return Err("Unsupported PDF page count.".into());
    }
    let mut text = String::new();
    for (number, id) in pages {
        let operations = doc.get_page_content_with_limit(id, LIMIT).map_err(error)?;
        if operations.len() > LIMIT {
            return Err("PDF page too large.".into());
        }
        let content = Content::decode(&operations).map_err(error)?;
        let fonts = doc.get_page_fonts(id).map_err(error)?;
        for font in fonts.values() {
            font.get_font_encoding_with_limit(&doc, LIMIT)
                .map_err(error)?;
        }
        for op in &content.operations {
            if op.operator == "Tf" {
                let name = op
                    .operands
                    .first()
                    .ok_or("Missing PDF font")?
                    .as_name()
                    .map_err(error)?;
                if !fonts.contains_key(name) {
                    return Err("A PDF font could not be found. The original was kept.".into());
                }
            }
        }
        if content.operations.iter().any(|op| {
            matches!(
                op.operator.as_str(),
                "BI" | "Do" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "sh"
            )
        }) {
            return Err("PDF contains graphics. The original was kept.".into());
        }
        let page = doc
            .extract_text_with_limit(&[number], LIMIT)
            .map_err(error)?;
        if page.trim().is_empty() {
            return Err("A PDF page has no readable text. Scanned documents require OCR.".into());
        }
        text.push_str(&page);
        text.push('\n');
        if text.len() > 2 * 1024 * 1024 {
            return Err("PDF too long.".into());
        }
    }
    Ok(text)
}
pub fn encode(text: &str) -> Result<Vec<u8>> {
    let font_path = std::path::PathBuf::from(
        std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("Fonts/tahoma.ttf");
    let font_data =
        std::fs::read(font_path).map_err(|e| format!("Windows Tahoma font unavailable: {e}"))?;
    let face = ttf_parser::Face::parse(&font_data, 0).map_err(error)?;
    if face.permissions() == Some(ttf_parser::Permissions::Restricted)
        || !face.is_outline_embedding_allowed()
    {
        return Err("This font does not allow PDF embedding.".into());
    }
    let scale = 1000.0 / face.units_per_em() as f32;
    let mut chars = BTreeMap::<char, (u16, u16, i64)>::new();
    for c in text.chars().chain(Some(' ')) {
        if c == '\n' || c == '\t' || chars.contains_key(&c) {
            continue;
        }
        // shortcut: no shaping engine; reject joining scripts rather than emit broken text.
        if matches!(c as u32,0x0590..=0x08ff|0x0900..=0x109f|0x1780..=0x17ff|0xfb1d..=0xfdff|0xfe70..=0xfeff)
        {
            return Err(
                "This script requires specialized text shaping. The original was kept.".into(),
            );
        }
        let glyph = face.glyph_index(c).ok_or_else(|| {
            format!("The PDF font does not contain the character '{c}'. The original was kept.")
        })?;
        let width = (face
            .glyph_hor_advance(glyph)
            .ok_or("Unknown character width")? as f32
            * scale)
            .round() as i64;
        if chars.len() >= 65534 {
            return Err("Too many different characters in this PDF.".into());
        }
        chars.insert(c, (chars.len() as u16 + 1, glyph.0, width));
    }
    let mut doc = Document::with_version("1.7");
    let font_stream = doc.add_object(Stream::new(
        dictionary! {"Length1"=>font_data.len() as i64},
        font_data.clone(),
    ));
    let bbox = face.global_bounding_box();
    let unit = |v: i16| (v as f32 * scale).round() as i64;
    let descriptor=doc.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"Tahoma","Flags"=>32,"FontBBox"=>vec![unit(bbox.x_min).into(),unit(bbox.y_min).into(),unit(bbox.x_max).into(),unit(bbox.y_max).into()],"ItalicAngle"=>0,"Ascent"=>unit(face.ascender()),"Descent"=>unit(face.descender()),"CapHeight"=>unit(face.capital_height().unwrap_or(face.ascender())),"StemV"=>80,"FontFile2"=>font_stream});
    let mut glyph_map = vec![0u8; (chars.len() + 1) * 2];
    let mut widths = Vec::<Object>::new();
    let mut cmap=String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /PetalUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    let entries: Vec<_> = chars.iter().collect();
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (c, (cid, glyph, width)) in chunk {
            glyph_map[*cid as usize * 2..*cid as usize * 2 + 2]
                .copy_from_slice(&glyph.to_be_bytes());
            widths.push((*cid as i64).into());
            widths.push(Object::Array(vec![(*width).into()]));
            let utf16 = c
                .encode_utf16(&mut [0u16; 2])
                .iter()
                .map(|u| format!("{u:04X}"))
                .collect::<String>();
            cmap.push_str(&format!("<{cid:04X}> <{utf16}>\n"));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let mapping = doc.add_object(Stream::new(dictionary! {}, glyph_map));
    let unicode = doc.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
    let descendant=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"Tahoma","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"CIDToGIDMap"=>mapping,"DW"=>1000,"W"=>widths});
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"Tahoma","Encoding"=>"Identity-H","DescendantFonts"=>vec![descendant.into()],"ToUnicode"=>unicode});
    // Wrap at a word boundary where possible; never drop any non-whitespace character.
    let mut lines = Vec::<String>::new();
    for paragraph in text.split('\n') {
        let expanded = paragraph.replace('\t', "    ");
        let mut line = String::new();
        let mut width = 0.0f32;
        for c in expanded.chars() {
            let advance = chars[&c].2 as f32 * 11.0 / 1000.0;
            if width + advance > 495.0 && !line.is_empty() {
                if let Some(space) = line.rfind(' ').filter(|pos| *pos > line.len() / 2) {
                    let rest = line.split_off(space + 1);
                    lines.push(std::mem::replace(&mut line, rest));
                    width = line
                        .chars()
                        .map(|c| chars[&c].2 as f32 * 11.0 / 1000.0)
                        .sum();
                } else {
                    lines.push(std::mem::take(&mut line));
                    width = 0.0;
                }
            }
            line.push(c);
            width += advance;
        }
        lines.push(line);
    }
    // Avoid a spurious blank final page when the source ends with a newline.
    while lines.last().is_some_and(|s| s.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() || lines.len() > 49 * 2000 {
        return Err("Document empty or too long for PDF export.".into());
    }
    let pages_id = doc.new_object_id();
    let resources = doc.add_object(dictionary! {"Font"=>dictionary!{"F1"=>font}});
    let mut kids = Vec::<Object>::new();
    for lines in lines.chunks(49) {
        let mut operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 11.into()]),
            Operation::new("TL", vec![15.into()]),
            Operation::new("Td", vec![50.into(), 791.into()]),
        ];
        for (n, line) in lines.iter().enumerate() {
            if n > 0 {
                operations.push(Operation::new("T*", vec![]));
            }
            let bytes = line
                .chars()
                .flat_map(|c| chars[&c].0.to_be_bytes())
                .collect::<Vec<_>>();
            operations.push(Operation::new(
                "Tj",
                vec![Object::String(bytes, lopdf::StringFormat::Hexadecimal)],
            ));
        }
        operations.push(Operation::new("ET", vec![]));
        let content = doc.add_object(Stream::new(
            dictionary! {},
            Content { operations }.encode().map_err(error)?,
        ));
        let page =
            doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages_id,"Contents"=>content,"Resources"=>resources});
        kids.push(page.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id,Object::Dictionary(dictionary!{"Type"=>"Pages","Kids"=>kids,"Count"=>count,"MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}));
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
    doc.trailer.set("Root", root);
    doc.compress();
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).map_err(error)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searchable_pdf_wraps_and_paginates() {
        let text=(0..140).map(|i|format!("Line {i}: Unicode \u{00e9}, \u{00eb}, \u{20ac} and \u{0152}. A long sentence to verify pagination and character mapping across every page.\n")).collect::<String>();
        let bytes = encode(&text).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        assert!(doc.get_pages().len() >= 3);
        let decoded = decode(&bytes).unwrap();
        let compact = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(compact(&decoded), compact(&text));
        std::fs::create_dir_all("target/test-data").unwrap();
        std::fs::write("target/test-data/document-pages.pdf", bytes).unwrap();
    }
    #[test]
    fn image_pdf_is_refused_and_missing_glyph_preserves_content() {
        let image = image::RgbImage::new(2, 2);
        assert!(decode(&crate::pdf::encode(&image).unwrap()).is_err());
        assert!(encode("Hello \u{1f980}").is_err());
        assert!(encode("\u{0645}\u{0631}\u{062d}\u{0628}\u{0627}").is_err());
    }
    #[test]
    fn an_unresolved_font_cannot_silently_drop_part_of_the_text() {
        let mut doc = Document::load_mem(&encode("Hello").unwrap()).unwrap();
        let page = doc.get_pages()[&1];
        let bytes = doc.get_page_content(page);
        let mut content = Content::decode(&bytes).unwrap();
        content
            .operations
            .push(Operation::new("Tf", vec!["MissingFont".into(), 11.into()]));
        content.operations.push(Operation::new(
            "Tj",
            vec![Object::string_literal("lost text")],
        ));
        let stream = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        doc.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Contents", stream);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        assert!(decode(&bytes).unwrap_err().contains("font"));
    }
}

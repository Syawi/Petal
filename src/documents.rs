//! Text conversions intentionally flatten formatting; embedded content is refused.
use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const INPUT_LIMIT: u64 = 64 * 1024 * 1024;
const TEXT_LIMIT: usize = 2 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
type Result<T> = std::result::Result<T, String>;
const COMPLEX: &str =
    "This document contains non-text content or an unsupported structure. The original was kept.";

pub fn extension(path: &Path) -> String {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "text" | "log" | "csv" | "tsv" => "txt".into(),
        "markdown" => "md".into(),
        "htm" => "html".into(),
        ext => ext.into(),
    }
}
pub fn supported(path: &Path) -> bool {
    matches!(
        extension(path).as_str(),
        "txt" | "md" | "html" | "rtf" | "docx" | "odt" | "pdf"
    )
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn clean(text: String) -> Result<String> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if text.len() > TEXT_LIMIT {
        return Err("Document too long (maximum 2 MiB of text).".into());
    }
    if text.trim().is_empty() {
        return Err("No readable text was found in this document.".into());
    }
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\u{c}'))
        || text.contains('\u{fffd}')
    {
        return Err("Unsupported encoding or characters. Save the text as UTF-8.".into());
    }
    Ok(text.replace('\u{c}', "\n"))
}
fn utf(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 {
            return Err("Incomplete UTF-16 data.".into());
        }
        let little = bytes[0] == 0xff;
        let units: Vec<_> = bytes[2..]
            .chunks_exact(2)
            .map(|s| {
                if little {
                    u16::from_le_bytes([s[0], s[1]])
                } else {
                    u16::from_be_bytes([s[0], s[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(error)
    } else {
        String::from_utf8(
            bytes
                .strip_prefix(&[0xef, 0xbb, 0xbf])
                .unwrap_or(bytes)
                .to_vec(),
        )
        .map_err(|_| "The text is not UTF-8. Save it as UTF-8 or UTF-16 with a BOM.".into())
    }
}
fn newline(out: &mut String) {
    if !out.ends_with('\n') {
        out.push('\n');
    }
}
fn escape(text: &str) -> String {
    html_escape::encode_text(text).to_string()
}

fn markdown(text: &str) -> Result<String> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out = String::new();
    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Image {..}) | Event::Html(_) | Event::InlineHtml(_) => return Err(COMPLEX.into()),
            Event::Start(Tag::List(Some(_))) => return Err(COMPLEX.into()),
            Event::Start(Tag::Link {..}) => return Err("Markdown links are not converted to avoid losing their addresses. The original was kept.".into()),
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item) => newline(&mut out),
            Event::Rule => {newline(&mut out);out.push_str("---\n");},
            _ => (),
        }
    }
    Ok(out)
}
fn html(text: &str) -> Result<String> {
    fn visit(
        handle: tl::NodeHandle,
        dom: &tl::VDom<'_>,
        out: &mut String,
        depth: usize,
        pre: bool,
    ) -> Result<()> {
        if depth > 128 {
            return Err(COMPLEX.into());
        }
        let node = handle.get(dom.parser()).ok_or(COMPLEX)?;
        match node {
            tl::Node::Raw(raw) => {
                let decoded =
                    html_escape::decode_html_entities(raw.as_utf8_str().as_ref()).to_string();
                if pre {
                    out.push_str(&decoded);
                } else {
                    for c in decoded.chars() {
                        if c.is_whitespace() {
                            if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                                out.push(' ');
                            }
                        } else {
                            out.push(c);
                        }
                    }
                }
            }
            tl::Node::Tag(tag) => {
                let name = tag.name().as_utf8_str().to_ascii_lowercase();
                if matches!(name.as_str(), "head" | "style" | "title" | "meta" | "link") {
                    return Ok(());
                }
                if matches!(
                    name.as_str(),
                    "img"
                        | "svg"
                        | "math"
                        | "canvas"
                        | "audio"
                        | "video"
                        | "object"
                        | "embed"
                        | "iframe"
                        | "script"
                        | "form"
                        | "input"
                        | "ol"
                        | "a"
                ) {
                    return Err(COMPLEX.into());
                }
                let block = matches!(
                    name.as_str(),
                    "p" | "div"
                        | "section"
                        | "article"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "li"
                        | "tr"
                        | "pre"
                        | "blockquote"
                );
                if block || name == "br" {
                    newline(out);
                }
                for child in tag.children().top().iter() {
                    visit(*child, dom, out, depth + 1, pre || name == "pre")?;
                }
                if block {
                    newline(out);
                }
                if name == "td" || name == "th" {
                    out.push('\t');
                }
            }
            _ => (),
        }
        if out.len() > TEXT_LIMIT {
            return Err(COMPLEX.into());
        }
        Ok(())
    }
    let dom = tl::parse(text, tl::ParserOptions::default()).map_err(error)?;
    let mut out = String::new();
    for handle in dom.children() {
        visit(*handle, &dom, &mut out, 0, false)?;
    }
    Ok(out)
}
fn package(bytes: &[u8], odt: bool) -> Result<String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
    if zip.len() > 1000 {
        return Err(COMPLEX.into());
    }
    let names: Vec<_> = zip.file_names().map(str::to_ascii_lowercase).collect();
    if names.iter().any(|n| {
        n.starts_with("word/media/")
            || n.starts_with("word/embeddings/")
            || n.starts_with("pictures/")
            || n.starts_with("object")
            || n.starts_with("word/header")
            || n.starts_with("word/footer")
            || matches!(
                n.as_str(),
                "word/footnotes.xml"
                    | "word/endnotes.xml"
                    | "word/comments.xml"
                    | "word/vbaproject.bin"
            )
    }) {
        return Err(COMPLEX.into());
    }
    let mut xml = String::new();
    zip.by_name(if odt {
        "content.xml"
    } else {
        "word/document.xml"
    })
    .map_err(error)?
    .take((TEXT_LIMIT * 8 + 1) as u64)
    .read_to_string(&mut xml)
    .map_err(error)?;
    if xml.len() > TEXT_LIMIT * 8 {
        return Err(COMPLEX.into());
    }
    let doc = roxmltree::Document::parse(&xml).map_err(error)?;
    let ns = if odt {
        "urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    } else {
        "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    };
    for node in doc.descendants().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        if (odt
            && matches!(
                tag,
                "frame"
                    | "image"
                    | "object"
                    | "list"
                    | "note"
                    | "annotation"
                    | "table"
                    | "tracked-changes"
            ))
            || (!odt
                && matches!(
                    tag,
                    "drawing"
                        | "pict"
                        | "object"
                        | "numPr"
                        | "fldChar"
                        | "instrText"
                        | "del"
                        | "ins"
                        | "txbxContent"
                        | "altChunk"
                        | "sym"
                        | "hyperlink"
                ))
        {
            return Err(COMPLEX.into());
        }
    }
    let mut out = String::new();
    for p in doc.descendants().filter(|n| {
        n.is_element()
            && n.tag_name().namespace() == Some(ns)
            && (n.tag_name().name() == "p" || (odt && n.tag_name().name() == "h"))
    }) {
        for n in p.descendants().skip(1) {
            if odt && n.is_text() {
                out.push_str(n.text().unwrap_or(""));
            }
            if n.is_element() && n.tag_name().namespace() == Some(ns) {
                match n.tag_name().name() {
                    "t" if !odt => out.push_str(n.text().unwrap_or("")),
                    "tab" => out.push('\t'),
                    "br" | "cr" | "line-break" => out.push('\n'),
                    "s" if odt => {
                        let count = n
                            .attribute((ns, "c"))
                            .unwrap_or("1")
                            .parse::<usize>()
                            .map_err(error)?;
                        if count > TEXT_LIMIT {
                            return Err(COMPLEX.into());
                        }
                        out.extend(std::iter::repeat_n(' ', count));
                    }
                    _ => (),
                }
            }
            if out.len() > TEXT_LIMIT {
                return Err(COMPLEX.into());
            }
        }
        out.push('\n');
    }
    Ok(out)
}
fn zip_text(text: &str, odt: bool) -> Result<Vec<u8>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let deflate = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut entry = |name: &str, data: &str, options| -> Result<()> {
        writer.start_file(name, options).map_err(error)?;
        writer.write_all(data.as_bytes()).map_err(error)
    };
    if odt {
        entry(
            "mimetype",
            "application/vnd.oasis.opendocument.text",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )?;
        let paragraphs = text
            .split('\n')
            .map(|line| {
                format!(
                    "<text:p xml:space=\"preserve\">{}</text:p>",
                    escape(line)
                        .replace(' ', "<text:s/>")
                        .replace('\t', "<text:tab/>")
                )
            })
            .collect::<String>();
        entry(
            "content.xml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3"><office:body><office:text>{paragraphs}</office:text></office:body></office:document-content>"#
            ),
            deflate,
        )?;
        entry(
            "META-INF/manifest.xml",
            r#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#,
            deflate,
        )?;
    } else {
        entry(
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            deflate,
        )?;
        entry(
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            deflate,
        )?;
        let paragraphs = text
            .split('\n')
            .map(|line| {
                format!(
                    "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                    escape(line).replace('\t', "</w:t><w:tab/><w:t xml:space=\"preserve\">")
                )
            })
            .collect::<String>();
        entry(
            "word/document.xml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{paragraphs}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr></w:body></w:document>"#
            ),
            deflate,
        )?;
    }
    writer.finish().map(|c| c.into_inner()).map_err(error)
}

fn rtf(bytes: &[u8]) -> Result<String> {
    use std::{ptr::null_mut, sync::OnceLock};
    use windows_sys::Win32::{
        System::LibraryLoader::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
        UI::WindowsAndMessaging::*,
    };
    let lower = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    if !lower.starts_with("{\\rtf")
        || bytes.contains(&0)
        || [
            "\\pict",
            "\\object",
            "\\objdata",
            "\\bin",
            "\\field",
            "\\shp",
            "\\footnote",
            "\\header",
            "\\footer",
        ]
        .iter()
        .any(|s| lower.contains(s))
    {
        return Err(COMPLEX.into());
    }
    static LOADED: OnceLock<bool> = OnceLock::new();
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    if !*LOADED.get_or_init(|| unsafe {
        !LoadLibraryExW(
            wide("Msftedit.dll").as_ptr(),
            null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
        .is_null()
    }) {
        return Err("The Windows RTF reader is unavailable.".into());
    }
    // Hidden, worker-owned control: Windows decodes RTF code pages and Unicode escapes.
    unsafe {
        let hwnd = CreateWindowExW(
            0,
            wide("RICHEDIT50W").as_ptr(),
            wide("").as_ptr(),
            WS_POPUP | ES_MULTILINE as u32,
            0,
            0,
            1,
            1,
            null_mut(),
            null_mut(),
            null_mut(),
            null(),
        );
        if hwnd.is_null() {
            return Err("Could not read the RTF document.".into());
        }
        struct Window(windows_sys::Win32::Foundation::HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let _window = Window(hwnd);
        SendMessageW(hwnd, WM_USER + 53, 0, TEXT_LIMIT as isize);
        #[repr(C)]
        struct SetText {
            flags: u32,
            codepage: u32,
        }
        let set = SetText {
            flags: 0,
            codepage: 1252,
        };
        let mut input = bytes.to_vec();
        input.push(0);
        if SendMessageW(
            hwnd,
            WM_USER + 97,
            &set as *const _ as usize,
            input.as_ptr() as isize,
        ) == 0
        {
            return Err("Empty or invalid RTF document.".into());
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 || len as usize >= TEXT_LIMIT {
            return Err(COMPLEX.into());
        }
        let mut text = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16(&text[..read as usize]).map_err(error)
    }
}
use std::ptr::null;
fn encode_rtf(text: &str) -> Vec<u8> {
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\uc1 ");
    for u in text.encode_utf16() {
        match u {
            10 => out.push_str("\\par\n"),
            9 => out.push_str("\\tab "),
            92 | 123 | 125 => {
                out.push('\\');
                out.push(char::from_u32(u as u32).unwrap());
            }
            32..=126 => out.push(char::from_u32(u as u32).unwrap()),
            _ => out.push_str(&format!("\\u{}?", u as i16)),
        }
    }
    out.push('}');
    out.into_bytes()
}
fn read(bytes: &[u8], ext: &str) -> Result<String> {
    clean(match ext {
        "pdf" => crate::text_pdf::decode(bytes)?,
        "docx" => package(bytes, false)?,
        "odt" => package(bytes, true)?,
        "rtf" => rtf(bytes)?,
        "md" => markdown(&utf(bytes)?)?,
        "html" => html(&utf(bytes)?)?,
        "txt" => utf(bytes)?,
        _ => return Err("Unknown text format.".into()),
    })
}
fn encode(text: &str, ext: &str) -> Result<Vec<u8>> {
    Ok(match ext {
        "txt"=>text.as_bytes().to_vec(),
        "md"=>{let mut s=String::new();for c in text.chars(){if "\\`*_{}[]<>()#+-.!|~>".contains(c){s.push('\\');} if c=='\n'{s.push_str("  \n");}else{s.push(c);}}s.into_bytes()},
        "html"=>format!("<!doctype html><html lang=\"und\"><head><meta charset=\"utf-8\"><title>Document</title></head><body><pre>{}</pre></body></html>",escape(text)).into_bytes(),
        "rtf"=>encode_rtf(text),"docx"=>zip_text(text,false)?,"odt"=>zip_text(text,true)?,"pdf"=>crate::text_pdf::encode(text)?,_=>return Err("This format is not a text document format.".into()),
    })
}
fn signature(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn replace(source: &Path, index: usize) -> Result<PathBuf> {
    let ext = crate::convert::FORMATS
        .get(index)
        .ok_or("Unknown format")?
        .2;
    if !matches!(ext, "txt" | "docx" | "md" | "html" | "rtf" | "odt" | "pdf") {
        return Err("This format is not a text document format.".into());
    }
    if ext == extension(source) {
        return Err("The document already uses this format.".into());
    }
    let mut source_handle = crate::convert::source_file(source)?;
    if source_handle.metadata().map_err(error)?.len() > INPUT_LIMIT {
        return Err("Document too large (maximum 64 MiB).".into());
    }
    let destination = source.with_extension(ext);
    if destination.try_exists().map_err(error)? {
        return Err("A file with this name already exists. It will not be overwritten.".into());
    }
    let mut bytes = Vec::new();
    (&mut source_handle)
        .take(INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err(COMPLEX.into());
    }
    let text = read(&bytes, &extension(source))?;
    let output = encode(&text, ext)?;
    let verified = read(&output, ext)?;
    if signature(&verified) != signature(&text) {
        return Err("Converted text verification failed. The original was kept.".into());
    }
    let path = source.with_file_name(format!(
        ".petal-text-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(error)?;
    let temporary = Temporary(path);
    file.write_all(&output).map_err(error)?;
    file.sync_all().map_err(error)?;
    drop(file);
    crate::pdf::verify_file(&temporary.0, &output).map_err(error)?;
    crate::convert::publish(source_handle, &temporary.0, &destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = "Unicode: \u{00e9} \u{00e8} \u{20ac} \u{0152} \u{0153}\nSecond paragraph: <text> & \"quote\".\nTab\tend. \u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}\n";
    #[test]
    fn formats_roundtrip_accents_and_paragraphs() {
        for ext in ["txt", "md", "html", "rtf", "docx", "odt", "pdf"] {
            let encoded = encode(SAMPLE, ext).unwrap_or_else(|e| panic!("{ext}: {e}"));
            let decoded = read(&encoded, ext).unwrap_or_else(|e| panic!("{ext}: {e}"));
            if matches!(ext, "docx" | "odt" | "rtf" | "html") {
                let directory =
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data/documents");
                fs::create_dir_all(&directory).unwrap();
                fs::write(directory.join(format!("sample.{ext}")), &encoded).unwrap();
            }
            assert_eq!(signature(&decoded), signature(SAMPLE), "{ext}: {decoded}");
            assert!(decoded.contains('\n'), "{ext}");
        }
    }
    #[test]
    fn menus_have_six_distinct_formats_and_pdf_last() {
        for ext in [
            "txt", "log", "csv", "md", "markdown", "html", "htm", "rtf", "docx", "odt", "pdf",
        ] {
            let path = PathBuf::from(format!("document.{ext}"));
            assert!(crate::convert::supported(&path));
            let choices = crate::convert::menu_formats(&path);
            let formats = choices.map(|id| crate::convert::FORMATS[id].2);
            assert!(!formats.contains(&extension(&path).as_str()));
            assert_eq!(
                formats
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                6
            );
            if ext != "pdf" {
                assert_eq!(formats[5], "pdf");
            }
        }
    }
    #[test]
    fn binary_embedded_content_and_empty_text_refused() {
        for (bytes, ext) in [
            (b"\0binary".as_slice(), "txt"),
            (b"<p>hello</p><img src='x'>", "html"),
            (b"![photo](x.png)", "md"),
            (br"{\rtf1 hello {\pict image}}", "rtf"),
            (b"   ", "txt"),
        ] {
            assert!(read(bytes, ext).is_err(), "{ext}");
        }
        assert!(utf(&[0xff, 0xfe, 0x61]).is_err());
        assert_eq!(utf(&[0xff, 0xfe, 0xe9, 0]).unwrap(), "\u{00e9}");
    }
    #[test]
    fn replacement_and_collision_preserve_original() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(format!(
                "documents-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample \u{00e9}.txt");
        fs::write(&source, SAMPLE).unwrap();
        let id = |ext| {
            crate::convert::FORMATS
                .iter()
                .position(|f| f.2 == ext)
                .unwrap()
        };
        let pdf = replace(&source, id("pdf")).unwrap();
        assert!(!source.exists());
        let docx = replace(&pdf, id("docx")).unwrap();
        assert!(!pdf.exists());
        assert_eq!(
            signature(&read(&fs::read(&docx).unwrap(), "docx").unwrap()),
            signature(SAMPLE)
        );
        let collision = docx.with_extension("txt");
        fs::write(&collision, b"existing").unwrap();
        assert!(replace(&docx, id("txt")).is_err());
        assert!(docx.exists());
        assert_eq!(fs::read(collision).unwrap(), b"existing");
        let bad = dir.join("bad.html");
        let badbytes = b"<p>text</p><img src='image.png'>";
        fs::write(&bad, badbytes).unwrap();
        assert!(replace(&bad, id("txt")).is_err());
        assert_eq!(fs::read(bad).unwrap(), badbytes);
    }
}

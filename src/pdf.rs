//! Single-page image PDF export. No printer driver or external runtime is required.
use flate2::{write::ZlibEncoder, Compression};
use image::RgbImage;
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

pub fn encode(image: &RgbImage) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(image.as_raw())?;
    let compressed = encoder.finish()?;
    // PDF 1.4 pages are at most 200 inches. Keep the full pixel data at any page size.
    let scale = 0.75f64.min(14400.0 / image.width().max(image.height()) as f64);
    let width = image.width() as f64 * scale;
    let height = image.height() as f64 * scale;
    let mut doc = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = vec![0usize];
    fn start(doc: &mut Vec<u8>, offsets: &mut Vec<usize>) -> io::Result<()> {
        offsets.push(doc.len());
        writeln!(doc, "{} 0 obj", offsets.len() - 1)
    }
    start(&mut doc, &mut offsets)?;
    doc.extend_from_slice(b"<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    start(&mut doc, &mut offsets)?;
    doc.extend_from_slice(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    start(&mut doc, &mut offsets)?;
    writeln!(doc,"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width:.4} {height:.4}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>\nendobj")?;
    start(&mut doc, &mut offsets)?;
    writeln!(doc,"<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream",image.width(),image.height(),compressed.len())?;
    doc.extend_from_slice(&compressed);
    drop(compressed);
    doc.extend_from_slice(b"\nendstream\nendobj\n");
    let content = format!("q\n{width:.4} 0 0 {height:.4} 0 0 cm\n/Im0 Do\nQ\n");
    start(&mut doc, &mut offsets)?;
    writeln!(doc, "<< /Length {} >>\nstream", content.len())?;
    doc.extend_from_slice(content.as_bytes());
    doc.extend_from_slice(b"endstream\nendobj\n");
    let xref = doc.len();
    writeln!(doc, "xref\n0 {}\n0000000000 65535 f ", offsets.len())?;
    for offset in offsets.iter().skip(1) {
        writeln!(doc, "{offset:010} 00000 n ")?;
    }
    writeln!(
        doc,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        offsets.len()
    )?;
    Ok(doc)
}

// Re-read every byte after sync_all, before the shared replacement step deletes the source.
pub fn verify_file(path: &Path, expected: &[u8]) -> io::Result<()> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() != expected.len() as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Incomplete PDF"));
    }
    let mut buffer = [0u8; 65536];
    for chunk in expected.chunks(buffer.len()) {
        file.read_exact(&mut buffer[..chunk.len()])?;
        if &buffer[..chunk.len()] != chunk {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PDF verification failed",
            ));
        }
    }
    Ok(())
}

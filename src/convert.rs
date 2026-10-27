use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const FORMATS: [(&str, Option<ImageFormat>, &str); 24] = [
    ("JPG", Some(ImageFormat::Jpeg), "jpg"),
    ("PNG", Some(ImageFormat::Png), "png"),
    ("WebP", Some(ImageFormat::WebP), "webp"),
    ("GIF", Some(ImageFormat::Gif), "gif"),
    ("BMP", Some(ImageFormat::Bmp), "bmp"),
    ("PDF", None, "pdf"),
    ("TIFF", Some(ImageFormat::Tiff), "tiff"),
    ("MP4", None, "mp4"),
    ("MKV", None, "mkv"),
    ("WebM", None, "webm"),
    ("MOV", None, "mov"),
    ("MP3", None, "mp3"),
    ("WAV", None, "wav"),
    ("AVI", None, "avi"),
    ("FLAC", None, "flac"),
    ("M4A", None, "m4a"),
    ("OGG", None, "ogg"),
    ("OPUS", None, "opus"),
    ("TXT", None, "txt"),
    ("DOCX", None, "docx"),
    ("MD", None, "md"),
    ("HTML", None, "html"),
    ("RTF", None, "rtf"),
    ("ODT", None, "odt"),
];
pub fn menu_formats(source: &Path) -> [usize; 6] {
    if crate::documents::supported(source) {
        return ["txt", "docx", "md", "html", "rtf", "pdf"].map(|ext| {
            let ext = if ext == crate::documents::extension(source) {
                "odt"
            } else {
                ext
            };
            FORMATS.iter().position(|f| f.2 == ext).unwrap()
        });
    }
    if let Some(kind) = crate::media::kind(source) {
        let (formats, alternative) = match kind {
            crate::media::Kind::Audio => (["mp3", "wav", "flac", "m4a", "ogg", "mp4"], "opus"),
            crate::media::Kind::Video => (["mp4", "mkv", "webm", "mov", "mp3", "wav"], "avi"),
        };
        return formats.map(|ext| {
            let ext = if ext == crate::media::extension(source) {
                alternative
            } else {
                ext
            };
            FORMATS.iter().position(|f| f.2 == ext).unwrap()
        });
    }
    let mut choices = [0, 1, 2, 3, 4, 5];
    if let Ok(format) = ImageFormat::from_path(source) {
        for choice in &mut choices[..5] {
            if FORMATS[*choice].1 == Some(format) {
                *choice = 6;
            }
        }
    }
    choices
}
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn supported(path: &Path) -> bool {
    crate::documents::supported(path)
        || crate::media::kind(path).is_some()
        || path.extension().and_then(|x| x.to_str()).is_some_and(|x| {
            matches!(
                x.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "ico"
            )
        })
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

// Hold the source open without write/delete sharing until the destination is committed.
pub(crate) fn source_file(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| format!("Could not read the file: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_file()
        || metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
    {
        return Err("Links and online-only cloud files are not supported. Copy the file to a local folder first.".into());
    }
    Ok(file)
}

pub fn convert_file(source: &Path, index: usize) -> Result<PathBuf, String> {
    if crate::documents::supported(source) {
        return crate::documents::convert_file(source, index);
    }
    if crate::media::kind(source).is_some() {
        return crate::media::convert_file(source, index);
    }
    if index >= 7 {
        return Err("This format is not an image format.".into());
    }
    let (_, format, extension) = *FORMATS.get(index).ok_or("Unknown format")?;
    if !supported(source) {
        return Err("This source format is not supported.".into());
    }
    let source_handle = source_file(source)?;
    let mut reader = ImageReader::new(std::io::BufReader::new(&source_handle))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32_768);
    limits.max_image_height = Some(32_768);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    if format.is_some() && reader.format() == format {
        return Err("The image already uses this format.".into());
    }
    let probe = || {
        File::open(source)
            .map(std::io::BufReader::new)
            .map_err(|e| e.to_string())
    };
    let multiple = match reader.format() {
        Some(ImageFormat::Png) => image::codecs::png::PngDecoder::new(probe()?)
            .map_err(|e| e.to_string())?
            .is_apng()
            .map_err(|e| e.to_string())?,
        Some(ImageFormat::WebP) => image::codecs::webp::WebPDecoder::new(probe()?)
            .map_err(|e| e.to_string())?
            .has_animation(),
        Some(ImageFormat::Gif) => {
            let mut gif =
                image::codecs::gif::GifDecoder::new(probe()?).map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(32768);
            limits.max_image_height = Some(32768);
            limits.max_alloc = Some(512 * 1024 * 1024);
            gif.set_limits(limits).map_err(|e| e.to_string())?;
            gif.into_frames()
                .take(2)
                .try_fold(0, |n, frame| frame.map(|_| n + 1))
                .map_err(|e| e.to_string())?
                > 1
        }
        Some(ImageFormat::Tiff) => tiff::decoder::Decoder::new(probe()?)
            .map_err(|e| e.to_string())?
            .more_images(),
        _ => false,
    };
    if multiple {
        return Err(
            "Animated or multipage image: conversion refused to preserve every frame.".into(),
        );
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Could not decode the image: {e}"))?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Could not decode the image: {e}"))?;
    image.apply_orientation(orientation);
    let destination = source.with_extension(extension);
    if destination.exists() {
        return Err(format!(
            "{} already exists. Rename it before converting.",
            destination.display()
        ));
    }
    let temp_path = source.with_file_name(format!(
        ".petal-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| e.to_string())?;
    let temp = Temporary(temp_path);
    let mut writer = BufWriter::new(file);
    let output = match format {
        Some(ImageFormat::Jpeg) | Some(ImageFormat::Bmp) | None => {
            let mut rgba = image.to_rgba8();
            for p in rgba.pixels_mut() {
                let a = p[3] as u32;
                for c in 0..3 {
                    p[c] = ((p[c] as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
                }
                p[3] = 255;
            }
            DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(rgba).to_rgb8())
        }
        _ => image,
    };
    let expected = (output.width(), output.height());
    let pdf = if format.is_none() {
        Some(
            crate::pdf::encode(&output.to_rgb8())
                .map_err(|e| format!("Could not create the PDF: {e}"))?,
        )
    } else {
        None
    };
    if let Some(bytes) = &pdf {
        writer.write_all(bytes).map_err(|e| e.to_string())?;
    } else {
        output
            .write_to(&mut writer, format.unwrap())
            .map_err(|e| format!("Conversion failed: {e}"))?;
    }
    writer.flush().map_err(|e| e.to_string())?;
    writer.get_ref().sync_all().map_err(|e| e.to_string())?;
    drop(writer);
    if let Some(bytes) = &pdf {
        crate::pdf::verify_file(&temp.0, bytes)
            .map_err(|e| format!("Could not verify the PDF: {e}"))?;
    } else {
        let check = ImageReader::open(&temp.0)
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| format!("Verification failed: {e}"))?;
        if (check.width(), check.height()) != expected {
            return Err("Image dimension verification failed.".into());
        }
    }
    drop(pdf);
    publish(source_handle, &temp.0, &destination)
}

pub(crate) fn publish(
    source_handle: File,
    temporary: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    use windows_sys::Win32::Storage::FileSystem::*;
    // Windows rename refuses an existing target, including a target created since exists().
    if unsafe {
        MoveFileExW(
            crate::wide(&temporary.to_string_lossy()).as_ptr(),
            crate::wide(&destination.to_string_lossy()).as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(format!(
            "Could not save the converted file. The original was kept: {}",
            std::io::Error::last_os_error()
        ));
    }
    drop(source_handle);
    Ok(destination.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf) {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data");
        fs::create_dir_all(&base).unwrap();
        let dir = base.join(format!(
            "petal-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("image source \u{00e9}.png");
        image::RgbaImage::from_pixel(12, 8, image::Rgba([255, 0, 0, 128]))
            .save(&path)
            .unwrap();
        (dir, path)
    }
    #[test]
    fn read_only_source_is_preserved() {
        let (dir, source) = fixture();
        let original = fs::read(&source).unwrap();
        let mut permissions = fs::metadata(&source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&source, permissions.clone()).unwrap();
        let converted = convert_file(&source, 2);
        let unchanged = fs::read(&source).unwrap();
        let still_readonly = fs::metadata(&source).unwrap().permissions().readonly();
        permissions.set_readonly(false);
        fs::set_permissions(&source, permissions).unwrap();
        assert!(converted.unwrap().is_file());
        assert_eq!(unchanged, original);
        assert!(still_readonly);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn publication_never_overwrites_a_target_created_before_commit() {
        let (dir, source) = fixture();
        let locked = source_file(&source).unwrap();
        let temporary = dir.join("converted.tmp");
        let destination = source.with_extension("jpg");
        fs::write(&temporary, b"converted").unwrap();
        fs::write(&destination, b"already present").unwrap();
        assert!(publish(locked, &temporary, &destination).is_err());
        assert!(source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"already present");
        assert_eq!(fs::read(&temporary).unwrap(), b"converted");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn menu_omits_source_format_and_keeps_pdf_last() {
        for extension in [
            "jpg", "JPEG", "png", "webp", "gif", "bmp", "tif", "TIFF", "ico",
        ] {
            let source = PathBuf::from(format!("photo.{extension}"));
            let choices = menu_formats(&source);
            let input = ImageFormat::from_path(&source).unwrap();
            assert!(choices.iter().all(|&i| FORMATS[i].1 != Some(input)));
            assert_eq!(FORMATS[choices[5]].0, "PDF");
            let mut unique = choices.to_vec();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), 6);
        }
        assert_eq!(menu_formats(Path::new("photo.jpeg")), [6, 1, 2, 3, 4, 5]);
        assert_eq!(menu_formats(Path::new("photo.png")), [0, 6, 2, 3, 4, 5]);
    }
    #[test]
    fn converts_all_formats_and_preserves_source() {
        for index in [0, 2, 3, 4, 6] {
            let (dir, source) = fixture();
            let original = fs::read(&source).unwrap();
            let dest =
                convert_file(&source, index).unwrap_or_else(|e| panic!("format {index}: {e}"));
            assert_eq!(fs::read(&source).unwrap(), original);
            assert_eq!(image::image_dimensions(&dest).unwrap(), (12, 8));
            if index == 6 {
                assert_eq!(
                    ImageReader::open(&dest)
                        .unwrap()
                        .with_guessed_format()
                        .unwrap()
                        .format(),
                    Some(ImageFormat::Tiff)
                );
                assert_eq!(
                    image::open(&dest).unwrap().to_rgba8().get_pixel(0, 0),
                    &image::Rgba([255, 0, 0, 128])
                );
            }
            if index == 0 {
                assert!(image::open(&dest).unwrap().to_rgb8().get_pixel(0, 0)[1] > 100);
            }
            fs::remove_dir_all(dir).unwrap();
        }
        let (dir, source) = fixture();
        let webp = convert_file(&source, 2).unwrap();
        assert!(convert_file(&webp, 1).is_err());
        assert!(source.exists());
        assert!(webp.exists());
        assert!(convert_file(&webp, 0).unwrap().exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn pdf_export_preserves_source_and_checks_collisions() {
        let (dir, source) = fixture();
        let mut image = image::RgbaImage::new(320, 200);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = match (x < 160, y < 100) {
                (true, true) => image::Rgba([255, 0, 0, 255]),
                (false, true) => image::Rgba([0, 255, 0, 255]),
                (true, false) => image::Rgba([0, 0, 255, 255]),
                _ => image::Rgba([0, 0, 0, 0]),
            };
        }
        image.save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        let dest = convert_file(&source, 5).unwrap();
        let bytes = fs::read(&dest).unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        crate::pdf::verify_file(&dest, &bytes).unwrap();
        let qa = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/pdf-checks");
        fs::create_dir_all(&qa).unwrap();
        fs::write(qa.join("render-check.pdf"), &bytes).unwrap();
        fs::write(&dest, b"truncated").unwrap();
        assert!(crate::pdf::verify_file(&dest, &bytes).is_err());
        image.save(&source).unwrap();
        assert!(convert_file(&source, 5).is_err());
        assert!(source.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"truncated");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn animated_gif_is_preserved() {
        let (dir, source) = fixture();
        let gif = source.with_extension("gif");
        let mut encoder = image::codecs::gif::GifEncoder::new(File::create(&gif).unwrap());
        encoder
            .encode_frames([
                image::Frame::new(image::RgbaImage::from_pixel(
                    12,
                    8,
                    image::Rgba([255, 0, 0, 255]),
                )),
                image::Frame::new(image::RgbaImage::from_pixel(
                    12,
                    8,
                    image::Rgba([0, 255, 0, 255]),
                )),
            ])
            .unwrap();
        drop(encoder);
        let before = fs::read(&gif).unwrap();
        assert!(convert_file(&gif, 5).unwrap_err().contains("Animated"));
        assert_eq!(fs::read(&gif).unwrap(), before);
        assert!(!gif.with_extension("pdf").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn collision_and_same_format_preserve_source() {
        let (dir, source) = fixture();
        let before = fs::read(&source).unwrap();
        fs::write(source.with_extension("webp"), b"existing").unwrap();
        assert!(convert_file(&source, 2).is_err());
        assert!(convert_file(&source, 1).is_err());
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(
            fs::read(source.with_extension("webp")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_source_is_never_deleted() {
        let (dir, source) = fixture();
        fs::write(&source, b"broken image").unwrap();
        assert!(convert_file(&source, 0).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"broken image");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}

use std::{
    fs::{self, File, OpenOptions},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Audio,
    Video,
}

pub fn extension(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "m4v" => "mp4".into(),
        "qt" => "mov".into(),
        "wave" => "wav".into(),
        "oga" => "ogg".into(),
        "m4b" => "m4a".into(),
        _ => ext,
    }
}
pub fn kind(path: &Path) -> Option<Kind> {
    match extension(path).as_str() {
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "wma" | "aif" | "aiff" => {
            Some(Kind::Audio)
        }
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "mpeg" | "mpg" | "m2ts" | "mts" | "ts"
        | "ogv" | "3gp" => Some(Kind::Video),
        _ => None,
    }
}
fn video_output(ext: &str) -> bool {
    matches!(ext, "mp4" | "mkv" | "webm" | "mov" | "avi")
}

fn tool(name: &str) -> Result<PathBuf, String> {
    #[cfg(test)]
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    #[cfg(not(test))]
    let root = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Petal folder not found")?
        .to_path_buf();
    let path = root.join("ffmpeg").join(format!("{name}.exe"));
    if path.is_file() {
        return Ok(path);
    }
    #[cfg(test)]
    {
        Err("Media test tools are missing. Run Install-MediaEngine.ps1.".into())
    }
    #[cfg(not(test))]
    {
        let directory = crate::runtime::data_directory()?.join("media/9.0.2");
        install_engine(&directory)?;
        let path = directory.join(format!("{name}.exe"));
        if path.is_file() {
            Ok(path)
        } else {
            Err("Media engine setup did not produce the required tools. Check your connection and try again.".into())
        }
    }
}
#[cfg(not(test))]
fn install_engine(directory: &Path) -> Result<(), String> {
    static INSTALL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _lock = INSTALL
        .lock()
        .map_err(|_| "Media setup was interrupted. Restart Petal and try again.")?;
    if directory.join("ffmpeg.exe").is_file() && directory.join("ffprobe.exe").is_file() {
        return Ok(());
    }
    let destination = directory.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules'); & {{\n{}\n}} -Destination '{}'",
        include_str!("../Install-MediaEngine.ps1"),
        destination
    );
    let windows = std::env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?;
    let mut installer =
        Command::new(PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    installer
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    run(installer, "First-use audio/video engine download (109 MB)")?;
    std::fs::write(
        directory.join("SOURCES.md"),
        include_str!("../ffmpeg/SOURCES.md"),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn command(name: &str) -> Result<Command, String> {
    let mut cmd = Command::new(tool(name)?);
    cmd.creation_flags(0x08000000) // CREATE_NO_WINDOW: no terminal on each conversion.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
        ]);
    if name == "ffmpeg" {
        cmd.args(["-filter_threads", "1", "-filter_complex_threads", "1"]);
    }
    Ok(cmd)
}
fn run(mut cmd: Command, stage: &str) -> Result<Output, String> {
    let result = cmd.output().map_err(|e| format!("{stage} failed: {e}"))?;
    if !result.status.success() {
        let detail: String = String::from_utf8_lossy(&result.stderr)
            .chars()
            .take(700)
            .collect();
        return Err(format!(
            "{stage} failed. The original was kept.\n{}",
            detail.trim()
        ));
    }
    Ok(result)
}

#[derive(Debug, Default)]
struct Info {
    videos: Vec<(u32, u32)>,
    audio: Vec<u32>,
    subtitles: usize,
    duration: f64,
}
fn parse_info(text: &str) -> Result<Info, String> {
    let mut info = Info::default();
    let (mut kind, mut width, mut height, mut channels, mut picture) =
        (String::new(), 0, 0, 0, false);
    for line in text.lines().map(str::trim) {
        match line {
            "[STREAM]" => {
                kind.clear();
                width = 0;
                height = 0;
                channels = 0;
                picture = false;
            }
            "[/STREAM]" => match kind.as_str() {
                "video" if !picture => info.videos.push((width, height)),
                "audio" => info.audio.push(channels),
                "subtitle" => info.subtitles += 1,
                _ => {}
            },
            _ => {
                if let Some((key, value)) = line.split_once('=') {
                    match key {
                        "codec_type" => kind = value.into(),
                        "width" => width = value.parse().unwrap_or(0),
                        "height" => height = value.parse().unwrap_or(0),
                        "channels" => channels = value.parse().unwrap_or(0),
                        "attached_pic" => picture = value == "1",
                        "duration" => info.duration = value.parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
            }
        }
    }
    if !info.duration.is_finite()
        || info.duration <= 0.0
        || (info.videos.is_empty() && info.audio.is_empty())
    {
        return Err(
            "Empty or unreadable media, or unknown duration. The original was kept.".into(),
        );
    }
    if info
        .videos
        .iter()
        .any(|&(w, h)| w == 0 || h == 0 || w > 8192 || h > 8192)
    {
        return Err("Unsupported video dimensions (maximum 8192 x 8192).".into());
    }
    Ok(info)
}
fn probe(path: &Path) -> Result<Info, String> {
    let mut cmd = command("ffprobe")?;
    cmd.args([
        "-show_entries",
        "format=duration:stream=codec_type,width,height,channels:stream_disposition=attached_pic",
        "-of",
        "default=noprint_wrappers=0:nokey=0",
    ])
    .arg(path);
    let output = run(cmd, "Media analysis")?;
    parse_info(&String::from_utf8_lossy(&output.stdout))
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace {
    directory: PathBuf,
    input: PathBuf,
    output: PathBuf,
}
impl Workspace {
    fn new(source: &Path, extension: &str) -> Result<Self, String> {
        let directory = source.with_file_name(format!(
            ".petal-media-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)
            .map_err(|e| format!("Could not create a temporary folder: {e}"))?;
        let input = directory
            .join("input")
            .with_extension(source.extension().unwrap_or_default());
        let output = directory.join("output").with_extension(extension);
        Ok(Self {
            directory,
            input,
            output,
        })
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.input);
        let _ = fs::remove_file(&self.output);
        let _ = fs::remove_dir(&self.directory);
    }
}

pub fn convert_file(source: &Path, index: usize) -> Result<PathBuf, String> {
    if kind(source).is_none() || !(7..18).contains(&index) {
        return Err("This format is not an audio or video format.".into());
    }
    let (_, _, ext) = *crate::convert::FORMATS.get(index).ok_or("Unknown format")?;
    if extension(source) == ext {
        return Err("The file already uses this format.".into());
    }
    // Resolve our own tools before copying a potentially large source.
    tool("ffmpeg")?;
    tool("ffprobe")?;
    let source = std::path::absolute(source).map_err(|e| e.to_string())?;
    let mut original = crate::convert::source_file(&source)?;
    let destination = source.with_extension(ext);
    if destination.exists() {
        return Err(format!(
            "{} already exists. Rename it before converting.",
            destination.display()
        ));
    }
    let work = Workspace::new(&source, ext)?;
    // Decode a private copy while keeping the original locked through publication.
    let mut copy = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&work.input)
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut original, &mut copy)
        .map_err(|e| format!("Could not create a temporary copy: {e}"))?;
    copy.sync_all().map_err(|e| e.to_string())?;
    drop(copy);
    let input = probe(&work.input)?;
    let video = video_output(ext);
    if input.audio.len() > 1 || (video && (input.videos.len() > 1 || input.subtitles > 0)) {
        return Err(
            "Multiple tracks or subtitles: conversion refused to avoid losing content.".into(),
        );
    }
    if !video && input.audio.is_empty() {
        return Err("This video has no audio track to extract.".into());
    }
    let black = video && input.videos.is_empty();
    let mut cmd = command("ffmpeg")?;
    cmd.args([
        "-nostdin", "-nostats", "-xerror", "-n", "-threads", "2", "-i",
    ])
    .arg(&work.input);
    if black {
        // Bound the generated video; let the complete audio stream reach EOF.
        let background = format!("color=c=black:s=1280x720:r=2:d={}", input.duration);
        cmd.args([
            "-f",
            "lavfi",
            "-i",
            &background,
            "-map",
            "1:v:0",
            "-map",
            "0:a:0",
        ]);
    } else if video {
        cmd.args(["-map", "0:V:0", "-map", "0:a:0?"]);
    } else {
        cmd.args(["-map", "0:a:0", "-vn"]);
    }
    cmd.args(["-map_metadata", "0", "-threads", "2"]);
    if video {
        cmd.args([
            "-vf",
            "pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-pix_fmt",
            "yuv420p",
        ]);
        match ext {
            "webm" => {
                cmd.args([
                    "-c:v",
                    "libvpx-vp9",
                    "-deadline",
                    "good",
                    "-cpu-used",
                    "5",
                    "-crf",
                    "32",
                    "-b:v",
                    "0",
                    "-c:a",
                    "libopus",
                    "-b:a",
                    "160k",
                ]);
            }
            "avi" => {
                cmd.args([
                    "-c:v",
                    "mpeg4",
                    "-q:v",
                    "3",
                    "-c:a",
                    "libmp3lame",
                    "-b:a",
                    "192k",
                    "-ac",
                    "2",
                ]);
            }
            _ => {
                cmd.args([
                    "-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-c:a", "aac", "-b:a",
                    "192k",
                ]);
            }
        }
        if matches!(ext, "mp4" | "mov") {
            cmd.args(["-movflags", "+faststart"]);
        }
    } else {
        match ext {
            "mp3" => {
                cmd.args(["-c:a", "libmp3lame", "-b:a", "192k", "-ac", "2"]);
            }
            "wav" => {
                cmd.args(["-c:a", "pcm_s16le"]);
            }
            "flac" => {
                cmd.args(["-c:a", "flac"]);
            }
            "m4a" => {
                cmd.args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"]);
            }
            "ogg" => {
                cmd.args(["-c:a", "libvorbis", "-q:a", "5"]);
            }
            "opus" => {
                cmd.args(["-c:a", "libopus", "-b:a", "160k"]);
            }
            _ => return Err("Unknown audio format".into()),
        }
    }
    cmd.arg(&work.output);
    run(cmd, "Conversion")?;
    let file = File::options()
        .read(true)
        .write(true)
        .open(&work.output)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    let output = probe(&work.output)?;
    if output.videos.len() != usize::from(video)
        || output.audio.len() != input.audio.len()
        || (output.duration - input.duration).abs() > 1.0
    {
        return Err(format!(
            "Track or duration verification failed. The original was kept.\nInput: {input:?}\nOutput: {output:?}"
        ));
    }
    if video {
        let expected = if black { (1280, 720) } else { input.videos[0] };
        let padded = ((expected.0 + 1) & !1, (expected.1 + 1) & !1);
        if output.videos[0] != padded && output.videos[0] != (padded.1, padded.0) {
            return Err("Video dimension verification failed. The original was kept.".into());
        }
    }
    // Decode every selected stream to EOF before publishing the converted file.
    let mut check = command("ffmpeg")?;
    check
        .args(["-nostdin", "-nostats", "-xerror", "-threads", "2", "-i"])
        .arg(&work.output)
        .args([
            "-map", "0:V?", "-map", "0:a?", "-threads", "2", "-f", "null", "-",
        ]);
    run(check, "Full output verification")?;
    crate::convert::publish(original, &work.output, &destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn format(ext: &str) -> usize {
        crate::convert::FORMATS
            .iter()
            .position(|f| f.2 == ext)
            .unwrap()
    }
    fn fixture(ext: &str) -> (PathBuf, PathBuf) {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-data");
        fs::create_dir_all(&base).unwrap();
        let dir = base.join(format!(
            "media-\u{00e9} space-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let source = dir.join(format!("clip test.{ext}"));
        (dir, source)
    }
    fn video_fixture(path: &Path, audio: bool) {
        let mut cmd = command("ffmpeg").unwrap();
        cmd.args([
            "-nostdin",
            "-n",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=96x64:rate=10",
        ]);
        if audio {
            cmd.args(["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100"]);
        }
        cmd.args([
            "-t", "0.7", "-c:v", "libx264", "-threads", "2", "-pix_fmt", "yuv420p",
        ]);
        if audio {
            cmd.args(["-c:a", "aac"]);
        }
        cmd.arg(path);
        run(cmd, "Video fixture").unwrap();
    }
    #[test]
    fn menus_keep_six_distinct_choices_without_the_source_format() {
        for ext in [
            "MP3", "wav", "wave", "flac", "m4a", "m4b", "ogg", "oga", "opus", "aac", "mp4", "m4v",
            "MKV", "webm", "mov", "qt", "avi", "wmv", "mts",
        ] {
            let path = PathBuf::from(format!("test.{ext}"));
            assert!(crate::convert::supported(&path));
            let choices = crate::convert::menu_formats(&path);
            let mut unique = choices.to_vec();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), 6);
            assert!(choices
                .iter()
                .all(|&i| i >= 7 && crate::convert::FORMATS[i].2 != extension(&path)));
        }
        let choices = crate::convert::menu_formats(Path::new("music.mp3"));
        assert_eq!(crate::convert::FORMATS[choices[5]].0, "MP4");
        assert_eq!(kind(Path::new("photo.png")), None);
    }
    #[test]
    fn audio_formats_and_mp3_to_mp4_preserve_sources() {
        for ext in ["mp3", "flac", "m4a", "ogg", "opus"] {
            let (dir, source) = fixture("wav");
            fs::write(&source, include_bytes!("../assets/paid.wav")).unwrap();
            let dest = convert_file(&source, format(ext)).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!(source.exists());
            assert_eq!(probe(&dest).unwrap().audio.len(), 1);
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
            assert_eq!(
                fs::read(&source).unwrap(),
                include_bytes!("../assets/paid.wav")
            );
            if ext == "mp3" {
                let duration = probe(&dest).unwrap().duration;
                let video = convert_file(&dest, format("mp4")).unwrap();
                let info = probe(&video).unwrap();
                assert_eq!(info.videos, [(1280, 720)]);
                assert_eq!(info.audio.len(), 1);
                assert!((info.duration - duration).abs() <= 0.5);
                assert!(dest.exists());
                assert!(convert_file(&video, format("wav")).is_err());
                let renamed = dir.join("roundtrip.mp4");
                fs::rename(&video, &renamed).unwrap();
                let video = renamed;
                let wav = convert_file(&video, format("wav")).unwrap();
                assert_eq!(probe(&wav).unwrap().audio.len(), 1);
                assert!((probe(&wav).unwrap().duration - 0.6).abs() < 0.1);
                assert!(video.exists());
            }
            fs::remove_dir_all(dir).unwrap();
        }
    }
    #[test]
    fn video_formats_and_audio_extraction_work() {
        for ext in ["mkv", "webm", "mov", "avi", "mp3", "wav"] {
            let (dir, source) = fixture("mp4");
            video_fixture(&source, true);
            let original = fs::read(&source).unwrap();
            let dest = convert_file(&source, format(ext)).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert_eq!(fs::read(&source).unwrap(), original);
            let info = probe(&dest).unwrap();
            assert_eq!(info.videos.len(), usize::from(video_output(ext)));
            assert_eq!(info.audio.len(), 1);
            if ext == "mkv" {
                assert!(convert_file(&dest, format("mp4")).is_err());
                let renamed = dir.join("roundtrip.mkv");
                fs::rename(&dest, &renamed).unwrap();
                assert!(convert_file(&renamed, format("mp4")).unwrap().is_file());
            }
            fs::remove_dir_all(dir).unwrap();
        }
    }
    #[test]
    fn corrupt_inputs_collisions_and_silent_audio_preserve_sources() {
        let (dir, source) = fixture("mp3");
        fs::write(&source, b"not an audio file").unwrap();
        assert!(convert_file(&source, format("wav")).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"not an audio file");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::write(source.with_extension("mp4"), b"existing destination").unwrap();
        assert!(convert_file(&source, format("mp4")).is_err());
        assert_eq!(
            fs::read(source.with_extension("mp4")).unwrap(),
            b"existing destination"
        );
        assert!(convert_file(&source, format("mp3")).is_err());
        fs::remove_dir_all(dir).unwrap();
        let (dir, source) = fixture("mp4");
        video_fixture(&source, false);
        assert!(convert_file(&source, format("mp3"))
            .unwrap_err()
            .contains("no audio track"));
        assert!(source.is_file());
        assert!(!source.with_extension("mp3").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn multiple_audio_tracks_are_never_silently_discarded() {
        let (dir, source) = fixture("mp4");
        video_fixture(&source, true);
        let multiple = dir.join("two tracks.mkv");
        let mut cmd = command("ffmpeg").unwrap();
        cmd.args(["-nostdin", "-n", "-i"])
            .arg(&source)
            .args([
                "-map", "0:v:0", "-map", "0:a:0", "-map", "0:a:0", "-c", "copy",
            ])
            .arg(&multiple);
        run(cmd, "Multitrack fixture").unwrap();
        let before = fs::read(&multiple).unwrap();
        assert!(convert_file(&multiple, format("mp4"))
            .unwrap_err()
            .contains("Multiple tracks"));
        assert_eq!(fs::read(&multiple).unwrap(), before);
        assert!(!multiple.with_extension("mp4").exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn probe_ignores_cover_art_and_rejects_unknown_duration() {
        let info = parse_info("[STREAM]\ncodec_type=video\nwidth=100\nheight=100\n[DISPOSITION]\nattached_pic=1\n[/DISPOSITION]\n[/STREAM]\n[STREAM]\ncodec_type=audio\nchannels=2\n[/STREAM]\n[FORMAT]\nduration=5.25\n[/FORMAT]").unwrap();
        assert!(info.videos.is_empty());
        assert_eq!(info.audio, [2]);
        assert!(parse_info("duration=NaN").is_err());
    }
}

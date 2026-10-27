#[cfg(not(test))]
use windows_sys::Win32::Media::Audio::{
    PlaySoundW, SND_ASYNC, SND_FILENAME, SND_MEMORY, SND_NODEFAULT,
};

const PAID: &[u8] = include_bytes!("../assets/paid.wav");

#[cfg(test)]
pub static PLAYS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn success() {
    #[cfg(not(test))]
    unsafe {
        if let Some(path) = crate::settings::sound_file() {
            let path = crate::wide(&path.to_string_lossy());
            if PlaySoundW(
                path.as_ptr(),
                std::ptr::null_mut(),
                SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
            ) != 0
            {
                return;
            }
        }
        // Embedded bytes live for the entire asynchronous playback.
        PlaySoundW(
            PAID.as_ptr().cast(),
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
    #[cfg(test)]
    PLAYS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
pub fn stop() {
    #[cfg(not(test))]
    unsafe {
        PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_paid_is_valid_short_pcm_without_clipping() {
        assert_eq!(&PAID[..4], b"RIFF");
        assert_eq!(&PAID[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(PAID[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(PAID[22..24].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(PAID[24..28].try_into().unwrap()), 44100);
        assert_eq!(u16::from_le_bytes(PAID[34..36].try_into().unwrap()), 16);
        assert_eq!(&PAID[36..40], b"data");
        assert_eq!(PAID.len(), 44 + 44100 * 2 * 3 / 5);
        assert_eq!(
            u32::from_le_bytes(PAID[40..44].try_into().unwrap()) as usize,
            PAID.len() - 44
        );
        let peak = PAID[44..]
            .chunks_exact(2)
            .map(|s| i16::from_le_bytes(s.try_into().unwrap()).unsigned_abs())
            .max()
            .unwrap();
        assert!((980..4000).contains(&peak));
    }
}

pub fn validate_file(path: &std::path::Path) -> Result<(), String> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("Could not read the sound file: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut header = [0u8; 12];
    if !(44..=8 * 1024 * 1024).contains(&size)
        || file.read_exact(&mut header).is_err()
        || &header[..4] != b"RIFF"
        || &header[8..] != b"WAVE"
        || u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8 > size
    {
        return Err("Choose a valid WAV sound smaller than 8 MB.".into());
    }
    Ok(())
}

#[cfg(test)]
mod custom_tests {
    use super::*;
    #[test]
    fn custom_sound_validation_rejects_invalid_files() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(format!("sound-validation-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("chosen.wav");
        std::fs::write(&path, PAID).unwrap();
        assert!(validate_file(&path).is_ok());
        std::fs::write(&path, b"not a wave file").unwrap();
        assert!(validate_file(&path).is_err());
        let mut truncated = PAID[..44].to_vec();
        truncated[4..8].copy_from_slice(&900_000u32.to_le_bytes());
        std::fs::write(&path, truncated).unwrap();
        assert!(validate_file(&path).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

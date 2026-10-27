#[cfg(not(test))]
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

const PAID: &[u8] = include_bytes!("../assets/paid.wav");

#[cfg(test)]
pub static PLAYS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn success() {
    #[cfg(not(test))]
    unsafe {
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

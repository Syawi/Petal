use windows_sys::Win32::System::Registry::*;
const KEY: &str = "Software\\Petal";
const SOUND: &str = "SoundEnabled";

pub fn sound_enabled() -> bool {
    #[cfg(test)]
    {
        true
    }
    #[cfg(not(test))]
    unsafe {
        let mut value = 1u32;
        let mut size = 4;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            crate::wide(KEY).as_ptr(),
            crate::wide(SOUND).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut value as *mut _ as *mut _,
            &mut size,
        );
        if status == 0 && size == 4 {
            value != 0
        } else {
            true
        }
    }
}
pub fn save_sound(enabled: bool) -> Result<(), String> {
    let value = u32::from(enabled);
    let result = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            crate::wide(KEY).as_ptr(),
            crate::wide(SOUND).as_ptr(),
            REG_DWORD,
            &value as *const _ as *const _,
            4,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "Could not save the sound setting: {}",
            std::io::Error::from_raw_os_error(result as i32)
        ))
    }
}

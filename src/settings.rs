use std::{
    path::{Path, PathBuf},
    ptr::null_mut,
};
use windows_sys::Win32::{System::Registry::*, UI::Input::KeyboardAndMouse::*};
const KEY: &str = "Software\\Petal";
const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub modifiers: u8,
    pub key: u16,
}
impl Default for Binding {
    fn default() -> Self {
        Self {
            modifiers: 1,
            key: VK_RBUTTON,
        }
    }
}
impl Binding {
    pub fn encode(self) -> u32 {
        ((self.modifiers as u32) << 16) | self.key as u32
    }
    pub fn decode(value: u32) -> Option<Self> {
        let binding = Self {
            modifiers: (value >> 16) as u8,
            key: value as u16,
        };
        (value >> 20 == 0 && binding.key > 0 && binding.key < 255 && !modifier_key(binding.key))
            .then_some(binding)
    }
    pub fn mouse(self) -> bool {
        matches!(
            self.key,
            VK_LBUTTON | VK_RBUTTON | VK_MBUTTON | VK_XBUTTON1 | VK_XBUTTON2
        )
    }
    pub fn matches(self, key: u16, modifiers: u8) -> bool {
        self.key == key && self.modifiers == modifiers
    }
    pub fn label(self) -> String {
        let mut parts: Vec<String> = [(2, "Ctrl"), (1, "Shift"), (4, "Alt"), (8, "Win")]
            .into_iter()
            .filter(|(flag, _)| self.modifiers & flag != 0)
            .map(|(_, s)| s.into())
            .collect();
        let key = match self.key {
            VK_LBUTTON => "Left-click".into(),
            VK_RBUTTON => "Right-click".into(),
            VK_MBUTTON => "Middle-click".into(),
            VK_XBUTTON1 => "Mouse 4".into(),
            VK_XBUTTON2 => "Mouse 5".into(),
            0x30..=0x39 | 0x41..=0x5a => char::from_u32(self.key as u32).unwrap().to_string(),
            0x70..=0x87 => format!("F{}", self.key - 0x70 + 1),
            _ => unsafe {
                let scan = MapVirtualKeyW(self.key as u32, MAPVK_VK_TO_VSC_EX);
                let extended = if scan & 0xff00 != 0 { 1 << 24 } else { 0 };
                let mut text = [0u16; 96];
                let count = GetKeyNameTextW(
                    (((scan & 255) << 16) | extended) as i32,
                    text.as_mut_ptr(),
                    96,
                );
                if count > 0 {
                    String::from_utf16_lossy(&text[..count as usize])
                } else {
                    format!("Key {:02X}", self.key)
                }
            },
        };
        parts.push(key);
        parts.join(" + ")
    }
}
pub fn modifier_key(key: u16) -> bool {
    matches!(
        key,
        VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
    )
}
pub unsafe fn modifiers() -> u8 {
    u8::from(GetAsyncKeyState(VK_SHIFT as i32) < 0)
        | (u8::from(GetAsyncKeyState(VK_CONTROL as i32) < 0) << 1)
        | (u8::from(GetAsyncKeyState(VK_MENU as i32) < 0) << 2)
        | (u8::from(GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0)
            << 3)
}
fn read_dword(name: &str) -> Option<u32> {
    unsafe {
        let mut value = 0u32;
        let mut size = 4;
        (RegGetValueW(
            HKEY_CURRENT_USER,
            crate::wide(KEY).as_ptr(),
            crate::wide(name).as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut value as *mut _ as *mut _,
            &mut size,
        ) == 0
            && size == 4)
            .then_some(value)
    }
}
fn write_dword(name: &str, value: u32) -> Result<(), String> {
    unsafe {
        status(RegSetKeyValueW(
            HKEY_CURRENT_USER,
            crate::wide(KEY).as_ptr(),
            crate::wide(name).as_ptr(),
            REG_DWORD,
            &value as *const _ as *const _,
            4,
        ))
    }
}
fn read_string(key: &str, name: &str) -> String {
    unsafe {
        let mut text = vec![0u16; 32768];
        let mut size = (text.len() * 2) as u32;
        if RegGetValueW(
            HKEY_CURRENT_USER,
            crate::wide(key).as_ptr(),
            crate::wide(name).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            text.as_mut_ptr().cast(),
            &mut size,
        ) != 0
        {
            return String::new();
        }
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16_lossy(&text[..end])
    }
}
fn write_string(key: &str, name: &str, value: &str) -> Result<(), String> {
    let text = crate::wide(value);
    unsafe {
        status(RegSetKeyValueW(
            HKEY_CURRENT_USER,
            crate::wide(key).as_ptr(),
            crate::wide(name).as_ptr(),
            REG_SZ,
            text.as_ptr().cast(),
            (text.len() * 2) as u32,
        ))
    }
}
fn status(code: u32) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!(
            "Could not save settings: {}",
            std::io::Error::from_raw_os_error(code as i32)
        ))
    }
}
pub fn binding() -> Binding {
    #[cfg(test)]
    {
        Binding::default()
    }
    #[cfg(not(test))]
    {
        read_dword("ActivationBinding")
            .and_then(Binding::decode)
            .unwrap_or_default()
    }
}
pub fn save_binding(binding: Binding) -> Result<(), String> {
    if Binding::decode(binding.encode()).is_none() {
        return Err("Choose a key or mouse button, with optional modifiers.".into());
    }
    write_dword("ActivationBinding", binding.encode())
}
pub fn sound_enabled() -> bool {
    #[cfg(test)]
    {
        true
    }
    #[cfg(not(test))]
    {
        read_dword("SoundEnabled").unwrap_or(1) != 0
    }
}
pub fn save_sound(enabled: bool) -> Result<(), String> {
    write_dword("SoundEnabled", u32::from(enabled))
}
pub fn sound_file() -> Option<PathBuf> {
    let path = read_string(KEY, "SoundFile");
    (!path.is_empty()).then(|| PathBuf::from(path))
}
pub fn save_sound_file(path: Option<&Path>) -> Result<(), String> {
    write_string(
        KEY,
        "SoundFile",
        &path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}
pub fn startup_enabled() -> bool {
    !read_string(RUN, "Petal").is_empty()
}
fn startup_command(path: &Path) -> String {
    format!("\"{}\" --background", path.display())
}
pub fn save_startup(enabled: bool) -> Result<(), String> {
    if enabled {
        write_string(
            RUN,
            "Petal",
            &startup_command(&std::env::current_exe().map_err(|e| e.to_string())?),
        )?;
    } else {
        unsafe {
            let result = RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                crate::wide(RUN).as_ptr(),
                crate::wide("Petal").as_ptr(),
            );
            if result != 0 && result != 2 {
                status(result)?;
            }
        }
    }
    write_dword("StartupEnabled", u32::from(enabled))
}
pub fn initialize_startup() -> Result<(), String> {
    match read_dword("StartupEnabled") {
        None => save_startup(true),
        Some(1) if startup_enabled() => {
            let command = startup_command(&std::env::current_exe().map_err(|e| e.to_string())?);
            if read_string(RUN, "Petal") != command {
                write_string(RUN, "Petal", &command)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn universal_bindings_roundtrip_and_require_exact_modifiers() {
        assert_eq!(Binding::default().label(), "Shift + Right-click");
        for modifiers in 0..16 {
            for key in [
                VK_RBUTTON,
                VK_LBUTTON,
                VK_MBUTTON,
                VK_XBUTTON1,
                VK_XBUTTON2,
                0x41,
                VK_F8,
                VK_SPACE,
            ] {
                let binding = Binding { modifiers, key };
                assert_eq!(Binding::decode(binding.encode()), Some(binding));
                assert!(binding.matches(key, modifiers));
                assert!(!binding.matches(key, modifiers ^ 1));
                assert!(!binding.label().is_empty());
            }
        }
        for value in [
            0,
            0xffff,
            0x100000,
            0x80000000,
            VK_SHIFT as u32,
            VK_LWIN as u32,
        ] {
            assert!(Binding::decode(value).is_none());
        }
        assert!(Binding::default().mouse());
        assert!(!Binding {
            modifiers: 2,
            key: 0x4b
        }
        .mouse());
    }
    #[test]
    fn startup_quotes_executable_paths() {
        assert_eq!(
            startup_command(Path::new("C:\\Program Files\\Petal\\Petal.exe")),
            "\"C:\\Program Files\\Petal\\Petal.exe\" --background"
        );
    }
}

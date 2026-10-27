use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
fn write_asset(directory: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|e| format!("Could not prepare Petal data: {e}"))?;
    let path = directory.join(name);
    if fs::read(&path).ok().as_deref() != Some(bytes) {
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(path)
}
pub fn data_directory() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("Petal"))
        .ok_or_else(|| "LOCALAPPDATA is unavailable".into())
}
pub fn logo_file() -> Result<PathBuf, String> {
    write_asset(
        &data_directory()?,
        "petal.png",
        include_bytes!("../assets/petal.png"),
    )
}
fn license_bytes() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("../assets/third-party-licenses.txt.gz")[..])
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn licenses_file() -> Result<PathBuf, String> {
    write_asset(&data_directory()?, "LICENSES.txt", &license_bytes()?)
}
pub unsafe fn open_licenses(owner: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    let path = licenses_file()?;
    let result = windows_sys::Win32::UI::Shell::ShellExecuteW(
        owner,
        crate::wide("open").as_ptr(),
        crate::wide(&path.to_string_lossy()).as_ptr(),
        std::ptr::null(),
        std::ptr::null(),
        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
    );
    if result as usize <= 32 {
        Err("Could not open license notices.".into())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_assets_work_without_adjacent_files() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(format!("standalone-assets-{}", std::process::id()));
        let logo =
            write_asset(&folder, "petal.png", include_bytes!("../assets/petal.png")).unwrap();
        assert!(image::image_dimensions(logo).is_ok());
        let notices = license_bytes().unwrap();
        let text = String::from_utf8(notices.clone()).unwrap();
        assert!(text.contains("MIT License"));
        assert!(text.contains("BENCHO COMPLETION SOUND"));
        assert!(text.contains("RUST STANDARD LIBRARY"));
        let path = write_asset(&folder, "LICENSES.txt", &notices).unwrap();
        fs::write(&path, "outdated").unwrap();
        assert_eq!(
            fs::read(write_asset(&folder, "LICENSES.txt", &notices).unwrap()).unwrap(),
            notices
        );
        fs::remove_dir_all(folder).unwrap();
    }
}

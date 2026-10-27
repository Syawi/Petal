use std::path::{Path, PathBuf};
use windows::{
    core::{Interface, GUID, HSTRING, PCWSTR, PROPVARIANT},
    Data::Xml::Dom::XmlDocument,
    Win32::{
        System::{
            Com::{
                CoCreateInstance, IPersistFile,
                StructuredStorage::{PropVariantChangeType, PROPVAR_CHANGE_FLAGS},
                CLSCTX_INPROC_SERVER,
            },
            Variant::VT_LPWSTR,
            WinRT::{RoInitialize, RO_INIT_SINGLETHREADED},
        },
        UI::Shell::{
            IShellLinkW,
            PropertiesSystem::{IPropertyStore, PROPERTYKEY},
            SetCurrentProcessExplicitAppUserModelID, ShellLink,
        },
    },
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier},
};
use windows_sys::Win32::System::Registry::*;

pub const APP_ID: &str = "Petal.Converter";
pub struct Notifications {
    notifier: ToastNotifier,
    image_uri: String,
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn xml_escape(value: &str) -> String {
    html_escape::encode_double_quoted_attribute(value).to_string()
}
fn file_uri(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or("The logo path is not valid Unicode")?
        .replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let mut uri = String::from(if path.starts_with("//") {
        "file:"
    } else {
        "file:///"
    });
    for b in path.as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(b) {
            uri.push(*b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    Ok(uri)
}
fn payload(title: &str, message: &str, image: &str) -> String {
    format!(
        r#"<toast><visual><binding template="ToastGeneric"><image placement="appLogoOverride" hint-crop="none" src="{}"/><text>{}</text><text>{}</text></binding></visual><audio silent="true"/></toast>"#,
        xml_escape(image),
        xml_escape(title),
        xml_escape(message)
    )
}
unsafe fn register_string(name: &str, value: &str) -> Result<(), String> {
    let key = crate::wide(&format!("Software\\Classes\\AppUserModelId\\{APP_ID}"));
    let value = crate::wide(value);
    let status = RegSetKeyValueW(
        HKEY_CURRENT_USER,
        key.as_ptr(),
        crate::wide(name).as_ptr(),
        REG_SZ,
        value.as_ptr() as _,
        (value.len() * 2) as u32,
    );
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status as i32).to_string())
    }
}
unsafe fn shortcut(exe: &Path, icon: &Path) -> Result<(), String> {
    let start = std::env::var_os("APPDATA").ok_or("APPDATA is unavailable")?;
    let path = PathBuf::from(start).join("Microsoft/Windows/Start Menu/Programs/Petal.lnk");
    let link: IShellLinkW =
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(error)?;
    let exe = crate::wide(&exe.to_string_lossy());
    link.SetPath(PCWSTR(exe.as_ptr())).map_err(error)?;
    link.SetIconLocation(PCWSTR(crate::wide(&icon.to_string_lossy()).as_ptr()), 0)
        .map_err(error)?;
    link.SetDescription(windows::core::w!("Petal file converter"))
        .map_err(error)?;
    let store: IPropertyStore = link.cast().map_err(error)?;
    let key = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };
    let source = PROPVARIANT::from(APP_ID);
    let mut app_id = PROPVARIANT::default();
    PropVariantChangeType(&mut app_id, &source, PROPVAR_CHANGE_FLAGS(0), VT_LPWSTR)
        .map_err(error)?;
    store.SetValue(&key, &app_id).map_err(error)?;
    store.Commit().map_err(error)?;
    let persist: IPersistFile = link.cast().map_err(error)?;
    persist
        .Save(PCWSTR(crate::wide(&path.to_string_lossy()).as_ptr()), true)
        .map_err(error)?;
    // Tell the Shell to reload this shortcut's icon after a portable-app update.
    windows_sys::Win32::UI::Shell::SHChangeNotify(
        windows_sys::Win32::UI::Shell::SHCNE_UPDATEITEM as i32,
        windows_sys::Win32::UI::Shell::SHCNF_PATHW | windows_sys::Win32::UI::Shell::SHCNF_FLUSH,
        crate::wide(&path.to_string_lossy()).as_ptr().cast(),
        std::ptr::null(),
    );
    Ok(())
}
impl Notifications {
    pub fn new() -> Result<Self, String> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED).map_err(error)?;
            SetCurrentProcessExplicitAppUserModelID(windows::core::w!("Petal.Converter"))
                .map_err(error)?;
            let exe = std::env::current_exe().map_err(error)?;
            let image = crate::runtime::logo_file()?;
            let image_uri = file_uri(&image)?;
            let (attribution_icon, shortcut_icon) = crate::runtime::attribution_icon_files()?;
            register_string("DisplayName", "Petal")?;
            register_string("IconUri", &attribution_icon.to_string_lossy())?;
            register_string("IconBackgroundColor", "FF0C1015")?;
            shortcut(&exe, &shortcut_icon)?;
            let notifier =
                ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
                    .map_err(error)?;
            Ok(Self {
                notifier,
                image_uri,
            })
        }
    }
    pub fn show(&self, title: &str, message: &str) -> Result<(), String> {
        let xml = XmlDocument::new().map_err(error)?;
        xml.LoadXml(&HSTRING::from(payload(title, message, &self.image_uri)))
            .map_err(error)?;
        let toast = ToastNotification::CreateToastNotification(&xml).map_err(error)?;
        self.notifier.Show(&toast).map_err(error)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logo_path_is_a_valid_file_uri() {
        assert_eq!(
            file_uri(Path::new("C:\\Apps\\Petal #1\\logo.png")).unwrap(),
            "file:///C:/Apps/Petal%20%231/logo.png"
        );
        assert_eq!(
            file_uri(Path::new("C:\\Apps\\\u{00e9}\\logo.png")).unwrap(),
            "file:///C:/Apps/%C3%A9/logo.png"
        );
    }
    #[test]
    fn toast_contains_explicit_logo_and_escapes_file_names() {
        let text = payload(
            "Conversion complete",
            "image <1> & 2.webp",
            "file:///C:/Petal/assets/petal.png",
        );
        let doc = roxmltree::Document::parse(&text).unwrap();
        let image = doc.descendants().find(|n| n.has_tag_name("image")).unwrap();
        assert_eq!(image.attribute("placement"), Some("appLogoOverride"));
        assert_eq!(
            image.attribute("src"),
            Some("file:///C:/Petal/assets/petal.png")
        );
        let messages: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .map(|n| n.text().unwrap())
            .collect();
        assert_eq!(messages, ["Conversion complete", "image <1> & 2.webp"]);
        assert!(doc
            .descendants()
            .any(|n| n.has_tag_name("audio") && n.attribute("silent") == Some("true")));
    }
}

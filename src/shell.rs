use std::path::PathBuf;
use windows::{
    core::{Interface, VARIANT},
    Win32::{
        Foundation::POINT,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch, IServiceProvider,
            CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement,
                IUIAutomationSelectionItemPattern, UIA_ListItemControlTypeId,
                UIA_SelectionItemPatternId,
            },
            Shell::{
                IShellBrowser, IShellFolderViewDual, IShellWindows, IWebBrowserApp,
                SID_STopLevelBrowser, ShellWindows, SVGIO_BACKGROUND, SWC_DESKTOP,
                SWFO_NEEDDISPATCH,
            },
        },
    },
};

pub fn desktop_host_class(class: &str) -> bool {
    matches!(class, "Progman" | "WorkerW")
}

pub unsafe fn is_desktop_root(hwnd: isize) -> bool {
    let mut name = [0u16; 128];
    let len = windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(
        hwnd as _,
        name.as_mut_ptr(),
        128,
    );
    desktop_host_class(&String::from_utf16_lossy(&name[..len as usize]))
}

unsafe fn desktop_view(windows: &IShellWindows) -> windows::core::Result<IShellFolderViewDual> {
    let location = VARIANT::from(0);
    let empty = VARIANT::default();
    let mut hwnd = 0;
    let dispatch =
        windows.FindWindowSW(&location, &empty, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
    let provider: IServiceProvider = dispatch.cast()?;
    let browser: IShellBrowser = provider.QueryService(&SID_STopLevelBrowser)?;
    let shell_view = browser.QueryActiveShellView()?;
    let dispatch: IDispatch = shell_view.GetItemObject(SVGIO_BACKGROUND)?;
    dispatch.cast()
}

unsafe fn selected_file(
    view: &IShellFolderViewDual,
    element: &IUIAutomationElement,
    hit_name: &str,
) -> windows::core::Result<Option<PathBuf>> {
    if view.SelectedItems()?.Count()? > 1 {
        return Ok(None);
    }
    let selection: IUIAutomationSelectionItemPattern =
        element.GetCurrentPatternAs(UIA_SelectionItemPatternId)?;
    selection.Select()?;
    let selected = view.SelectedItems()?;
    if selected.Count()? != 1 {
        return Ok(None);
    }
    let item = selected.Item(&VARIANT::from(0))?;
    let name = item.Name()?.to_string();
    let path = PathBuf::from(item.Path()?.to_string());
    // Compare the clicked icon with the Shell item, including hidden file extensions.
    if hit_name != name
        && path
            .file_name()
            .is_none_or(|n| n.to_string_lossy() != hit_name)
    {
        return Ok(None);
    }
    Ok((path.is_file() && crate::convert::supported(&path)).then_some(path))
}

// Called on a worker, never from the low-level mouse hook.
pub fn file_at(hwnd: isize, x: i32, y: i32) -> Option<PathBuf> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok()?;
        let result = find(hwnd, x, y);
        CoUninitialize();
        result.ok().flatten()
    }
}

unsafe fn find(hwnd: isize, x: i32, y: i32) -> windows::core::Result<Option<PathBuf>> {
    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)?;
    let mut element = automation.ElementFromPoint(POINT { x, y })?;
    let walker = automation.ControlViewWalker()?;
    let mut hit_name = None;
    for _ in 0..8 {
        if element.CurrentControlType()? == UIA_ListItemControlTypeId {
            hit_name = Some(element.CurrentName()?.to_string());
            break;
        }
        element = match walker.GetParentElement(&element) {
            Ok(e) => e,
            Err(_) => break,
        };
    }
    let Some(hit_name) = hit_name else {
        return Ok(None);
    };
    let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
    if is_desktop_root(hwnd) {
        return selected_file(&desktop_view(&windows)?, &element, &hit_name);
    }
    for i in 0..windows.Count()? {
        let Ok(browser) = windows
            .Item(&VARIANT::from(i))
            .and_then(|d| d.cast::<IWebBrowserApp>())
        else {
            continue;
        };
        if browser.HWND()?.0 != hwnd {
            continue;
        }
        let Ok(view) = browser
            .Document()
            .and_then(|d| d.cast::<IShellFolderViewDual>())
        else {
            continue;
        };
        return selected_file(&view, &element, &hit_name);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_hosts_exclude_taskbar_and_other_apps() {
        assert!(desktop_host_class("Progman"));
        assert!(desktop_host_class("WorkerW"));
        for class in [
            "Shell_TrayWnd",
            "CabinetWClass",
            "Chrome_WidgetWin_1",
            "SHELLDLL_DefView",
            "",
        ] {
            assert!(!desktop_host_class(class));
        }
    }
    #[test]
    #[ignore = "Requires the interactive Windows desktop, outside the build sandbox"]
    fn interactive_desktop_view_is_available() {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            let result = (|| -> windows::core::Result<i32> {
                let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
                let view = desktop_view(&windows)?;
                view.SelectedItems()?.Count()
            })();
            CoUninitialize();
            assert!(result.unwrap() >= 0);
        }
    }
}

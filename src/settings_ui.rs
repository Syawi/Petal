use crate::{render, settings, wide};
use std::{
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::Dialogs::*,
        Controls::*,
        HiDpi::*,
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
const RECORDED: u32 = WM_APP + 40;
const CANCEL_CAPTURE: u32 = WM_APP + 41;
const STARTUP: i32 = 101;
const SHORTCUT: i32 = 102;
const RESET: i32 = 103;
const SOUND: i32 = 104;
const CHOOSE: i32 = 105;
const PREVIEW: i32 = 106;
const DEFAULT_SOUND: i32 = 107;
const DONE: i32 = 108;
const LICENSES: i32 = 109;
const THEME_GLASS: i32 = 110;
const THEME_CLASSIC: i32 = 111;
static PANEL: AtomicUsize = AtomicUsize::new(0);
static CAPTURING: AtomicBool = AtomicBool::new(false);
struct Panel {
    owner: HWND,
    scale: f64,
    binding: settings::Binding,
    sound: bool,
    startup: bool,
    /// Radial menu theme: Liquid glass or Classic.
    glass: bool,
    sound_file: Option<std::path::PathBuf>,
    status: String,
    hover: usize,
    fonts: Vec<HFONT>,
    background: HBRUSH,
}
fn rgb(value: u32) -> u32 {
    ((value >> 16) & 255) | (value & 0xff00) | ((value & 255) << 16)
}
fn colors() -> (u32, u32, u32, u32, u32) {
    if render::dark_theme() {
        (0x202020, 0x292929, 0xf6f6f6, 0xb9b9b9, 0x404040)
    } else {
        (0xf3f3f3, 0xffffff, 0x191919, 0x595959, 0xd8d8d8)
    }
}
unsafe fn data(hwnd: HWND) -> &'static mut Panel {
    &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Panel)
}
pub unsafe fn dialog_message(msg: &MSG) -> bool {
    let hwnd = PANEL.load(Ordering::Relaxed) as HWND;
    !hwnd.is_null() && IsDialogMessageW(hwnd, msg) != 0
}
pub fn capturing() -> bool {
    CAPTURING.load(Ordering::Relaxed)
}
pub unsafe fn record(key: u16, modifiers: u8) -> bool {
    let hwnd = PANEL.load(Ordering::Relaxed) as HWND;
    if !capturing() || hwnd.is_null() || GetForegroundWindow() != hwnd {
        return false;
    }
    if (settings::Binding { key, modifiers }).mouse() {
        let mut point = zeroed();
        GetCursorPos(&mut point);
        if GetAncestor(WindowFromPoint(point), GA_ROOT) != hwnd {
            CAPTURING.store(false, Ordering::Relaxed);
            PostMessageW(hwnd, CANCEL_CAPTURE, 0, 0);
            return false;
        }
    }
    if settings::modifier_key(key) {
        return false;
    }
    CAPTURING.store(false, Ordering::Relaxed);
    if key == VK_ESCAPE {
        PostMessageW(hwnd, CANCEL_CAPTURE, 0, 0);
    } else {
        PostMessageW(
            hwnd,
            RECORDED,
            settings::Binding { key, modifiers }.encode() as usize,
            0,
        );
    }
    true
}
pub unsafe fn close() {
    let hwnd = PANEL.load(Ordering::Relaxed) as HWND;
    if !hwnd.is_null() {
        DestroyWindow(hwnd);
    }
}
unsafe fn font(size: i32, weight: u32, scale: f64) -> HFONT {
    CreateFontW(
        -(size as f64 * scale).round() as i32,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Segoe UI").as_ptr(),
    )
}
unsafe fn rect(x: i32, y: i32, w: i32, h: i32, s: f64) -> RECT {
    RECT {
        left: (x as f64 * s).round() as i32,
        top: (y as f64 * s).round() as i32,
        right: ((x + w) as f64 * s).round() as i32,
        bottom: ((y + h) as f64 * s).round() as i32,
    }
}
unsafe fn text(dc: HDC, font: HFONT, color: u32, value: &str, mut area: RECT, flags: u32) {
    SelectObject(dc, font);
    SetTextColor(dc, rgb(color));
    SetBkMode(dc, TRANSPARENT as i32);
    DrawTextW(dc, wide(value).as_ptr(), -1, &mut area, flags);
}
unsafe fn rounded(dc: HDC, area: RECT, fill: u32, border: u32, radius: i32) {
    let brush = CreateSolidBrush(rgb(fill));
    let pen = CreatePen(PS_SOLID, 1, rgb(border));
    let oldbrush = SelectObject(dc, brush);
    let oldpen = SelectObject(dc, pen);
    RoundRect(
        dc,
        area.left,
        area.top,
        area.right,
        area.bottom,
        radius,
        radius,
    );
    SelectObject(dc, oldbrush);
    SelectObject(dc, oldpen);
    DeleteObject(brush);
    DeleteObject(pen);
}
unsafe extern "system" fn button_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let parent = GetParent(hwnd);
    if !GetWindowLongPtrW(parent, GWLP_USERDATA).eq(&0) {
        if msg == WM_MOUSEMOVE {
            let panel = data(parent);
            panel.hover = GetDlgCtrlID(hwnd) as usize;
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut track);
            InvalidateRect(hwnd, null(), 0);
        }
        if msg == WM_MOUSELEAVE {
            data(parent).hover = 0;
            InvalidateRect(hwnd, null(), 0);
        }
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
unsafe fn button(hwnd: HWND, id: i32, label: &str, x: i32, y: i32, w: i32, h: i32) {
    let panel = data(hwnd);
    let area = rect(x, y, w, h, panel.scale);
    let child = CreateWindowExW(
        0,
        wide("BUTTON").as_ptr(),
        wide(label).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        area.left,
        area.top,
        area.right - area.left,
        area.bottom - area.top,
        hwnd,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(child, WM_SETFONT, panel.fonts[2] as usize, 0);
    SetWindowSubclass(child, Some(button_proc), 1, 0);
}
unsafe fn label(hwnd: HWND, id: i32, value: &str) {
    SetWindowTextW(GetDlgItem(hwnd, id), wide(value).as_ptr());
    InvalidateRect(GetDlgItem(hwnd, id), null(), 0);
}
unsafe fn refresh(hwnd: HWND) {
    let panel = data(hwnd);
    label(
        hwnd,
        SHORTCUT,
        &if capturing() {
            "Press keys or a mouse button…".into()
        } else {
            panel.binding.label()
        },
    );
    label(
        hwnd,
        STARTUP,
        if panel.startup {
            "Launch at sign-in: On"
        } else {
            "Launch at sign-in: Off"
        },
    );
    label(
        hwnd,
        SOUND,
        if panel.sound {
            "Completion sound: On"
        } else {
            "Completion sound: Off"
        },
    );
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn changed(hwnd: HWND, result: Result<(), String>) {
    let panel = data(hwnd);
    panel.status = match result {
        Ok(()) => "Changes saved automatically.".into(),
        Err(e) => e,
    };
    refresh(hwnd);
}
unsafe fn publish_settings(hwnd: HWND) {
    PostMessageW(data(hwnd).owner, crate::SETTINGS_CHANGED, 0, 0);
}
unsafe fn paint(hwnd: HWND) {
    let panel = data(hwnd);
    let (bg, surface, fg, muted, border) = colors();
    let s = panel.scale;
    let mut ps = zeroed();
    let target = BeginPaint(hwnd, &mut ps);
    let dc = CreateCompatibleDC(target);
    let mut client = zeroed();
    GetClientRect(hwnd, &mut client);
    let bitmap = CreateCompatibleBitmap(target, client.right, client.bottom);
    let old = SelectObject(dc, bitmap);
    let brush = CreateSolidBrush(rgb(bg));
    FillRect(dc, &client, brush);
    DeleteObject(brush);
    let icon = LoadImageW(
        GetModuleHandleW(null()),
        1usize as *const u16,
        IMAGE_ICON,
        (44.0 * s) as i32,
        (44.0 * s) as i32,
        LR_SHARED,
    ) as HICON;
    DrawIconEx(
        dc,
        (32.0 * s) as i32,
        (29.0 * s) as i32,
        icon,
        (44.0 * s) as i32,
        (44.0 * s) as i32,
        0,
        null_mut(),
        DI_NORMAL,
    );
    text(
        dc,
        panel.fonts[0],
        fg,
        "Settings",
        rect(92, 23, 490, 39, s),
        DT_SINGLELINE,
    );
    text(
        dc,
        panel.fonts[3],
        muted,
        "Petal, your way.",
        rect(94, 65, 480, 24, s),
        DT_SINGLELINE,
    );
    for (y, h) in [(112, 126), (254, 88), (358, 162)] {
        rounded(
            dc,
            rect(32, y, 576, h, s),
            surface,
            border,
            (14.0 * s) as i32,
        );
    }
    text(
        dc,
        panel.fonts[1],
        fg,
        "Open Petal",
        rect(50, 127, 500, 25, s),
        DT_SINGLELINE,
    );
    text(
        dc,
        panel.fonts[3],
        muted,
        "Point at a file, then use your shortcut.",
        rect(50, 157, 520, 23, s),
        DT_SINGLELINE,
    );
    text(
        dc,
        panel.fonts[1],
        fg,
        "Launch at sign-in",
        rect(50, 272, 440, 26, s),
        DT_SINGLELINE,
    );
    text(
        dc,
        panel.fonts[3],
        muted,
        "Start quietly in the system tray.",
        rect(50, 303, 440, 23, s),
        DT_SINGLELINE,
    );
    text(
        dc,
        panel.fonts[1],
        fg,
        "Completion sound",
        rect(50, 376, 440, 26, s),
        DT_SINGLELINE,
    );
    let sound = panel
        .sound_file
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Paid · built-in sound".into());
    text(
        dc,
        panel.fonts[3],
        muted,
        &sound,
        rect(50, 411, 510, 24, s),
        DT_SINGLELINE | DT_END_ELLIPSIS,
    );
    let status = if capturing() {
        "Press a shortcut, or Esc to cancel. Windows may reserve some keys."
    } else {
        &panel.status
    };
    text(
        dc,
        panel.fonts[3],
        muted,
        status,
        rect(34, 545, 440, 54, s),
        DT_WORDBREAK,
    );
    BitBlt(target, 0, 0, client.right, client.bottom, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old);
    DeleteObject(bitmap);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}
unsafe fn draw_button(hwnd: HWND, draw: &DRAWITEMSTRUCT) {
    let panel = data(hwnd);
    let (_, surface, fg, _, border) = colors();
    let accent = render::system_accent() & 0xffffff;
    let id = draw.CtlID as i32;
    let toggle = id == STARTUP || id == SOUND;
    let on = match id {
        STARTUP => panel.startup,
        SOUND => panel.sound,
        THEME_GLASS => panel.glass,
        THEME_CLASSIC => !panel.glass,
        _ => false,
    };
    // Toggles and the chosen theme show as selected (accent fill, black text).
    let selected = on && (toggle || id == THEME_GLASS || id == THEME_CLASSIC);
    let hover = panel.hover == id as usize || draw.itemState & ODS_SELECTED != 0;
    let fill = if selected || hover { accent } else { surface };
    let color = if selected || hover { 0 } else { fg };
    let area = draw.rcItem;
    rounded(
        draw.hDC,
        area,
        fill,
        if hover { accent } else { border },
        (9.0 * panel.scale) as i32,
    );
    if toggle {
        let size = (18.0 * panel.scale) as i32;
        let margin = (6.0 * panel.scale) as i32;
        let x = if on {
            area.right - size - margin
        } else {
            area.left + margin
        };
        let cy = (area.top + area.bottom) / 2;
        let brush = CreateSolidBrush(rgb(color));
        let old = SelectObject(draw.hDC, brush);
        let oldpen = SelectObject(draw.hDC, GetStockObject(NULL_PEN));
        Ellipse(draw.hDC, x, cy - size / 2, x + size, cy + size / 2);
        SelectObject(draw.hDC, old);
        SelectObject(draw.hDC, oldpen);
        DeleteObject(brush);
    } else {
        let mut value = [0u16; 256];
        let count = GetWindowTextW(draw.hwndItem, value.as_mut_ptr(), 256);
        text(
            draw.hDC,
            panel.fonts[2],
            color,
            &String::from_utf16_lossy(&value[..count as usize]),
            area,
            DT_SINGLELINE | DT_CENTER | DT_VCENTER | DT_END_ELLIPSIS,
        );
    }
    if draw.itemState & ODS_FOCUS != 0 {
        let mut focus = area;
        InflateRect(&mut focus, -3, -3);
        DrawFocusRect(draw.hDC, &focus);
    }
}
unsafe fn choose_sound(hwnd: HWND) -> Result<(), String> {
    let mut path = vec![0u16; 32768];
    let filter = wide("WAV audio (*.wav)\0*.wav\0\0");
    let mut dialog: OPENFILENAMEW = zeroed();
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = hwnd;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    let title = wide("Choose a completion sound");
    dialog.lpstrTitle = title.as_ptr();
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER;
    if GetOpenFileNameW(&mut dialog) == 0 {
        let error = CommDlgExtendedError();
        return if error == 0 {
            Ok(())
        } else {
            Err(format!("Could not open the sound picker ({error})."))
        };
    }
    let end = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    let selected = std::path::PathBuf::from(String::from_utf16_lossy(&path[..end]));
    crate::sound::validate_file(&selected)?;
    settings::save_sound_file(Some(&selected))?;
    data(hwnd).sound_file = Some(selected);
    Ok(())
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if GetWindowLongPtrW(hwnd, GWLP_USERDATA) == 0 {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DRAWITEM => {
            draw_button(hwnd, &*(lp as *const DRAWITEMSTRUCT));
            1
        }
        RECORDED => {
            if let Some(binding) = settings::Binding::decode(wp as u32) {
                let result = settings::save_binding(binding);
                if result.is_ok() {
                    data(hwnd).binding = binding;
                    publish_settings(hwnd);
                }
                changed(hwnd, result);
            }
            0
        }
        CANCEL_CAPTURE => {
            CAPTURING.store(false, Ordering::Relaxed);
            refresh(hwnd);
            0
        }
        WM_COMMAND => {
            let id = (wp & 0xffff) as i32;
            CAPTURING.store(false, Ordering::Relaxed);
            let result = match id {
                STARTUP => {
                    let enabled = !data(hwnd).startup;
                    let result = settings::save_startup(enabled);
                    if result.is_ok() {
                        data(hwnd).startup = enabled;
                    }
                    result
                }
                SHORTCUT => {
                    CAPTURING.store(true, Ordering::Relaxed);
                    SetFocus(hwnd);
                    refresh(hwnd);
                    return 0;
                }
                RESET => {
                    let binding = settings::Binding::default();
                    let result = settings::save_binding(binding);
                    if result.is_ok() {
                        data(hwnd).binding = binding;
                    }
                    result
                }
                SOUND => {
                    let enabled = !data(hwnd).sound;
                    let result = settings::save_sound(enabled);
                    if result.is_ok() {
                        data(hwnd).sound = enabled;
                        if !enabled {
                            crate::sound::stop();
                        }
                    }
                    result
                }
                THEME_GLASS | THEME_CLASSIC => {
                    let glass = id == THEME_GLASS;
                    let result = settings::save(settings::GLASS, glass);
                    if result.is_ok() {
                        data(hwnd).glass = glass;
                        RedrawWindow(hwnd, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
                    }
                    result
                }
                CHOOSE => choose_sound(hwnd),
                PREVIEW => {
                    crate::sound::success();
                    Ok(())
                }
                LICENSES => crate::runtime::open_licenses(hwnd),
                DEFAULT_SOUND => {
                    let result = settings::save_sound_file(None);
                    if result.is_ok() {
                        data(hwnd).sound_file = None;
                    }
                    result
                }
                DONE => {
                    DestroyWindow(hwnd);
                    return 0;
                }
                _ => Ok(()),
            };
            publish_settings(hwnd);
            changed(hwnd, result);
            0
        }
        WM_KEYDOWN if wp as u16 == VK_ESCAPE => {
            if capturing() {
                CAPTURING.store(false, Ordering::Relaxed);
                refresh(hwnd);
            } else {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_ACTIVATE if wp as u16 == WA_INACTIVE as u16 => {
            CAPTURING.store(false, Ordering::Relaxed);
            refresh(hwnd);
            0
        }
        WM_DPICHANGED => {
            let area = &*(lp as *const RECT);
            let panel = data(hwnd);
            panel.scale = ((wp >> 16) & 0xffff) as f64 / 96.0;
            let old = std::mem::replace(
                &mut panel.fonts,
                vec![
                    font(29, 700, panel.scale),
                    font(17, 700, panel.scale),
                    font(14, 600, panel.scale),
                    font(13, 400, panel.scale),
                ],
            );
            for (id, x, y, w, h) in [
                (THEME_GLASS, 380, 40, 112, 34),
                (THEME_CLASSIC, 496, 40, 112, 34),
                (SHORTCUT, 50, 188, 390, 34),
                (RESET, 454, 188, 136, 34),
                (STARTUP, 528, 281, 60, 30),
                (SOUND, 528, 384, 60, 30),
                (CHOOSE, 50, 459, 156, 36),
                (PREVIEW, 218, 459, 112, 36),
                (DEFAULT_SOUND, 342, 459, 144, 36),
                (DONE, 496, 544, 112, 38),
                (LICENSES, 34, 578, 132, 25),
            ] {
                let child = GetDlgItem(hwnd, id);
                let bounds = rect(x, y, w, h, panel.scale);
                MoveWindow(
                    child,
                    bounds.left,
                    bounds.top,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    0,
                );
                SendMessageW(child, WM_SETFONT, panel.fonts[2] as usize, 0);
            }
            for font in old {
                DeleteObject(font);
            }
            SetWindowPos(
                hwnd,
                null_mut(),
                area.left,
                area.top,
                area.right - area.left,
                area.bottom - area.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            RedrawWindow(hwnd, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DWMCOLORIZATIONCOLORCHANGED => {
            let dark = i32::from(render::dark_theme());
            DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as *const _, 4);
            let (bg, _, _, _, _) = colors();
            let panel = data(hwnd);
            DeleteObject(panel.background);
            panel.background = CreateSolidBrush(rgb(bg));
            RedrawWindow(hwnd, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
            0
        }
        WM_CTLCOLORBTN => data(hwnd).background as isize,
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_NCDESTROY => {
            CAPTURING.store(false, Ordering::Relaxed);
            PANEL.store(0, Ordering::Relaxed);
            let panel = Box::from_raw(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Panel);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            for f in &panel.fonts {
                DeleteObject(*f);
            }
            DeleteObject(panel.background);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
pub unsafe fn show(owner: HWND) {
    let existing = PANEL.load(Ordering::Relaxed) as HWND;
    if !existing.is_null() {
        ShowWindow(existing, SW_RESTORE);
        SetForegroundWindow(existing);
        return;
    }
    let instance = GetModuleHandleW(null());
    let class = wide("Petal.Settings");
    let mut wc: WNDCLASSEXW = zeroed();
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(procedure);
    wc.hInstance = instance;
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.hIcon = LoadIconW(instance, 1usize as *const u16);
    wc.hIconSm = wc.hIcon;
    wc.lpszClassName = class.as_ptr();
    RegisterClassExW(&wc);
    let dpi = GetDpiForSystem().max(96);
    let scale = dpi as f64 / 96.0;
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let ex = WS_EX_APPWINDOW | WS_EX_CONTROLPARENT;
    let mut bounds = rect(0, 0, 640, 610, scale);
    AdjustWindowRectExForDpi(&mut bounds, style, 0, ex, dpi);
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    let mut point = zeroed();
    GetCursorPos(&mut point);
    let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    let y = info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 2;
    let hwnd = CreateWindowExW(
        ex,
        class.as_ptr(),
        wide("Petal Settings").as_ptr(),
        style,
        x,
        y,
        width,
        height,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if hwnd.is_null() {
        return;
    }
    let (bg, _, _, _, _) = colors();
    let panel = Box::new(Panel {
        owner,
        scale,
        binding: settings::binding(),
        sound: settings::sound_enabled(),
        startup: settings::startup_enabled(),
        glass: settings::enabled(settings::GLASS),
        sound_file: settings::sound_file(),
        status: "Changes saved automatically.".into(),
        hover: 0,
        fonts: vec![
            font(29, 700, scale),
            font(17, 700, scale),
            font(14, 600, scale),
            font(13, 400, scale),
        ],
        background: CreateSolidBrush(rgb(bg)),
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(panel) as isize);
    PANEL.store(hwnd as usize, Ordering::Relaxed);
    button(hwnd, THEME_GLASS, "Liquid glass", 380, 40, 112, 34);
    button(hwnd, THEME_CLASSIC, "Classic", 496, 40, 112, 34);
    button(hwnd, SHORTCUT, "", 50, 188, 390, 34);
    button(hwnd, RESET, "Reset", 454, 188, 136, 34);
    button(hwnd, STARTUP, "", 528, 281, 60, 30);
    button(hwnd, SOUND, "", 528, 384, 60, 30);
    button(hwnd, CHOOSE, "Choose WAV…", 50, 459, 156, 36);
    button(hwnd, PREVIEW, "Preview", 218, 459, 112, 36);
    button(hwnd, DEFAULT_SOUND, "Use default", 342, 459, 144, 36);
    button(hwnd, DONE, "Done", 496, 544, 112, 38);
    button(hwnd, LICENSES, "Licenses", 34, 578, 132, 25);
    let dark = i32::from(render::dark_theme());
    DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as *const _, 4);
    let corners = 2i32;
    DwmSetWindowAttribute(hwnd, 33, &corners as *const _ as *const _, 4);
    refresh(hwnd);
    ShowWindow(hwnd, SW_SHOW);
    SetWindowPos(
        hwnd,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
    );
    SetForegroundWindow(hwnd);
    SetFocus(GetDlgItem(hwnd, SHORTCUT));
}

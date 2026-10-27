//! The liquid glass settings window, opened from the tray icon.
use crate::{render, settings, wide};
use std::{
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::WM_MOUSELEAVE, HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::*,
        WindowsAndMessaging::*,
    },
};

const OPEN_MS: f32 = 260.0;
const CLOSE_MS: f32 = 140.0;
const SWITCH_MS: f32 = 180.0;
const TIMER: usize = 1;
const NAMES: [&str; 3] = [settings::GLASS, settings::MOTION, settings::SOUND];
/// WM_COMMAND ids the main window already handles (save + apply) for each row.
const COMMANDS: [usize; 3] = [4, 5, 3];

struct Window {
    hwnd: HWND,
    owner: HWND,
    scale: f64,
    dark: bool,
    accent: u32,
    glass: Option<Vec<u32>>,
    motion: bool,
    opened: Instant,
    closing: Option<Instant>,
    on: [bool; 3],
    from: [f32; 3],
    flipped: [Option<Instant>; 3],
    hover: i32,
}
static mut PANEL: *mut Window = null_mut();

fn ms(since: Instant) -> f32 {
    since.elapsed().as_secs_f32() * 1000.0
}
impl Window {
    fn switch(&self, i: usize) -> f32 {
        let target = if self.on[i] { 1.0 } else { 0.0 };
        let t = self.flipped[i].map_or(1.0, |s| (ms(s) / SWITCH_MS).min(1.0));
        self.from[i] + (target - self.from[i]) * (1.0 - (1.0 - t).powi(3))
    }
    fn animating(&self) -> bool {
        self.closing.is_some()
            || (self.motion && ms(self.opened) < OPEN_MS)
            || self.flipped.iter().flatten().any(|&s| ms(s) < SWITCH_MS)
    }
    unsafe fn paint(&self) {
        let open = if self.motion { (ms(self.opened) / OPEN_MS).min(1.0) } else { 1.0 };
        let fade = match self.closing {
            Some(start) => 1.0 - (ms(start) / CLOSE_MS).min(1.0),
            None => (open * 3.0).min(1.0),
        };
        render::paint_panel(
            self.hwnd,
            self.scale,
            self.dark,
            &render::Panel {
                accent: self.accent,
                glass: self.glass.as_deref(),
                switches: std::array::from_fn(|i| self.switch(i)),
                hover: self.hover,
                open,
                alpha: (fade * 255.0).round() as u8,
            },
        );
    }
    unsafe fn animate(&self) {
        SetTimer(self.hwnd, TIMER, 16, None);
    }
    unsafe fn close(&mut self) {
        if IsWindowVisible(self.hwnd) == 0 || self.closing.is_some() {
            return;
        }
        if self.motion {
            self.closing = Some(Instant::now());
            self.animate();
        } else {
            self.hide();
        }
    }
    unsafe fn hide(&mut self) {
        KillTimer(self.hwnd, TIMER);
        ShowWindow(self.hwnd, SW_HIDE);
        self.closing = None;
        self.glass = None;
    }
    unsafe fn act(&mut self, target: i32) {
        match target {
            0..=2 => {
                let i = target as usize;
                self.from[i] = self.switch(i);
                SendMessageW(self.owner, WM_COMMAND, COMMANDS[i], 0);
                self.on[i] = settings::enabled(NAMES[i]);
                self.flipped[i] = self.motion.then(Instant::now);
                if i == 1 {
                    self.motion = self.on[1];
                }
                self.animate();
                self.paint();
            }
            3 => self.close(),
            _ => {}
        }
    }
}
/// Opens (or focuses) the panel in the corner of the screen next to the tray.
pub unsafe fn open(owner: HWND, accent: u32, dark: bool) {
    if PANEL.is_null() {
        let Some(hwnd) = create() else { return };
        PANEL = Box::into_raw(Box::new(Window {
            hwnd,
            owner,
            scale: 1.0,
            dark,
            accent,
            glass: None,
            motion: true,
            opened: Instant::now(),
            closing: None,
            on: [true; 3],
            from: [1.0; 3],
            flipped: [None; 3],
            hover: -1,
        }));
    }
    let p = &mut *PANEL;
    if IsWindowVisible(p.hwnd) != 0 && p.closing.is_none() {
        SetForegroundWindow(p.hwnd);
        return;
    }
    let mut cursor = zeroed();
    GetCursorPos(&mut cursor);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST), &mut info);
    let a = info.rcWork;
    // Park the hidden window on the target monitor first so it reports that monitor's DPI.
    SetWindowPos(p.hwnd, HWND_TOPMOST, a.right - 1, a.bottom - 1, 1, 1, SWP_NOACTIVATE);
    p.scale = GetDpiForWindow(p.hwnd).max(96) as f64 / 96.0;
    let (w, h) = (
        (render::PANEL_W as f64 * p.scale).round() as i32,
        (render::PANEL_H as f64 * p.scale).round() as i32,
    );
    let margin = (12.0 * p.scale) as i32;
    let (x, y) = (a.right - w - margin, a.bottom - h - margin);
    p.dark = dark;
    p.accent = accent;
    p.on = NAMES.map(settings::enabled);
    p.from = p.on.map(|on| if on { 1.0 } else { 0.0 });
    p.flipped = [None; 3];
    p.motion = p.on[1];
    p.glass = p.on[0]
        .then(|| render::capture_glass(x, y, w, h, p.scale, dark, render::panel_bend))
        .flatten();
    p.hover = -1;
    p.closing = None;
    p.opened = Instant::now();
    SetWindowPos(p.hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW);
    p.paint();
    SetForegroundWindow(p.hwnd);
    p.animate();
}
unsafe fn create() -> Option<HWND> {
    let instance = GetModuleHandleW(null());
    let class = wide("Petal.Settings");
    let mut wc: WNDCLASSEXW = zeroed();
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(proc);
    wc.hInstance = instance;
    wc.lpszClassName = class.as_ptr();
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    RegisterClassExW(&wc);
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        class.as_ptr(),
        wide("Petal settings").as_ptr(),
        WS_POPUP,
        0,
        0,
        1,
        1,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    (!hwnd.is_null()).then_some(hwnd)
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if PANEL.is_null() || (*PANEL).hwnd != hwnd {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    let p = &mut *PANEL;
    let at = |lp: LPARAM| {
        (
            (lp as u16 as i16) as f32 / p.scale as f32,
            (((lp as u32) >> 16) as u16 as i16) as f32 / p.scale as f32,
        )
    };
    match msg {
        WM_TIMER => {
            if p.closing.is_some_and(|s| ms(s) >= CLOSE_MS) {
                p.hide();
                return 0;
            }
            p.paint();
            if !p.animating() {
                KillTimer(hwnd, TIMER);
            }
            0
        }
        WM_PAINT => {
            p.paint();
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            let (x, y) = at(lp);
            let hover = render::panel_hit(x, y);
            if hover != p.hover {
                p.hover = hover;
                p.paint();
            }
            0
        }
        WM_MOUSELEAVE => {
            p.hover = -1;
            p.paint();
            0
        }
        WM_SETCURSOR if p.hover >= 0 => {
            SetCursor(LoadCursorW(null_mut(), IDC_HAND));
            1
        }
        WM_LBUTTONUP => {
            let (x, y) = at(lp);
            p.act(render::panel_hit(x, y));
            0
        }
        WM_KEYDOWN => {
            match wp as u16 {
                VK_ESCAPE => p.close(),
                VK_DOWN | VK_TAB => {
                    p.hover = (p.hover + 1).rem_euclid(4);
                    p.paint();
                }
                VK_UP => {
                    p.hover = (p.hover - 1).rem_euclid(4);
                    p.paint();
                }
                VK_SPACE | VK_RETURN => p.act(p.hover),
                _ => {}
            }
            0
        }
        WM_ACTIVATE if wp as u16 == WA_INACTIVE as u16 => {
            p.close();
            0
        }
        WM_CLOSE => {
            p.close();
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#![cfg_attr(not(test), windows_subsystem = "windows")]
mod convert;
mod documents;
mod media;
mod notifications;
mod panel;
mod pdf;
mod render;
mod settings;
mod shell;
mod sound;
mod text_pdf;
use std::{
    f64::consts::{PI, TAU},
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::*},
    UI::{
        Controls::WM_MOUSELEAVE, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
        WindowsAndMessaging::*,
    },
};
const HIT: u32 = WM_APP + 1;
const TRAY: u32 = WM_APP + 2;
const DISMISS: u32 = WM_APP + 3;
const WIDTH: i32 = 400;
const HEIGHT: i32 = 400;
const MENU_SCALE: f64 = 0.75;
const FRAME_TIMER: usize = 2;
const MIN_WORKING_MS: u128 = 450;
const SUCCESS_MS: u128 = 1200;
static WINDOW: AtomicUsize = AtomicUsize::new(0);
static mut DOWN: Option<(POINT, usize)> = None;
static mut STATE: *mut State = null_mut();
enum Reply {
    Selection(Option<PathBuf>),
    Converted(Result<PathBuf, String>),
}
struct State {
    dark: bool,
    accent: u32,
    motion: bool,
    sound_enabled: bool,
    glass_enabled: bool,
    motion_enabled: bool,
    glass: Option<Vec<u32>>,
    notifications: Option<notifications::Notifications>,
    hover_from: [f32; 7],
    hover_started: Option<Instant>,
    work_started: Option<Instant>,
    success_started: Option<Instant>,
    formats: [usize; 6],
    pending_conversion: Option<Result<PathBuf, String>>,
    animation_started: Option<Instant>,
    source: Option<PathBuf>,
    point: POINT,
    explorer: HWND,
    source_foreground: HWND,
    hover: i32,
    visible: bool,
    busy: bool,
    resolving: bool,
    selection_dismissed: bool,
    scale: f64,
    sender: mpsc::Sender<Reply>,
    receiver: mpsc::Receiver<Reply>,
}
impl State {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            dark: render::dark_theme(),
            accent: render::system_accent(),
            // Petal's own Animations setting wins over the system-wide toggle, which many PCs turn off for speed.
            motion: settings::enabled(settings::MOTION),
            sound_enabled: settings::enabled(settings::SOUND),
            glass_enabled: settings::enabled(settings::GLASS),
            motion_enabled: settings::enabled(settings::MOTION),
            glass: None,
            notifications: None,
            hover_from: [0.0; 7],
            hover_started: None,
            work_started: None,
            success_started: None,
            formats: [0, 1, 2, 3, 4, 5],
            pending_conversion: None,
            animation_started: None,
            source: None,
            point: POINT { x: 0, y: 0 },
            explorer: null_mut(),
            source_foreground: null_mut(),
            hover: -1,
            visible: false,
            busy: false,
            resolving: false,
            selection_dismissed: false,
            scale: 1.0,
            sender,
            receiver,
        }
    }
    fn animating(&self) -> bool {
        self.visible
            && self.motion
            && (self.animation_started.is_some()
                || self.hover_started.is_some()
                || self.busy
                || self
                    .success_started
                    .is_some_and(|s| s.elapsed().as_millis() < render::CHECK_MS as u128))
    }
}
struct FrameClock {
    window: HWND,
    timer: HANDLE,
    armed: bool,
}
impl FrameClock {
    unsafe fn new(window: HWND) -> Self {
        Self {
            window,
            timer: CreateWaitableTimerExW(
                null(),
                null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS,
            ),
            armed: false,
        }
    }
    unsafe fn set_active(&mut self, active: bool) {
        if self.armed == active {
            return;
        }
        self.armed = active;
        if !active {
            if !self.timer.is_null() {
                CancelWaitableTimer(self.timer);
            }
            KillTimer(self.window, FRAME_TIMER);
        } else {
            if !self.timer.is_null()
                && SetWaitableTimerEx(self.timer, &-160_000, 16, None, null(), null(), 0) == 0
            {
                CloseHandle(self.timer);
                self.timer = null_mut();
            }
            if self.timer.is_null() {
                SetTimer(self.window, FRAME_TIMER, 16, None);
            }
        }
    }
}
impl Drop for FrameClock {
    fn drop(&mut self) {
        unsafe {
            KillTimer(self.window, FRAME_TIMER);
            if !self.timer.is_null() {
                CancelWaitableTimer(self.timer);
                CloseHandle(self.timer);
            }
        }
    }
}
unsafe fn animation_frame(hwnd: HWND) {
    if !(*STATE).visible {
        return;
    }
    if (*STATE)
        .hover_started
        .is_some_and(|s| s.elapsed().as_millis() >= render::HOVER_MS as u128)
    {
        (*STATE).hover_started = None;
    }
    if (*STATE)
        .animation_started
        .is_some_and(|s| s.elapsed().as_millis() >= render::ANIMATION_MS as u128)
    {
        (*STATE).animation_started = None;
    }
    paint(hwnd);
}
unsafe fn message_loop(hwnd: HWND) {
    let mut clock = FrameClock::new(hwnd);
    let mut msg: MSG = zeroed();
    loop {
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            if msg.message == WM_QUIT {
                return;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        clock.set_active((*STATE).animating());
        let count = u32::from(clock.armed && !clock.timer.is_null());
        let result = MsgWaitForMultipleObjectsEx(
            count,
            &clock.timer,
            INFINITE,
            QS_ALLINPUT,
            MWMO_INPUTAVAILABLE,
        );
        if result == WAIT_FAILED {
            return;
        }
        if count == 1 && result == WAIT_OBJECT_0 {
            animation_frame(hwnd);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn hit(x: f64, y: f64) -> i32 {
    if (x - 200.0).hypot(y - 200.0) <= 86.0 {
        return 6;
    }
    let dx = x - 200.0;
    let dy = y - 200.0;
    if !(91.0..=176.0).contains(&dx.hypot(dy)) {
        return -1;
    }
    let a = (dy.atan2(dx) + PI / 2.0 + PI / 6.0).rem_euclid(TAU);
    let within = a % (TAU / 6.0);
    let inset = (render::PETAL_GAP / (2.0 * dx.hypot(dy))).asin();
    if within < inset || within > TAU / 6.0 - inset {
        return -1;
    }
    (a / (TAU / 6.0)) as i32
}
unsafe fn current_hover() -> [f32; 7] {
    if let Some(start) = (*STATE).hover_started {
        render::hover_weights(
            (*STATE).hover_from,
            (*STATE).hover,
            start.elapsed().as_millis() as u64,
        )
    } else {
        std::array::from_fn(|i| if i as i32 == (*STATE).hover { 1.0 } else { 0.0 })
    }
}
unsafe fn update_hover(hwnd: HWND, index: i32) {
    if (*STATE).busy || (*STATE).success_started.is_some() || (*STATE).hover == index {
        return;
    }
    (*STATE).hover_from = current_hover();
    (*STATE).hover = index;
    (*STATE).hover_started = (*STATE).motion.then(Instant::now);
    InvalidateRect(hwnd, null(), 0);
}
fn hover_hit(x: f64, y: f64, weights: [f32; 7], motion: bool) -> i32 {
    // Keep the resting target too, so the petal cannot move away from its pointer.
    let resting = hit(x, y);
    if resting >= 0 {
        return resting;
    }
    for (i, strength) in weights.iter().enumerate().take(6) {
        let (dx, dy) = render::hover_offset(i, *strength, motion);
        if hit(x - dx as f64, y - dy as f64) == i as i32 {
            return i as i32;
        }
    }
    -1
}
unsafe fn interactive_hit(x: f64, y: f64) -> i32 {
    if (*STATE).busy || (*STATE).success_started.is_some() {
        return if (x - 200.0).hypot(y - 200.0) <= render::FEEDBACK_RADIUS {
            6
        } else {
            -1
        };
    }
    let index = hover_hit(x, y, current_hover(), (*STATE).motion);
    let elapsed = (*STATE)
        .animation_started
        .map(|start| start.elapsed().as_millis() as u64);
    if (0..6).contains(&index) && render::petal_progress(elapsed, index as usize) < 0.5 {
        -1
    } else {
        index
    }
}
unsafe fn source_is_foreground(root: HWND) -> bool {
    let foreground = GetForegroundWindow();
    foreground == root
        || (shell::is_desktop_root(root as isize) && shell::is_desktop_root(foreground as isize))
}
unsafe fn activation_target(root: HWND) -> HWND {
    if shell::is_desktop_root(root as isize) {
        GetShellWindow()
    } else {
        root
    }
}
unsafe fn explorer_at(p: POINT) -> Option<HWND> {
    let leaf = WindowFromPoint(p);
    let root = GetAncestor(leaf, GA_ROOT);
    if root.is_null() {
        return None;
    }
    let mut name = [0u16; 128];
    let len = GetClassNameW(root, name.as_mut_ptr(), 128);
    let class = String::from_utf16_lossy(&name[..len as usize]);
    if class == "CabinetWClass" {
        return Some(root);
    }
    if shell::desktop_host_class(&class) {
        // Only intercept the actual desktop file view, not wallpaper/widget hosts.
        let mut child = leaf;
        for _ in 0..8 {
            if child.is_null() {
                break;
            }
            let len = GetClassNameW(child, name.as_mut_ptr(), 128);
            if String::from_utf16_lossy(&name[..len as usize]) == "SHELLDLL_DefView" {
                return Some(root);
            }
            child = GetParent(child);
        }
    }
    None
}
fn mouse_button_down(message: u32) -> bool {
    matches!(
        message,
        WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
    )
}
unsafe fn outside_wheel(window: HWND, point: POINT) -> bool {
    if window.is_null() {
        return false;
    }
    if IsWindowVisible(window) == 0 {
        return (*STATE).resolving;
    }
    if GetAncestor(WindowFromPoint(point), GA_ROOT) != window {
        return true;
    }
    let mut local = point;
    ScreenToClient(window, &mut local);
    interactive_hit(
        local.x as f64 / (*STATE).scale,
        local.y as f64 / (*STATE).scale,
    ) < 0
}
unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let e = &*(lp as *const MSLLHOOKSTRUCT);
        if e.dwExtraInfo != 0x50455441 {
            let window = WINDOW.load(Ordering::Relaxed) as HWND;
            if mouse_button_down(wp as u32) && !STATE.is_null() && outside_wheel(window, e.pt) {
                PostMessageW(window, DISMISS, 0, 0);
            }
            if wp as u32 == WM_RBUTTONDOWN {
                let modifier = GetAsyncKeyState(VK_SHIFT as i32) < 0
                    || GetAsyncKeyState(VK_CONTROL as i32) < 0
                    || GetAsyncKeyState(VK_MENU as i32) < 0;
                DOWN = if modifier {
                    None
                } else {
                    explorer_at(e.pt).map(|h| (e.pt, h as usize))
                };
                if matches!(DOWN, Some(_)) {
                    return 1;
                }
            } else if wp as u32 == WM_MOUSEMOVE {
                if let Some((p, _)) = DOWN {
                    if (p.x - e.pt.x).abs() >= 5 || (p.y - e.pt.y).abs() >= 5 {
                        DOWN = None;
                        resume_drag(p, e.pt);
                    }
                }
            } else if wp as u32 == WM_RBUTTONUP {
                let down = DOWN;
                DOWN = None;
                if let Some((p, root)) = down {
                    if (p.x - e.pt.x).abs() < 5
                        && (p.y - e.pt.y).abs() < 5
                        && explorer_at(e.pt).is_some_and(|h| h as usize == root)
                    {
                        let packed = p.x as u32 as u64 | ((p.y as u32 as u64) << 32);
                        PostMessageW(window, HIT, root, packed as isize);
                        return 1;
                    }
                }
            }
        }
    }
    CallNextHookEx(null_mut(), code, wp, lp)
}
// Replay the beginning of a right-drag after the movement threshold is crossed.
unsafe fn resume_drag(start: POINT, current: POINT) {
    let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
    let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
    let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(2) - 1;
    let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(2) - 1;
    let mut inputs: [INPUT; 3] = zeroed();
    for input in &mut inputs {
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dwExtraInfo = 0x50455441;
    }
    for (index, p) in [(0, start), (2, current)] {
        inputs[index].Anonymous.mi.dx = ((p.x - left) as i64 * 65535 / width as i64) as i32;
        inputs[index].Anonymous.mi.dy = ((p.y - top) as i64 * 65535 / height as i64) as i32;
        inputs[index].Anonymous.mi.dwFlags =
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    }
    inputs[1].Anonymous.mi.dwFlags = MOUSEEVENTF_RIGHTDOWN;
    SendInput(3, inputs.as_ptr(), size_of::<INPUT>() as i32);
}
unsafe fn hide(hwnd: HWND) {
    (*STATE).hover_started = None;
    (*STATE).success_started = None;
    (*STATE).animation_started = None;
    (*STATE).visible = false;
    ShowWindow(hwnd, SW_HIDE);
}
unsafe fn replay(hwnd: HWND) {
    let p = (*STATE).point;
    let explorer = (*STATE).explorer;
    hide(hwnd);
    if explorer.is_null() || IsWindow(explorer) == 0 {
        return;
    }
    SetForegroundWindow(activation_target(explorer));
    SetCursorPos(p.x, p.y);
    let mut inputs: [INPUT; 2] = zeroed();
    for (input, flags) in inputs
        .iter_mut()
        .zip([MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP])
    {
        input.r#type = INPUT_MOUSE;
        input.Anonymous.mi.dwFlags = flags;
        input.Anonymous.mi.dwExtraInfo = 0x50455441;
    }
    SendInput(2, inputs.as_ptr(), size_of::<INPUT>() as i32);
}
unsafe fn show(hwnd: HWND) {
    (*STATE).scale = GetDpiForWindow((*STATE).explorer).max(96) as f64 / 96.0 * MENU_SCALE;
    let s = (*STATE).scale;
    let w = (WIDTH as f64 * s) as i32;
    let h = (HEIGHT as f64 * s) as i32;
    let monitor = MonitorFromPoint((*STATE).point, MONITOR_DEFAULTTONEAREST);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    let a = info.rcWork;
    let x = ((*STATE).point.x - w / 2)
        .max(a.left)
        .min((a.right - w).max(a.left));
    let y = ((*STATE).point.y - (200.0 * s) as i32)
        .max(a.top)
        .min((a.bottom - h).max(a.top));
    (*STATE).dark = render::dark_theme();
    (*STATE).accent = render::system_accent();
    (*STATE).motion = (*STATE).motion_enabled;
    // shortcut: the backdrop is a snapshot taken while hidden; video behind the menu freezes for its lifetime.
    (*STATE).glass = if (*STATE).glass_enabled {
        render::capture_glass(x, y, w, h, s, (*STATE).dark, render::menu_bend)
    } else {
        None
    };
    if let Some(source) = &(*STATE).source {
        (*STATE).formats = convert::menu_formats(source);
    }
    (*STATE).hover_from = [0.0; 7];
    (*STATE).hover_started = None;
    (*STATE).success_started = None;
    (*STATE).animation_started = (*STATE).motion.then(Instant::now);
    (*STATE).hover = -1;
    (*STATE).visible = true;
    SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_SHOWWINDOW);
    paint(hwnd);
    ShowWindow(hwnd, SW_SHOW);
    SetForegroundWindow(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn paint(hwnd: HWND) {
    let feedback = if (*STATE).busy {
        render::Feedback::Working {
            elapsed: (*STATE)
                .work_started
                .map_or(0, |s| s.elapsed().as_millis() as u64),
            animate: (*STATE).motion,
        }
    } else if (*STATE).success_started.is_some() {
        render::Feedback::Success {
            elapsed: (*STATE).success_started.unwrap().elapsed().as_millis() as u64,
        }
    } else {
        render::Feedback::Menu
    };
    let visual = render::Visual {
        accent: (*STATE).accent,
        hover: current_hover(),
        motion: (*STATE).motion,
        formats: (*STATE).formats,
        feedback,
        glass: (*STATE).glass.as_deref(),
    };
    render::paint(
        hwnd,
        (*STATE).scale,
        (*STATE).dark,
        (*STATE)
            .animation_started
            .map(|start| start.elapsed().as_millis() as u64),
        &visual,
    );
}
unsafe fn notification_data(hwnd: HWND, title: &str, message: &str) -> NOTIFYICONDATAW {
    let mut d: NOTIFYICONDATAW = zeroed();
    d.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    d.hWnd = hwnd;
    d.uID = 1;
    d.uFlags = NIF_INFO | NIF_ICON;
    let instance = GetModuleHandleW(null());
    d.hIcon = LoadImageW(
        instance,
        1usize as *const u16,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        LR_SHARED,
    ) as HICON;
    d.hBalloonIcon = LoadImageW(
        instance,
        1usize as *const u16,
        IMAGE_ICON,
        GetSystemMetrics(SM_CXICON),
        GetSystemMetrics(SM_CYICON),
        LR_SHARED,
    ) as HICON;
    d.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON | NIIF_NOSOUND;
    for (a, b) in d.szInfoTitle.iter_mut().take(63).zip(wide(title)) {
        *a = b;
    }
    for (a, b) in d.szInfo.iter_mut().take(255).zip(wide(message)) {
        *a = b;
    }
    d
}
unsafe fn notify(hwnd: HWND, title: &str, message: &str, error: bool) {
    let shown = !STATE.is_null()
        && (*STATE)
            .notifications
            .as_ref()
            .is_some_and(|n| n.show(title, message).is_ok());
    if !shown {
        let d = notification_data(hwnd, title, message);
        Shell_NotifyIconW(NIM_MODIFY, &d);
    }
    #[cfg(test)]
    if error {
        eprintln!("Petal: {message}");
    }
    #[cfg(not(test))]
    if error {
        MessageBoxW(
            hwnd,
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_OK
                | if !STATE.is_null() && !(*STATE).sound_enabled {
                    0
                } else {
                    MB_ICONERROR
                },
        );
    }
}
unsafe fn act(hwnd: HWND, index: i32) {
    if (*STATE).busy || (*STATE).success_started.is_some() {
        return;
    }
    if index == 6 {
        replay(hwnd);
        return;
    }
    if !(0..6).contains(&index) {
        return;
    }
    let Some(source) = (*STATE).source.clone() else {
        return;
    };
    (*STATE).animation_started = None;
    (*STATE).hover_started = None;
    (*STATE).hover_from = [0.0; 7];
    (*STATE).hover = -1;
    (*STATE).busy = true;
    let format = (*STATE).formats[index as usize];
    (*STATE).work_started = Some(Instant::now());
    (*STATE).success_started = None;
    (*STATE).pending_conversion = None;
    paint(hwnd);

    let sender = (*STATE).sender.clone();
    thread::spawn(move || {
        let _ = sender.send(Reply::Converted(convert::convert_file(&source, format)));
    });
    SetTimer(hwnd, 1, 30, None);
}
unsafe fn tray_menu(busy: bool) -> HMENU {
    let menu = CreatePopupMenu();
    AppendMenuW(
        menu,
        MF_STRING | MF_DISABLED,
        1,
        wide("Petal · File Explorer and desktop").as_ptr(),
    );
    AppendMenuW(menu, MF_STRING, 6, wide("Settings…").as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING | if busy { MF_DISABLED } else { 0 },
        2,
        wide("Quit Petal").as_ptr(),
    );
    menu
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if STATE.is_null() {
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
    match msg {
        HIT => {
            let point = POINT {
                x: lp as u32 as i32,
                y: ((lp as u64) >> 32) as u32 as i32,
            };
            if (*STATE).resolving || (*STATE).busy {
                let packed = (point.x as u16 as u32) | ((point.y as u16 as u32) << 16);
                PostMessageW(wp as HWND, WM_CONTEXTMENU, wp, packed as isize);
                return 0;
            }
            hide(hwnd);
            (*STATE).point = point;
            (*STATE).selection_dismissed = false;
            (*STATE).explorer = wp as HWND;
            SetForegroundWindow(activation_target((*STATE).explorer));
            (*STATE).source_foreground = GetForegroundWindow();
            (*STATE).resolving = true;
            let sender = (*STATE).sender.clone();
            thread::spawn(move || {
                let _ = sender.send(Reply::Selection(shell::file_at(
                    wp as isize,
                    point.x,
                    point.y,
                )));
            });
            SetTimer(hwnd, 1, 30, None);
            0
        }
        WM_TIMER if wp == FRAME_TIMER => {
            animation_frame(hwnd);
            0
        }
        WM_TIMER => {
            while let Ok(reply) = (*STATE).receiver.try_recv() {
                match reply {
                    Reply::Selection(source) => {
                        (*STATE).resolving = false;
                        if (*STATE).selection_dismissed {
                            continue;
                        }
                        if GetForegroundWindow() != (*STATE).source_foreground
                            && !source_is_foreground((*STATE).explorer)
                        {
                            continue;
                        }
                        (*STATE).source = source;
                        if (*STATE).source.is_some() {
                            show(hwnd);
                        } else {
                            replay(hwnd);
                        }
                    }
                    Reply::Converted(result) => {
                        (*STATE).pending_conversion = Some(result);
                    }
                }
            }
            if (*STATE).busy {
                let ready = (*STATE).pending_conversion.is_some()
                    && (*STATE)
                        .work_started
                        .is_some_and(|s| s.elapsed().as_millis() >= MIN_WORKING_MS);
                if ready {
                    let result = (*STATE).pending_conversion.take().unwrap();
                    (*STATE).busy = false;
                    (*STATE).work_started = None;
                    match result {
                        Ok(path) => {
                            let message = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            (*STATE).source = Some(path);
                            (*STATE).success_started = Some(Instant::now());
                            if (*STATE).visible {
                                paint(hwnd);
                                if (*STATE).sound_enabled {
                                    sound::success();
                                }
                            }
                            notify(hwnd, "Conversion complete", &message, false);
                        }
                        Err(error) => {
                            hide(hwnd);
                            notify(hwnd, "Conversion failed", &error, true);
                        }
                    }
                }
            }
            if (*STATE)
                .success_started
                .is_some_and(|s| s.elapsed().as_millis() >= SUCCESS_MS)
            {
                hide(hwnd);
            }
            if !(*STATE).busy && !(*STATE).resolving && (*STATE).success_started.is_none() {
                KillTimer(hwnd, 1);
            }
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => {
            let mut point = POINT {
                x: lp as u16 as i16 as i32,
                y: ((lp as u32) >> 16) as u16 as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            if interactive_hit(
                point.x as f64 / (*STATE).scale,
                point.y as f64 / (*STATE).scale,
            ) < 0
            {
                HTTRANSPARENT as isize
            } else {
                HTCLIENT as isize
            }
        }
        DISMISS => {
            (*STATE).selection_dismissed = true;
            hide(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
            let x = lp as u16 as i16 as f64 / (*STATE).scale;
            let y = ((lp as u32) >> 16) as u16 as i16 as f64 / (*STATE).scale;
            update_hover(hwnd, interactive_hit(x, y));
            0
        }
        WM_MOUSELEAVE => {
            update_hover(hwnd, -1);
            0
        }
        WM_DWMCOLORIZATIONCOLORCHANGED | WM_THEMECHANGED | WM_SETTINGCHANGE => {
            (*STATE).accent = render::system_accent();
            (*STATE).dark = render::dark_theme();
            if (*STATE).visible {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            let x = lp as u16 as i16 as f64 / (*STATE).scale;
            let y = ((lp as u32) >> 16) as u16 as i16 as f64 / (*STATE).scale;
            act(hwnd, interactive_hit(x, y));
            0
        }
        WM_KEYDOWN => {
            if (*STATE).busy || (*STATE).success_started.is_some() {
                if wp as u16 == VK_ESCAPE {
                    hide(hwnd);
                }
                return 0;
            }
            match wp as u16 {
                VK_ESCAPE => hide(hwnd),
                0x31..=0x36 => act(hwnd, wp as i32 - 0x31),
                0x4D => replay(hwnd),
                VK_TAB => {
                    (*STATE).animation_started = None;
                    update_hover(hwnd, ((*STATE).hover + 1) % 7);
                }
                VK_RETURN | VK_SPACE => act(hwnd, (*STATE).hover),
                _ => {}
            }
            0
        }
        WM_ACTIVATE if wp as u16 == WA_INACTIVE as u16 => {
            (*STATE).selection_dismissed = true;
            hide(hwnd);
            0
        }
        TRAY if lp as u32 == WM_LBUTTONUP => {
            panel::open(hwnd, (*STATE).accent, (*STATE).dark);
            0
        }
        TRAY if lp as u32 == WM_RBUTTONUP => {
            let menu = tray_menu((*STATE).busy);
            let mut p = zeroed();
            GetCursorPos(&mut p);
            SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                p.x,
                p.y,
                0,
                hwnd,
                null(),
            );
            DestroyMenu(menu);
            if cmd == 6 {
                panel::open(hwnd, (*STATE).accent, (*STATE).dark);
            }
            if cmd == 2 && !(*STATE).busy {
                PostQuitMessage(0);
            } else if cmd == 2 {
                notify(
                    hwnd,
                    "Petal",
                    "A conversion is in progress. Try again when it has finished.",
                    false,
                );
            }
            0
        }
        WM_COMMAND if (3..=5).contains(&wp) => {
            let (name, flag) = match wp {
                3 => (settings::SOUND, &mut (*STATE).sound_enabled),
                4 => (settings::GLASS, &mut (*STATE).glass_enabled),
                _ => (settings::MOTION, &mut (*STATE).motion_enabled),
            };
            let enabled = !*flag;
            match settings::save(name, enabled) {
                Ok(()) => {
                    *flag = enabled;
                    if wp == 3 && !enabled {
                        sound::stop();
                    }
                }
                Err(error) => notify(hwnd, "Settings could not be saved", &error, true),
            }
            0
        }
        WM_CLOSE => {
            hide(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
fn main() {
    unsafe {
        let args: Vec<_> = std::env::args_os().collect();
        if args.len() == 2 && args[1] == "--test-notification" {
            let result = notifications::Notifications::new()
                .and_then(|n| n.show("Conversion complete", "Petal notification test"));
            if let Err(error) = result {
                eprintln!("{error}");
                std::process::exit(1);
            }
            println!("Windows accepted the branded Petal notification.");
            return;
        }
        if args.len() == 4 && args[1] == "--convert" {
            let index = convert::FORMATS
                .iter()
                .position(|(_, _, ext)| args[3].to_string_lossy().eq_ignore_ascii_case(ext));
            let result = index
                .ok_or_else(|| "Unknown format".into())
                .and_then(|i| convert::convert_file(&PathBuf::from(&args[2]), i));
            if let Err(e) = result {
                MessageBoxW(
                    null_mut(),
                    wide(&e).as_ptr(),
                    wide("Petal").as_ptr(),
                    MB_OK | MB_ICONERROR,
                );
                std::process::exit(1);
            }
            return;
        }
        let mutex = CreateMutexW(null(), 0, wide("Local\\Petal.ImageConverter.0.1").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return;
        }
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let notifications = notifications::Notifications::new().ok();
        let instance = GetModuleHandleW(null());
        let class = wide("Petal.Radial");
        let mut wc: WNDCLASSEXW = zeroed();
        wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hIcon = LoadIconW(instance, 1usize as *const u16);
        wc.hIconSm = LoadImageW(
            instance,
            1usize as *const u16,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        ) as HICON;
        RegisterClassExW(&wc);
        let window = CreateWindowExW(
            (if args.len() == 3 && args[1] == "--preview" {
                WS_EX_APPWINDOW
            } else {
                WS_EX_TOOLWINDOW
            }) | WS_EX_TOPMOST
                | WS_EX_LAYERED,
            class.as_ptr(),
            wide("Petal — Convert a file").as_ptr(),
            WS_POPUP,
            0,
            0,
            WIDTH,
            HEIGHT,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if window.is_null() {
            return;
        }
        // Consume STARTUPINFO's initial hidden state before the first radial popup.
        ShowWindow(window, SW_HIDE);
        STATE = Box::into_raw(Box::new(State::new()));
        (*STATE).notifications = notifications;
        WINDOW.store(window as usize, Ordering::Relaxed);
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0);
        if hook.is_null() {
            MessageBoxW(
                window,
                wide("Could not enable right-click integration.").as_ptr(),
                wide("Petal").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
            DestroyWindow(window);
            return;
        }
        let mut tray: NOTIFYICONDATAW = zeroed();
        tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        tray.hWnd = window;
        tray.uID = 1;
        tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        tray.uCallbackMessage = TRAY;
        tray.hIcon = wc.hIconSm;
        for (a, b) in tray
            .szTip
            .iter_mut()
            .zip(wide("Petal — images, media, documents · Shift for Windows"))
        {
            *a = b;
        }
        Shell_NotifyIconW(NIM_ADD, &tray);
        notify(window,"Petal","Active in File Explorer and on the desktop. Right-click an image, media file or text document to convert it. Shift + right-click opens the Windows menu.",false);
        if args.len() == 3 && args[1] == "--preview" {
            (*STATE).source = Some(PathBuf::from(&args[2]));
            GetCursorPos(&mut (*STATE).point);
            show(window);
        }
        message_loop(window);
        UnhookWindowsHookEx(hook);
        Shell_NotifyIconW(NIM_DELETE, &tray);
        DestroyWindow(window);
        drop(Box::from_raw(STATE));
        STATE = null_mut();
        CloseHandle(mutex);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_menu_has_a_checked_mute_option_and_embedded_icons() {
        unsafe {
            let notification = notification_data(null_mut(), "Conversion complete", "photo.webp");
            assert_eq!(notification.dwInfoFlags & 0xf, NIIF_USER);
            assert_ne!(notification.dwInfoFlags & NIIF_LARGE_ICON, 0);
            assert_ne!(notification.dwInfoFlags & NIIF_NOSOUND, 0);
            assert_ne!(notification.uFlags & NIF_ICON, 0);
            assert!(!notification.hIcon.is_null());
            assert!(!notification.hBalloonIcon.is_null());
            let mut balloon: ICONINFO = zeroed();
            assert_ne!(GetIconInfo(notification.hBalloonIcon, &mut balloon), 0);
            let mut dimensions: BITMAP = zeroed();
            assert_ne!(
                GetObjectW(
                    balloon.hbmColor,
                    size_of::<BITMAP>() as i32,
                    &mut dimensions as *mut _ as *mut _
                ),
                0
            );
            assert_eq!(
                (dimensions.bmWidth, dimensions.bmHeight),
                (GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON))
            );
            DeleteObject(balloon.hbmColor);
            DeleteObject(balloon.hbmMask);
            for busy in [false, true] {
                let menu = tray_menu(busy);
                assert!(!menu.is_null());
                let mut label = [0u16; 64];
                GetMenuStringW(menu, 6, label.as_mut_ptr(), 64, MF_BYCOMMAND);
                assert!(String::from_utf16_lossy(&label).starts_with("Settings"));
                assert_eq!(
                    GetMenuState(menu, 2, MF_BYCOMMAND) & MF_DISABLED != 0,
                    busy
                );
                DestroyMenu(menu);
            }
            let instance = GetModuleHandleW(null());
            for size in [16, 24, 32, 48, 256] {
                let icon =
                    LoadImageW(instance, 1usize as *const u16, IMAGE_ICON, size, size, 0) as HICON;
                assert!(!icon.is_null(), "Embedded Bloom icon missing at {size}px");
                let mut info: ICONINFO = zeroed();
                assert_ne!(GetIconInfo(icon, &mut info), 0);
                let mut bitmap: BITMAP = zeroed();
                assert_ne!(
                    GetObjectW(
                        info.hbmColor,
                        size_of::<BITMAP>() as i32,
                        &mut bitmap as *mut _ as *mut _
                    ),
                    0
                );
                assert_eq!((bitmap.bmWidth, bitmap.bmHeight), (size, size));
                DeleteObject(info.hbmColor);
                DeleteObject(info.hbmMask);
                DestroyIcon(icon);
            }
        }
    }
    #[test]
    fn conversion_feedback_runs_through_real_message_loop() {
        use std::{
            fs,
            time::{Duration, SystemTime, UNIX_EPOCH},
        };
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-data")
            .join(format!(
                "feedback-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("test.png");
        image::RgbaImage::from_pixel(12, 8, image::Rgba([20, 130, 220, 255]))
            .save(&source)
            .unwrap();
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = wide("Petal.FeedbackTest");
            let mut wc: WNDCLASSW = zeroed();
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = instance;
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            let window = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                wide("Petal test").as_ptr(),
                WS_POPUP,
                0,
                0,
                300,
                300,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            assert!(!window.is_null());
            STATE = Box::into_raw(Box::new(State::new()));
            (*STATE).source = Some(source.clone());
            (*STATE).formats = convert::menu_formats(&source);
            (*STATE).scale = 0.75;
            (*STATE).motion = true;
            (*STATE).visible = true;
            struct Cleanup(HWND);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    unsafe {
                        KillTimer(self.0, 1);
                        KillTimer(self.0, FRAME_TIMER);
                        DestroyWindow(self.0);
                        drop(Box::from_raw(STATE));
                        STATE = null_mut();
                    }
                }
            }
            let cleanup = Cleanup(window);
            let mut clock = FrameClock::new(window);
            assert!(!clock.timer.is_null());
            assert_eq!(WaitForSingleObject(clock.timer, 0), WAIT_TIMEOUT);
            clock.set_active(true);
            for _ in 0..3 {
                assert_eq!(WaitForSingleObject(clock.timer, 250), WAIT_OBJECT_0);
                animation_frame(window);
            }
            clock.set_active(false);
            assert!(!clock.armed);
            drop(clock);
            assert!(!(*STATE).animating());
            let ui_thread = GetCurrentThreadId();
            update_hover(window, 0);
            let stop = thread::spawn(move || {
                thread::sleep(Duration::from_millis(350));
                assert_ne!(PostThreadMessageW(ui_thread, WM_QUIT, 0, 0), 0);
            });
            message_loop(window);
            stop.join().unwrap();
            assert!((*STATE).hover_started.is_none());
            assert_eq!((*STATE).hover, 0);
            assert!(!(*STATE).animating());
            act(window, 1);
            assert!((*STATE).busy);
            assert_eq!((*STATE).formats[1], 6);
            assert!((*STATE).animating());
            assert_eq!(interactive_hit(200.0, 60.0), -1);
            assert_eq!(interactive_hit(200.0, 200.0), 6);
            assert!((*STATE).success_started.is_none());
            wndproc(window, DISMISS, 0, 0);
            assert!(!(*STATE).visible);
            assert!((*STATE).busy);
            assert!(!(*STATE).animating());
            let start = (*STATE).work_started.unwrap();
            let mut msg: MSG = zeroed();
            while (*STATE).success_started.is_none() && start.elapsed() < Duration::from_secs(4) {
                while PeekMessageW(&mut msg, window, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!((*STATE).success_started.is_some());
            assert!(!(*STATE).busy);
            assert_eq!(sound::PLAYS.load(Ordering::Relaxed), 0);
            assert!(start.elapsed().as_millis() >= MIN_WORKING_MS);
            assert!(source.exists());
            assert!(source.with_extension("tiff").exists());
            assert_eq!(
                (*STATE).source.as_ref(),
                Some(&source.with_extension("tiff"))
            );
            (*STATE).success_started =
                Some(Instant::now() - Duration::from_millis(SUCCESS_MS as u64 + 1));
            wndproc(window, WM_TIMER, 1, 0);
            assert!((*STATE).success_started.is_none());
            assert!(!(*STATE).visible);
            (*STATE).source = Some(source.with_extension("tiff"));
            (*STATE).formats = convert::menu_formats((*STATE).source.as_ref().unwrap());
            (*STATE).visible = true;
            act(window, 0);
            let visible_start = Instant::now();
            while (*STATE).busy && visible_start.elapsed() < Duration::from_secs(4) {
                while PeekMessageW(&mut msg, window, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!(!(*STATE).busy);
            assert!((*STATE).success_started.is_some());
            assert!(source.with_extension("jpg").exists());
            assert!(source.with_extension("tiff").exists());
            assert_eq!(sound::PLAYS.load(Ordering::Relaxed), 1);
            paint(window);
            wndproc(window, WM_TIMER, 1, 0);
            assert_eq!(sound::PLAYS.load(Ordering::Relaxed), 1);
            hide(window);
            let audio = dir.join("music.wav");
            fs::write(&audio, include_bytes!("../assets/paid.wav")).unwrap();
            (*STATE).source = Some(audio.clone());
            (*STATE).formats = convert::menu_formats(&audio);
            assert_eq!(convert::FORMATS[(*STATE).formats[5]].0, "MP4");
            act(window, 5);
            let media_start = Instant::now();
            while (*STATE).busy && media_start.elapsed() < Duration::from_secs(15) {
                while PeekMessageW(&mut msg, window, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!(!(*STATE).busy);
            assert!(audio.with_extension("mp4").is_file());
            assert!(audio.exists());
            assert_eq!(sound::PLAYS.load(Ordering::Relaxed), 1);
            hide(window);
            let document = dir.join("sample \u{00e9}.txt");
            fs::write(&document, "Unicode \u{00e9} \u{00eb}!\nSecond paragraph.").unwrap();
            (*STATE).source = Some(document.clone());
            (*STATE).sound_enabled = false;
            (*STATE).visible = true;
            (*STATE).formats = convert::menu_formats(&document);
            assert_eq!(convert::FORMATS[(*STATE).formats[5]].0, "PDF");
            act(window, 5);
            let document_start = Instant::now();
            while (*STATE).busy && document_start.elapsed() < Duration::from_secs(5) {
                while PeekMessageW(&mut msg, window, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!(!(*STATE).busy);
            assert!(document.with_extension("pdf").is_file());
            assert!(document.exists());
            assert_eq!(sound::PLAYS.load(Ordering::Relaxed), 1);
            hide(window);
            (*STATE).source = None;
            (*STATE).resolving = true;
            assert!(outside_wheel(window, POINT { x: 0, y: 0 }));
            (*STATE)
                .sender
                .send(Reply::Selection(Some(source.with_extension("tiff"))))
                .unwrap();
            wndproc(window, DISMISS, 0, 0);
            wndproc(window, WM_TIMER, 1, 0);
            assert!(!(*STATE).resolving);
            assert!((*STATE).source.is_none());
            assert!(!(*STATE).visible);
            drop(cleanup);
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn moving_petals_keep_their_inner_target_and_extend_the_outer_target() {
        for i in 0..6 {
            let angle = (-90.0 + i as f64 * 60.0).to_radians();
            let mut weights = [0.0; 7];
            weights[i] = 1.0;
            for radius in [92.0, 134.0, 184.0] {
                let x = 200.0 + radius * angle.cos();
                let y = 200.0 + radius * angle.sin();
                assert_eq!(hover_hit(x, y, weights, true), i as i32);
                if radius > 176.0 {
                    assert_eq!(hover_hit(x, y, weights, false), -1);
                }
            }
            assert_eq!(hover_hit(200.0, 200.0, weights, true), 6);
        }
    }
    #[test]
    fn outside_clicks_cover_all_buttons_and_transparent_spaces() {
        for button in [
            WM_LBUTTONDOWN,
            WM_RBUTTONDOWN,
            WM_MBUTTONDOWN,
            WM_XBUTTONDOWN,
        ] {
            assert!(mouse_button_down(button));
        }
        for other in [WM_MOUSEMOVE, WM_LBUTTONUP, WM_RBUTTONUP, WM_MOUSEWHEEL] {
            assert!(!mouse_button_down(other));
        }
        assert_eq!(hit(0.0, 0.0), -1);
        assert_eq!(hit(200.0, 112.0), -1);
        assert_eq!(hit(330.0, 200.0), -1);
        assert_eq!(hit(200.0, 200.0), 6);
        assert_eq!(hit(200.0, 55.0), 0);
    }
    #[test]
    fn separators_have_constant_width_at_all_radii() {
        for seam in 0..6 {
            let angle = (-120.0 + seam as f64 * 60.0).to_radians();
            let (dx, dy) = (angle.cos(), angle.sin());
            for along in [100.0, 130.0, 160.0] {
                for offset in [-3.5, 0.0, 3.5] {
                    assert_eq!(
                        hit(
                            200.0 + along * dx - offset * dy,
                            200.0 + along * dy + offset * dx
                        ),
                        -1
                    );
                }
                assert_eq!(
                    hit(200.0 + along * dx - 4.5 * dy, 200.0 + along * dy + 4.5 * dx),
                    seam
                );
                assert_eq!(
                    hit(200.0 + along * dx + 4.5 * dy, 200.0 + along * dy - 4.5 * dx),
                    (seam + 5) % 6
                );
            }
        }
    }
    #[test]
    fn radial_targets_and_gaps() {
        for i in 0..6 {
            let a = -PI / 2.0 + i as f64 * TAU / 6.0;
            assert_eq!(hit(200.0 + 133.0 * a.cos(), 200.0 + 133.0 * a.sin()), i);
        }
        assert_eq!(hit(200.0, 200.0), 6);
        assert_eq!(hit(285.0, 200.0), 6);
        assert_eq!(hit(288.0, 200.0), -1);
        assert_eq!(hit(200.0, 421.0), -1);
        assert_eq!(hit(0.0, 0.0), -1);
    }
}

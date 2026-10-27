use crate::{wide, HEIGHT, WIDTH};
use std::{
    f32::consts::PI,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::DwmGetColorizationColor, Gdi::*, GdiPlus::*},
    System::Registry::*,
    UI::WindowsAndMessaging::*,
};

pub const HOVER_MS: u64 = 200;
pub const CHECK_MS: u64 = 240;
pub const FEEDBACK_RADIUS: f64 = 30.0;
const TEXT_FACE: &str = "Segoe UI";
#[derive(Clone, Copy)]
pub enum Feedback {
    Menu,
    Working { elapsed: u64, animate: bool },
    Success { elapsed: u64 },
}
pub struct Visual {
    pub accent: u32,
    pub hover: [f32; 7],
    pub motion: bool,
    pub formats: [usize; 6],
    pub feedback: Feedback,
}
impl Visual {
    #[cfg(test)]
    pub fn menu(index: i32, accent: u32) -> Self {
        Self {
            accent,
            motion: true,
            formats: [0, 1, 2, 3, 4, 5],
            hover: std::array::from_fn(|i| if i as i32 == index { 1.0 } else { 0.0 }),
            feedback: Feedback::Menu,
        }
    }
}
pub fn system_accent() -> u32 {
    unsafe {
        let mut color = 0;
        let mut opaque = 0;
        if DwmGetColorizationColor(&mut color, &mut opaque) >= 0 {
            color | 0xff000000
        } else {
            0xff0078d4
        }
    }
}
pub fn hover_weights(from: [f32; 7], target: i32, elapsed: u64) -> [f32; 7] {
    let t = (elapsed as f32 / HOVER_MS as f32).min(1.0);
    let t = 1.0 - (1.0 - t).powi(3);
    std::array::from_fn(|i| {
        let goal = if i as i32 == target { 1.0 } else { 0.0 };
        from[i] + (goal - from[i]) * t
    })
}
pub fn hover_offset(index: usize, strength: f32, motion: bool) -> (f32, f32) {
    let angle = (-90.0 + index as f32 * 60.0) * PI / 180.0;
    let distance = if motion { 10.0 * strength } else { 0.0 };
    (distance * angle.cos(), distance * angle.sin())
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let mut result = 0;
    for shift in [0, 8, 16, 24] {
        let a = ((a >> shift) & 255) as f32;
        let b = ((b >> shift) & 255) as f32;
        result |= ((a + (b - a) * t).round() as u32) << shift;
    }
    result
}
fn contrast(a: u32, b: u32) -> f32 {
    let luminance = |c: u32| {
        let channel = |shift: u32| {
            let v = ((c >> shift) & 255u32) as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    };
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn readable_accent(raw: u32, dark: bool, surface: u32, hover: u32) -> u32 {
    let towards = if dark { 0xffffffff } else { 0xff000000 };
    for step in 0..=20 {
        let c = mix(raw, towards, step as f32 / 20.0);
        if contrast(c, surface) >= 4.5 && contrast(c, hover) >= 4.5 {
            return c;
        }
    }
    towards
}
pub const ANIMATION_MS: u64 = 600;
pub const PETAL_GAP: f64 = 8.0;

pub fn petal_progress(elapsed: Option<u64>, index: usize) -> f32 {
    let Some(elapsed) = elapsed else {
        return 1.0;
    };
    let delay = index as u64 * 60;
    let t = (elapsed.saturating_sub(delay) as f32 / 300.0).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
fn faded(color: u32, progress: f32) -> u32 {
    let alpha = ((color >> 24) as f32 * progress).round() as u32;
    (color & 0x00ffffff) | (alpha << 24)
}
pub fn dark_theme() -> bool {
    unsafe {
        let mut value = 1u32;
        let mut bytes = 4;
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            &mut value as *mut _ as *mut _,
            &mut bytes,
        ) == 0
            && value == 0
    }
}
fn initialize() -> bool {
    static TOKEN: OnceLock<Option<usize>> = OnceLock::new();
    TOKEN
        .get_or_init(|| unsafe {
            let mut token = 0;
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                DebugEventCallback: 0,
                SuppressBackgroundThread: 0,
                SuppressExternalCodecs: 1,
            };
            (GdiplusStartup(&mut token, &input, null_mut()) == 0).then_some(token)
        })
        .is_some()
}
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    image: *mut GpBitmap,
    graphics: *mut GpGraphics,
    pixels: *mut u32,
    width: i32,
    height: i32,
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.graphics.is_null() {
                GdipDeleteGraphics(self.graphics);
            }
            if !self.image.is_null() {
                GdipDisposeImage(self.image as _);
            }
            if !self.dc.is_null() {
                SelectObject(self.dc, self.previous);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.dc.is_null() {
                DeleteDC(self.dc);
            }
        }
    }
}
struct Palette {
    surface: u32,
    center: u32,
    hover: u32,
    border: u32,
    text: u32,
    secondary: u32,
    accent: u32,
}
impl Palette {
    fn new(dark: bool, raw: u32) -> Self {
        let mut p = if dark {
            Self {
                surface: 0xff292929,
                center: 0xff242424,
                hover: 0,
                border: 0xff414141,
                text: 0xfff6f6f6,
                secondary: 0xffbcbcbc,
                accent: raw,
            }
        } else {
            Self {
                surface: 0xfffafafa,
                center: 0xfff3f3f3,
                hover: 0,
                border: 0xffdcdcdc,
                text: 0xff202020,
                secondary: 0xff626262,
                accent: raw,
            }
        };
        p.hover = mix(p.surface, raw, if dark { 0.24 } else { 0.14 });
        p.accent = readable_accent(raw, dark, p.surface, p.hover);
        p
    }
}
unsafe fn brush(color: u32) -> *mut GpSolidFill {
    let mut b = null_mut();
    GdipCreateSolidFill(color, &mut b);
    b
}
unsafe fn ellipse(g: *mut GpGraphics, x: f32, y: f32, w: f32, h: f32, color: u32) {
    let b = brush(color);
    GdipFillEllipse(g, b as _, x, y, w, h);
    GdipDeleteBrush(b as _);
}
unsafe fn fill(g: *mut GpGraphics, path: *mut GpPath, color: u32) {
    let b = brush(color);
    GdipFillPath(g, b as _, path);
    GdipDeleteBrush(b as _);
}
unsafe fn outline(g: *mut GpGraphics, path: *mut GpPath, color: u32) {
    let mut p = null_mut();
    GdipCreatePen1(color, 0.75, UnitPixel, &mut p);
    GdipDrawPath(g, p, path);
    GdipDeletePen(p);
}
fn polar(radius: f32, angle: f32) -> (f32, f32) {
    let a = angle * PI / 180.0;
    (200.0 + radius * a.cos(), 200.0 + radius * a.sin())
}
unsafe fn corner(path: *mut GpPath, from: (f32, f32), control: (f32, f32), to: (f32, f32)) {
    GdipAddPathBezier(
        path,
        from.0,
        from.1,
        from.0 + (control.0 - from.0) * 2.0 / 3.0,
        from.1 + (control.1 - from.1) * 2.0 / 3.0,
        to.0 + (control.0 - to.0) * 2.0 / 3.0,
        to.1 + (control.1 - to.1) * 2.0 / 3.0,
        to.0,
        to.1,
    );
}
unsafe fn sector_path(index: usize) -> *mut GpPath {
    // Parallel side edges sit half a gap away from each radial separator.
    let base_a = -120.0 + index as f32 * 60.0;
    let base_b = base_a + 60.0;
    let half_gap = PETAL_GAP as f32 / 2.0;
    let outer_inset = (half_gap / 176.0).asin().to_degrees();
    let inner_inset = (half_gap / 91.0).asin().to_degrees();
    let outer_a = base_a + outer_inset;
    let outer_b = base_b - outer_inset;
    let inner_a = base_a + inner_inset;
    let inner_b = base_b - inner_inset;
    let along_side = |point: (f32, f32), angle: f32, distance: f32| {
        let radians = angle * PI / 180.0;
        (
            point.0 + distance * radians.cos(),
            point.1 + distance * radians.sin(),
        )
    };
    let outer_start = polar(176.0, outer_a);
    let outer_end = polar(176.0, outer_b);
    let inner_start = polar(91.0, inner_a);
    let inner_end = polar(91.0, inner_b);
    let mut path = null_mut();
    GdipCreatePath(FillModeWinding, &mut path);
    GdipAddPathArc(
        path,
        24.0,
        24.0,
        352.0,
        352.0,
        outer_a + 2.0,
        outer_b - outer_a - 4.0,
    );
    let p = along_side(outer_end, base_b, -6.0);
    corner(path, polar(176.0, outer_b - 2.0), outer_end, p);
    let q = along_side(inner_end, base_b, 6.0);
    GdipAddPathLine(path, p.0, p.1, q.0, q.1);
    corner(path, q, inner_end, polar(91.0, inner_b - 4.0));
    GdipAddPathArc(
        path,
        109.0,
        109.0,
        182.0,
        182.0,
        inner_b - 4.0,
        inner_a - inner_b + 8.0,
    );
    let p = along_side(inner_start, base_a, 6.0);
    corner(path, polar(91.0, inner_a + 4.0), inner_start, p);
    let q = along_side(outer_start, base_a, -6.0);
    GdipAddPathLine(path, p.0, p.1, q.0, q.1);
    corner(path, q, outer_start, polar(176.0, outer_a + 2.0));
    GdipClosePathFigure(path);
    path
}
fn pixel_text_layout(points: [PointF; 2], width: f32, size: f32) -> (RectF, f32) {
    let scale = (points[1].X - points[0].X) / width;
    let x = points[0].X.round();
    let y = points[0].Y.round();
    (
        rect(
            x,
            y,
            (points[1].X.round() - x).max(1.0),
            (points[1].Y.round() - y).max(1.0),
        ),
        (size * scale).round().max(10.0),
    )
}
unsafe fn native_font(size: f32) -> HFONT {
    let mut font: LOGFONTW = zeroed();
    font.lfHeight = -(size as i32);
    font.lfWeight = FW_BOLD as i32;
    font.lfQuality = CLEARTYPE_QUALITY;
    for (to, from) in font.lfFaceName.iter_mut().zip(wide(TEXT_FACE)) {
        *to = from;
    }
    CreateFontIndirectW(&font)
}
unsafe fn native_text(
    surface: &Surface,
    value: &str,
    layout: RectF,
    size: f32,
    color: u32,
) -> bool {
    let font = native_font(size);
    if font.is_null() {
        return false;
    }
    GdipFlush(surface.graphics, FlushIntentionSync);
    let mut bounds = RECT {
        left: layout.X as i32,
        top: layout.Y as i32,
        right: (layout.X + layout.Width) as i32,
        bottom: (layout.Y + layout.Height) as i32,
    };
    let left = bounds.left.clamp(0, surface.width);
    let right = bounds.right.clamp(left, surface.width);
    let top = bounds.top.clamp(0, surface.height);
    let bottom = bounds.bottom.clamp(top, surface.height);
    let mut original = Vec::with_capacity(((right - left) * (bottom - top)) as usize);
    for y in top..bottom {
        for x in left..right {
            original.push(*surface.pixels.add((y * surface.width + x) as usize));
        }
    }
    let saved = SaveDC(surface.dc);
    let previous = SelectObject(surface.dc, font);
    SetBkMode(surface.dc, TRANSPARENT as i32);
    SetTextColor(
        surface.dc,
        ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255),
    );
    let value = wide(value);
    let drawn = DrawTextW(
        surface.dc,
        value.as_ptr(),
        (value.len() - 1) as i32,
        &mut bounds,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
    GdiFlush();
    SelectObject(surface.dc, previous);
    if saved != 0 {
        RestoreDC(surface.dc, saved);
    }
    DeleteObject(font);
    let mut index = 0;
    for y in top..bottom {
        for x in left..right {
            let pixel = surface.pixels.add((y * surface.width + x) as usize);
            let old = original[index];
            // ClearType blends against opaque RGB; GDI does not maintain its alpha.
            *pixel = if old >> 24 == 255 {
                *pixel | 0xff000000
            } else {
                old
            };
            index += 1;
        }
    }
    drawn != 0
}
unsafe fn text(surface: &Surface, value: &str, layout: RectF, size: f32, color: u32) {
    let g = surface.graphics;
    let mut points = [
        PointF {
            X: layout.X,
            Y: layout.Y,
        },
        PointF {
            X: layout.X + layout.Width,
            Y: layout.Y + layout.Height,
        },
    ];
    GdipTransformPoints(
        g,
        CoordinateSpaceDevice,
        CoordinateSpaceWorld,
        points.as_mut_ptr(),
        2,
    );
    let (layout, size) = pixel_text_layout(points, layout.Width, size);
    if color >> 24 == 255 && native_text(surface, value, layout, size, color) {
        return;
    }
    let mut saved = 0;
    GdipSaveGraphics(g, &mut saved);
    // Rasterize at the final pixel size, without a fractional font transform.
    GdipResetWorldTransform(g);
    GdipSetPixelOffsetMode(g, PixelOffsetModeNone);
    let mut family = null_mut();
    GdipCreateFontFamilyFromName(wide(TEXT_FACE).as_ptr(), null_mut(), &mut family);
    let mut font = null_mut();
    let mut format = null_mut();
    GdipCreateFont(family, size, FontStyleBold, UnitPixel, &mut font);
    GdipCreateStringFormat(StringFormatFlagsNoWrap, 0, &mut format);
    GdipSetStringFormatAlign(format, StringAlignmentCenter);
    GdipSetStringFormatLineAlign(format, StringAlignmentCenter);
    GdipSetStringFormatTrimming(format, StringTrimmingEllipsisCharacter);
    let b = brush(color);
    let value = wide(value);
    GdipDrawString(
        g,
        value.as_ptr(),
        (value.len() - 1) as i32,
        font,
        &layout,
        format,
        b as _,
    );
    GdipDeleteBrush(b as _);
    GdipDeleteStringFormat(format);
    GdipDeleteFont(font);
    GdipDeleteFontFamily(family);
    GdipRestoreGraphics(g, saved);
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> RectF {
    RectF {
        X: x,
        Y: y,
        Width: w,
        Height: h,
    }
}
unsafe fn draw(
    surface: &Surface,
    filename: &str,
    dark: bool,
    elapsed: Option<u64>,
    visual: &Visual,
) {
    let g = surface.graphics;
    let p = Palette::new(dark, visual.accent);
    if !matches!(visual.feedback, Feedback::Menu) {
        let radius = FEEDBACK_RADIUS as f32;
        let mut path = null_mut();
        GdipCreatePath(FillModeWinding, &mut path);
        GdipAddPathEllipse(
            path,
            200.0 - radius,
            200.0 - radius,
            radius * 2.0,
            radius * 2.0,
        );
        fill(g, path, p.center);
        outline(g, path, p.border);
        GdipDeletePath(path);
    }
    match visual.feedback {
        Feedback::Working { elapsed, animate } => {
            watch_spinner(g, 200.0, 200.0, if animate { elapsed } else { 0 }, p.accent);
            return;
        }
        Feedback::Success { elapsed } => {
            draw_check(g, elapsed, visual.motion, p.accent);
            return;
        }
        Feedback::Menu => {}
    }
    // A translated, graduated shadow; surfaces stay opaque and text remains legible.
    for spread in (1..=18).rev() {
        let r = 176.0 + spread as f32;
        ellipse(
            g,
            200.0 - r,
            206.0 - r,
            r * 2.0,
            r * 2.0,
            faded(
                if dark { 0x03000000 } else { 0x02000000 },
                elapsed.map_or(1.0, |t| (t as f32 / ANIMATION_MS as f32).min(1.0)),
            ),
        );
    }
    let labels = visual.formats.map(|index| crate::convert::FORMATS[index].0);
    for (i, label) in labels.iter().enumerate() {
        let progress = petal_progress(elapsed, i);
        if progress == 0.0 {
            continue;
        }
        let angle = (-90.0 + i as f32 * 60.0) * PI / 180.0;
        let offset = -10.0 * (1.0 - progress);
        let strength = visual.hover[i];
        let (hover_x, hover_y) = hover_offset(i, strength, visual.motion);
        let mut saved = 0;
        GdipSaveGraphics(g, &mut saved);
        GdipTranslateWorldTransform(
            g,
            offset * angle.cos() + hover_x,
            offset * angle.sin() + hover_y,
            MatrixOrderPrepend,
        );
        let path = sector_path(i);
        fill(
            g,
            path,
            faded(mix(p.surface, visual.accent, strength), progress),
        );
        outline(
            g,
            path,
            faded(mix(p.border, visual.accent, strength), progress),
        );
        GdipDeletePath(path);
        let (x, y) = polar(134.0, -90.0 + i as f32 * 60.0);
        text(
            surface,
            label,
            rect(x - 42.0, y - 16.0, 84.0, 32.0),
            21.0,
            faded(mix(p.text, 0xff000000, strength), progress),
        );
        GdipRestoreGraphics(g, saved);
    }
    for spread in (1..=6).rev() {
        let r = 86.0 + spread as f32;
        ellipse(g, 200.0 - r, 203.0 - r, r * 2.0, r * 2.0, 0x04000000);
    }
    ellipse(
        g,
        114.0,
        114.0,
        172.0,
        172.0,
        mix(p.center, visual.accent, visual.hover[6]),
    );
    let mut pen = null_mut();
    GdipCreatePen1(
        mix(p.border, visual.accent, visual.hover[6]),
        0.8,
        UnitPixel,
        &mut pen,
    );
    GdipDrawEllipse(g, pen, 114.0, 114.0, 172.0, 172.0);
    GdipDeletePen(pen);
    let b = brush(mix(p.accent, 0xff000000, visual.hover[6]));
    for (x, y) in [
        (189.0, 148.0),
        (201.0, 148.0),
        (189.0, 160.0),
        (201.0, 160.0),
    ] {
        GdipFillRectangle(g, b as _, x, y, 10.0, 10.0);
    }
    GdipDeleteBrush(b as _);
    text(
        surface,
        "Windows menu",
        rect(125.0, 182.0, 150.0, 25.0),
        17.5,
        mix(p.text, 0xff000000, visual.hover[6]),
    );
    text(
        surface,
        filename,
        rect(128.0, 212.0, 144.0, 20.0),
        12.0,
        mix(p.secondary, 0xff000000, visual.hover[6]),
    );
}
unsafe fn draw_check(g: *mut GpGraphics, elapsed: u64, motion: bool, color: u32) {
    let t = if motion {
        (elapsed.max(1) as f32 / CHECK_MS as f32).min(1.0)
    } else {
        1.0
    };
    let progress = 1.0 - (1.0 - t).powi(3);
    let first = (progress / 0.34).min(1.0);
    let second = ((progress - 0.34) / 0.66).max(0.0);
    let mut pen = null_mut();
    GdipCreatePen1(color, 4.0, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);
    let mut path = null_mut();
    GdipCreatePath(FillModeWinding, &mut path);
    GdipAddPathLine(
        path,
        185.0,
        200.0,
        185.0 + 11.0 * first,
        200.0 + 11.0 * first,
    );
    if second > 0.0 {
        GdipAddPathLine(
            path,
            196.0,
            211.0,
            196.0 + 20.0 * second,
            211.0 - 22.0 * second,
        );
    }
    GdipDrawPath(g, pen, path);
    GdipDeletePath(path);
    GdipDeletePen(pen);
}
// Native reinterpretation of Amicro's Watch Spinner: three nested quarter-circle arcs.
unsafe fn watch_spinner(g: *mut GpGraphics, x: f32, y: f32, elapsed: u64, color: u32) {
    let mut pen = null_mut();
    GdipCreatePen1(color, 3.0, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    for (diameter, period, phase) in [
        (44.0, 1200.0, 0.0),
        (32.0, 900.0, 120.0),
        (20.0, 600.0, 240.0),
    ] {
        let angle = (elapsed as f32 % period) / period * 360.0 + phase - 90.0;
        GdipDrawArc(
            g,
            pen,
            x - diameter / 2.0,
            y - diameter / 2.0,
            diameter,
            diameter,
            angle,
            88.0,
        );
    }
    GdipDeletePen(pen);
}
#[cfg(test)]
unsafe fn surface(
    scale: f64,
    hover: i32,
    name: &str,
    dark: bool,
    elapsed: Option<u64>,
) -> Result<Surface, String> {
    let mut visual = Visual::menu(hover, 0xff0078d4);
    visual.formats = crate::convert::menu_formats(std::path::Path::new(name));
    surface_visual(scale, name, dark, elapsed, &visual)
}
unsafe fn surface_visual(
    scale: f64,
    name: &str,
    dark: bool,
    elapsed: Option<u64>,
    visual: &Visual,
) -> Result<Surface, String> {
    if !initialize() {
        return Err("Could not initialize GDI+".into());
    }
    let width = (WIDTH as f64 * scale).round() as i32;
    let height = (HEIGHT as f64 * scale).round() as i32;
    let mut s = Surface {
        dc: CreateCompatibleDC(null_mut()),
        bitmap: null_mut(),
        previous: null_mut(),
        image: null_mut(),
        graphics: null_mut(),
        pixels: null_mut(),
        width,
        height,
    };
    let mut info: BITMAPINFO = zeroed();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut pixels = null_mut();
    s.bitmap = CreateDIBSection(s.dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
    if s.dc.is_null() || s.bitmap.is_null() || pixels.is_null() {
        return Err("Graphics surface unavailable".into());
    }
    s.pixels = pixels.cast();
    s.previous = SelectObject(s.dc, s.bitmap);
    // PixelFormat32bppPARGB: 32-bit premultiplied alpha, matching UpdateLayeredWindow.
    let format =
        11 | (32 << 8) | PixelFormatAlpha as i32 | PixelFormatPAlpha as i32 | PixelFormatGDI as i32;
    if GdipCreateBitmapFromScan0(width, height, width * 4, format, pixels as _, &mut s.image) != 0 {
        return Err("Graphics bitmap unavailable".into());
    }
    if GdipGetImageGraphicsContext(s.image as _, &mut s.graphics) != 0 {
        return Err("Graphics context unavailable".into());
    }
    GdipGraphicsClear(s.graphics, 0);
    GdipSetSmoothingMode(s.graphics, SmoothingModeAntiAlias8x8);
    GdipSetPixelOffsetMode(s.graphics, PixelOffsetModeHalf);
    GdipSetTextRenderingHint(s.graphics, TextRenderingHintAntiAliasGridFit);
    GdipScaleWorldTransform(s.graphics, scale as f32, scale as f32, MatrixOrderPrepend);
    draw(&s, name, dark, elapsed, visual);
    GdipFlush(s.graphics, FlushIntentionSync);
    Result::Ok(s)
}
pub unsafe fn paint(
    hwnd: HWND,
    scale: f64,
    name: &str,
    dark: bool,
    elapsed: Option<u64>,
    visual: &Visual,
) {
    let mut ps = zeroed();
    BeginPaint(hwnd, &mut ps);
    if let Result::Ok(s) = surface_visual(scale, name, dark, elapsed, visual) {
        let source = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: s.width,
            cy: s.height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            hwnd,
            null_mut(),
            null(),
            &size,
            s.dc,
            &source,
            0,
            &blend,
            ULW_ALPHA,
        );
    }
    EndPaint(hwnd, &ps);
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn native_segoe_bold_uses_whole_pixels_and_preserves_alpha() {
        unsafe {
            assert!(initialize());
            let s = surface(0.75, -1, "photo sample 123 \u{00e9}.jpg", true, None).unwrap();
            let font = native_font(16.0);
            assert!(!font.is_null());
            let previous = SelectObject(s.dc, font);
            let mut face = [0u16; 32];
            assert!(GetTextFaceW(s.dc, 32, face.as_mut_ptr()) > 0);
            assert!(String::from_utf16_lossy(&face).starts_with(TEXT_FACE));
            let mut selected: LOGFONTW = zeroed();
            assert!(
                GetObjectW(
                    font,
                    size_of::<LOGFONTW>() as i32,
                    &mut selected as *mut _ as *mut _
                ) > 0
            );
            assert_eq!(selected.lfWeight, FW_BOLD as i32);
            SelectObject(s.dc, previous);
            DeleteObject(font);
            let before: Vec<_> =
                std::slice::from_raw_parts(s.pixels, (s.width * s.height) as usize).to_vec();
            // Deliberately cross transparent space, an edge and the opaque center.
            assert!(native_text(
                &s,
                "Clear text 123 \u{00e9}",
                rect(0.0, 150.0, 300.0, 30.0),
                16.0,
                0xffffffff
            ));
            let after = std::slice::from_raw_parts(s.pixels, before.len());
            assert!(before.iter().zip(after).any(|(a, b)| a != b));
            for (&a, &b) in before.iter().zip(after) {
                assert_eq!(
                    a >> 24,
                    b >> 24,
                    "GDI must preserve the layered window alpha"
                );
                if a >> 24 != 255 {
                    assert_eq!(a, b);
                }
            }
        }
        for scale in [0.75, 1.125, 1.5] {
            for size in [10.5, 12.0, 16.5, 19.0] {
                let (layout, pixels) = pixel_text_layout(
                    [
                        PointF {
                            X: 115.35 * scale,
                            Y: 126.12 * scale,
                        },
                        PointF {
                            X: 215.35 * scale,
                            Y: 156.12 * scale,
                        },
                    ],
                    100.0,
                    size,
                );
                assert_eq!(pixels.fract(), 0.0);
                assert!(pixels >= 10.0);
                for value in [layout.X, layout.Y, layout.Width, layout.Height] {
                    assert_eq!(value.fract(), 0.0);
                }
            }
        }
    }
    unsafe fn save(s: &Surface, path: &Path) {
        let (mut count, mut bytes) = (0, 0);
        assert_eq!(GdipGetImageEncodersSize(&mut count, &mut bytes), 0);
        let mut storage =
            vec![0usize; (bytes as usize + size_of::<usize>() - 1) / size_of::<usize>()];
        let codecs = storage.as_mut_ptr() as *mut ImageCodecInfo;
        assert_eq!(GdipGetImageEncoders(count, bytes, codecs), 0);
        let codecs = std::slice::from_raw_parts(codecs, count as usize);
        let codec = codecs
            .iter()
            .find(|c| {
                let mut n = 0;
                while *c.MimeType.add(n) != 0 {
                    n += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(c.MimeType, n)) == "image/png"
            })
            .unwrap();
        assert_eq!(
            GdipSaveImageToFile(
                s.image as _,
                wide(&path.to_string_lossy()).as_ptr(),
                &codec.Clsid,
                null()
            ),
            0
        );
    }
    #[test]
    fn accent_palette_stays_readable_and_hover_can_be_interrupted() {
        for dark in [false, true] {
            for accent in [
                0xff0078d4, 0xffe67e22, 0xff00b050, 0xffffee00, 0xffb300cc, 0xff000000, 0xffffffff,
            ] {
                let p = Palette::new(dark, accent);
                assert!(contrast(p.accent, p.surface) >= 4.5);
                assert!(contrast(p.accent, p.hover) >= 4.5);
                assert_ne!(p.hover, p.surface);
            }
        }
        let initial = [0.0; 7];
        let half = hover_weights(initial, 0, 70);
        assert!(half[0] > 0.0 && half[0] < 1.0);
        assert_eq!(hover_weights(half, 1, 0), half);
        let end = hover_weights(half, 1, HOVER_MS);
        assert_eq!(end[0], 0.0);
        assert_eq!(end[1], 1.0);
        assert_eq!(hover_weights(end, -1, HOVER_MS), [0.0; 7]);
    }
    #[test]
    fn accent_hover_spinner_and_success_render() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        for (name, dark, accent, hover, feedback) in [
            ("accent-blue-hover", true, 0xff0078d4, 1, Feedback::Menu),
            ("accent-orange-hover", true, 0xffe67e22, 1, Feedback::Menu),
            ("accent-green-hover", false, 0xff00b050, 2, Feedback::Menu),
            (
                "spinner-0",
                true,
                0xffe67e22,
                -1,
                Feedback::Working {
                    elapsed: 0,
                    animate: true,
                },
            ),
            (
                "spinner-300",
                true,
                0xffe67e22,
                -1,
                Feedback::Working {
                    elapsed: 300,
                    animate: true,
                },
            ),
            (
                "conversion-success",
                true,
                0xffe67e22,
                -1,
                Feedback::Success { elapsed: CHECK_MS },
            ),
            (
                "success-light",
                false,
                0xff00b050,
                -1,
                Feedback::Success { elapsed: CHECK_MS },
            ),
        ] {
            unsafe {
                let mut visual = Visual::menu(hover, accent);
                visual.formats = crate::convert::menu_formats(std::path::Path::new("photo.jpg"));
                visual.feedback = feedback;
                let s = surface_visual(0.75, "photo.jpg", dark, None, &visual).unwrap();
                save(&s, &out.join(format!("{name}.png")));
                if matches!(feedback, Feedback::Menu) {
                    let image = image::open(out.join(format!("{name}.png")))
                        .unwrap()
                        .to_rgba8();
                    let (x, y) = polar(134.0, -90.0 + hover as f32 * 60.0);
                    let (dx, dy) = hover_offset(hover as usize, 1.0, true);
                    let expected =
                        image::Rgba([(accent >> 16) as u8, (accent >> 8) as u8, accent as u8, 255]);
                    // The former shortcut badge is now uninterrupted solid accent.
                    for px in -5..=5 {
                        for py in 18..=25 {
                            assert_eq!(
                                *image.get_pixel(
                                    ((x + dx + px as f32) * 0.75).round() as u32,
                                    ((y + dy + py as f32) * 0.75).round() as u32
                                ),
                                expected,
                                "{name}"
                            );
                        }
                    }
                    assert!(
                        image.enumerate_pixels().any(|(px, py, p)| {
                            (px as f32 - (x + dx) * 0.75).abs() < 24.0
                                && (py as f32 - (y + dy) * 0.75).abs() < 10.0
                                && p[3] == 255
                                && p[0] < 10
                                && p[1] < 10
                                && p[2] < 10
                        }),
                        "Hovered label must be black: {name}"
                    );
                }
            }
        }
        assert_ne!(
            image::open(out.join("spinner-0.png"))
                .unwrap()
                .into_rgba8()
                .as_raw(),
            image::open(out.join("spinner-300.png"))
                .unwrap()
                .into_rgba8()
                .as_raw()
        );
        assert_ne!(
            image::open(out.join("spinner-0.png"))
                .unwrap()
                .into_rgba8()
                .as_raw(),
            image::open(out.join("conversion-success.png"))
                .unwrap()
                .into_rgba8()
                .as_raw()
        );
    }
    #[test]
    fn feedback_renders_a_small_circle_and_animates_the_check() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        for dark in [false, true] {
            for scale in [0.75, 1.5] {
                let mut frames = Vec::new();
                let mut visual = Visual::menu(-1, 0xff0078d4);
                for (name, feedback) in [
                    (
                        "working",
                        Feedback::Working {
                            elapsed: 160,
                            animate: true,
                        },
                    ),
                    ("check-start", Feedback::Success { elapsed: 0 }),
                    ("check-middle", Feedback::Success { elapsed: 40 }),
                    ("check-done", Feedback::Success { elapsed: CHECK_MS }),
                ] {
                    visual.feedback = feedback;
                    let path = out.join(format!("center-{name}-{dark}-{scale}.png"));
                    unsafe {
                        let s = surface_visual(scale, "photo.tiff", dark, None, &visual).unwrap();
                        save(&s, &path);
                    }
                    let pixels = image::open(path).unwrap().to_rgba8();
                    for (x, y, pixel) in pixels.enumerate_pixels() {
                        if (x as f64 / scale - 200.0).hypot(y as f64 / scale - 200.0)
                            > FEEDBACK_RADIUS + 2.0 / scale
                        {
                            assert_eq!(
                                pixel[3], 0,
                                "The compact circle must not leave a larger background"
                            );
                        }
                    }
                    assert_eq!(
                        pixels.get_pixel((200.0 * scale) as u32, (225.0 * scale) as u32)[3],
                        255
                    );
                    assert!(pixels.pixels().any(|p| p[3] > 0));
                    frames.push(pixels);
                }
                assert_ne!(frames[1], frames[2]);
                assert_ne!(frames[2], frames[3]);
                visual.motion = false;
                visual.feedback = Feedback::Success { elapsed: 0 };
                let path = out.join(format!("center-check-reduced-{dark}-{scale}.png"));
                unsafe {
                    save(
                        &surface_visual(scale, "photo.tiff", dark, None, &visual).unwrap(),
                        &path,
                    );
                }
                assert_eq!(image::open(path).unwrap().to_rgba8(), frames[3]);
            }
        }
    }
    #[test]
    fn reveal_is_ordered_and_finishes() {
        assert_eq!(petal_progress(Some(0), 0), 0.0);
        for index in 0..6 {
            assert_eq!(petal_progress(Some(index as u64 * 60), index), 0.0);
            assert!(petal_progress(Some(index as u64 * 60 + 20), index) > 0.0);
            assert_eq!(petal_progress(Some(ANIMATION_MS), index), 1.0);
            assert_eq!(petal_progress(None, index), 1.0);
        }
        for elapsed in 0..=ANIMATION_MS {
            for index in 0..5 {
                assert!(
                    petal_progress(Some(elapsed), index)
                        >= petal_progress(Some(elapsed), index + 1)
                );
            }
        }
    }
    #[test]
    #[ignore = "Exports native frames for the README; run explicitly when updating documentation"]
    fn export_documentation_frames() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/documentation-frames");
        std::fs::create_dir_all(&out).unwrap();
        for frame in 0..91 {
            let mut visual = Visual::menu(-1, 0xff0078d4);
            visual.formats = crate::convert::menu_formats(std::path::Path::new("sample.jpg"));
            let elapsed = if frame < 16 { Some(frame * 40) } else { None };
            match frame {
                24..=29 => visual.hover = hover_weights([0.0; 7], 1, (frame - 24) * 40),
                30..=39 => visual.hover[1] = 1.0,
                40..=63 => {
                    visual.feedback = Feedback::Working {
                        elapsed: (frame - 40) * 40,
                        animate: true,
                    }
                }
                64..=90 => {
                    visual.feedback = Feedback::Success {
                        elapsed: ((frame - 64) * 40).min(CHECK_MS),
                    }
                }
                _ => (),
            }
            unsafe {
                let surface = surface_visual(1.0, "sample.jpg", true, elapsed, &visual).unwrap();
                save(&surface, &out.join(format!("frame-{frame:03}.png")));
            }
        }
    }
    #[test]
    fn animation_frames_keep_center_and_reveal_in_order() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        for elapsed in [0, 150, 300, 600] {
            unsafe {
                let s = surface(0.75, -1, "holiday photo.jpg", true, Some(elapsed)).unwrap();
                let path = out.join(format!("animation-{elapsed}.png"));
                save(&s, &path);
                let image = image::open(path).unwrap().to_rgba8();
                assert_eq!(image.dimensions(), (300, 300));
                assert_eq!(image.get_pixel(150, 150)[3], 255);
                if elapsed == 0 {
                    assert_eq!(image.get_pixel(150, 41)[3], 0);
                }
                if elapsed == 600 {
                    for index in 0..6 {
                        let (x, y) = polar(145.0, -90.0 + index as f32 * 60.0);
                        assert_eq!(
                            image.get_pixel((x * 0.75) as u32, (y * 0.75) as u32)[3],
                            255
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn transparent_native_render_at_multiple_dpi() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        for (dark, name, hover, scale) in [
            (false, "light", -1, 1.0),
            (true, "compact", -1, 0.75),
            (true, "dark", -1, 1.0),
            (true, "dark-hover", 0, 1.0),
            (true, "dark-center", 6, 1.0),
            (true, "compact-center", 6, 0.75),
            (false, "light-150", 6, 1.5),
            (true, "dark-200", 2, 2.0),
            (true, "audio-menu", -1, 0.75),
            (false, "video-menu", -1, 0.75),
            (true, "document-menu", -1, 0.75),
            (false, "pdf-menu", -1, 0.75),
        ] {
            unsafe {
                let file = match name {
                    "audio-menu" => "music.mp3",
                    "video-menu" => "clip.mp4",
                    "document-menu" => "sample \u{00e9}.txt",
                    "pdf-menu" => "document.pdf",
                    _ => "holiday photo.jpg",
                };
                let s = surface(scale, hover, file, dark, None).unwrap();
                let path = out.join(format!("{name}.png"));
                save(&s, &path);
                let image = image::open(&path).unwrap().to_rgba8();
                if hover == 6 {
                    assert_eq!(
                        *image.get_pixel((200.0 * scale) as u32, (250.0 * scale) as u32),
                        image::Rgba([0, 120, 212, 255])
                    );
                    assert_eq!(
                        *image.get_pixel((194.0 * scale) as u32, (154.0 * scale) as u32),
                        image::Rgba([0, 0, 0, 255])
                    );
                    for (top, bottom) in [(182.0, 207.0), (212.0, 232.0)] {
                        assert!(
                            image
                                .enumerate_pixels()
                                .any(|(x, y, p)| x as f64 > 128.0 * scale
                                    && (x as f64) < 272.0 * scale
                                    && y as f64 > top * scale
                                    && (y as f64) < bottom * scale
                                    && p[3] == 255
                                    && p[0] < 10
                                    && p[1] < 10
                                    && p[2] < 10),
                            "Black central text missing in {name}"
                        );
                    }
                }
                assert_eq!(
                    image.dimensions(),
                    ((400.0 * scale) as u32, (400.0 * scale) as u32)
                );
                if hover == -1 {
                    for i in 0..6 {
                        let (x, y) = polar(134.0, -90.0 + i as f32 * 60.0);
                        let has_text = image.enumerate_pixels().any(|(px, py, p)| {
                            let (px, py) = (px as f64 / scale, py as f64 / scale);
                            px > x as f64 - 35.0
                                && px < x as f64 + 35.0
                                && py > y as f64 - 24.0
                                && py < y as f64 + 3.0
                                && p[3] == 255
                                && if dark {
                                    *p.0[..3].iter().max().unwrap() > 150
                                } else {
                                    *p.0[..3].iter().min().unwrap() < 140
                                }
                        });
                        assert!(has_text, "Missing format text in {name}, petal {i}");
                    }
                }
                assert_eq!(image.get_pixel(0, 0)[3], 0);
                assert_eq!(
                    image.get_pixel((200.0 * scale) as u32, (200.0 * scale) as u32)[3],
                    255
                );
                assert_eq!(
                    image.get_pixel((200.0 * scale) as u32, (55.0 * scale) as u32)[3],
                    255
                );
            }
        }
    }
}

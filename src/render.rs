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
pub struct Visual<'a> {
    pub accent: u32,
    pub hover: [f32; 7],
    pub motion: bool,
    pub formats: [usize; 6],
    pub feedback: Feedback,
    /// Device-pixel liquid glass from `glass_from`; `None` draws the classic opaque theme.
    pub glass: Option<&'a [u32]>,
}
impl Visual<'static> {
    #[cfg(test)]
    pub fn menu(index: i32, accent: u32) -> Self {
        Self {
            accent,
            motion: true,
            formats: [0, 1, 2, 3, 4, 5],
            hover: std::array::from_fn(|i| if i as i32 == index { 1.0 } else { 0.0 }),
            feedback: Feedback::Menu,
            glass: None,
        }
    }
}
/// Maps a logical pixel of a glass shape to the backdrop point seen through it.
pub type Bend = fn(f32, f32) -> (f32, f32);
/// Copies the screen behind a (still hidden) window and bakes it into liquid glass.
pub unsafe fn capture_glass(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    scale: f64,
    dark: bool,
    bend: Bend,
) -> Option<Vec<u32>> {
    let screen = GetDC(null_mut());
    let dc = CreateCompatibleDC(screen);
    let mut info: BITMAPINFO = zeroed();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = w;
    info.bmiHeader.biHeight = -h;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut bits = null_mut();
    let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    let mut raw = None;
    if !dc.is_null() && !bitmap.is_null() && !bits.is_null() {
        let previous = SelectObject(dc, bitmap);
        if BitBlt(dc, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT) != 0 {
            raw = Some(std::slice::from_raw_parts(bits as *const u32, (w * h) as usize).to_vec());
        }
        SelectObject(dc, previous);
    }
    if !bitmap.is_null() {
        DeleteObject(bitmap);
    }
    DeleteDC(dc);
    ReleaseDC(null_mut(), screen);
    raw.map(|raw| glass_from(&raw, w as usize, h as usize, scale, dark, bend))
}
/// The radial menu's lens: content bends near every petal edge and the center rim.
pub fn menu_bend(x: f32, y: f32) -> (f32, f32) {
    let (dx, dy) = (x - 200.0, y - 200.0);
    let r = dx.hypot(dy);
    let mut a = dy.atan2(dx);
    let bent = if r <= 88.0 {
        r - 88.0 * 0.14 * (r / 88.0).powi(4)
    } else if r <= 180.0 {
        let u = ((r - 133.5) / 43.5).clamp(-1.0, 1.0);
        // Petal sides: bend along the angle as well, towards the petal's middle.
        let rel = (a.to_degrees() + 120.0).rem_euclid(60.0) - 30.0;
        a -= (30.0 * 0.15 * (rel / 30.0).powi(3)).to_radians();
        r - 43.5 * 0.25 * u.powi(3)
    } else {
        r
    };
    (200.0 + bent * a.cos(), 200.0 + bent * a.sin())
}
/// The settings panel's lens: a slab whose last 18px curve the backdrop inward.
pub fn panel_bend(x: f32, y: f32) -> (f32, f32) {
    let edge = |v: f32, size: f32| {
        let rim = 18.0;
        let near = (rim - v).max(0.0) - (rim - (size - v)).max(0.0);
        v + near * near.abs() / rim * 0.6
    };
    (edge(x, PANEL_W as f32), edge(y, PANEL_H as f32))
}
/// Frosts (blur + saturation + theme wash) and refracts a backdrop through a shape.
/// The blur runs on a 4× smaller copy and is sampled back bilinearly: identical look for
/// a blurred surface, about 16× less work, which lets live glass resample every frame.
pub fn glass_from(raw: &[u32], w: usize, h: usize, scale: f64, dark: bool, bend: Bend) -> Vec<u32> {
    const F: usize = 4;
    let (lw, lh) = (w.div_ceil(F).max(1), h.div_ceil(F).max(1));
    let mut small = vec![0u32; lw * lh];
    for ly in 0..lh {
        for lx in 0..lw {
            let (mut sum, mut n) = ([0u32; 3], 0);
            for y in ly * F..((ly + 1) * F).min(h) {
                for x in lx * F..((lx + 1) * F).min(w) {
                    let c = raw[y * w + x];
                    for (k, shift) in [16, 8, 0].into_iter().enumerate() {
                        sum[k] += (c >> shift) & 255;
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            small[ly * lw + lx] = 0xff000000 | (sum[0] / n) << 16 | (sum[1] / n) << 8 | sum[2] / n;
        }
    }
    let mut buffer = vec![0u32; small.len()];
    let radius = (6.0 * scale / F as f64).round().max(1.0) as usize;
    for _ in 0..3 {
        box_blur(&small, &mut buffer, lw, lh, radius, true);
        box_blur(&buffer, &mut small, lw, lh, radius, false);
    }
    let (wash, toward) = if dark { (0.58, 28.0) } else { (0.55, 248.0) };
    let scale = scale as f32;
    let mut out = vec![0u32; raw.len()];
    for py in 0..h {
        for px in 0..w {
            let (bx, by) = bend(px as f32 / scale, py as f32 / scale);
            // Low-resolution coordinates of the bent sample, then a bilinear read.
            let fx = (bx * scale / F as f32 - 0.5).clamp(0.0, lw as f32 - 1.0);
            let fy = (by * scale / F as f32 - 0.5).clamp(0.0, lh as f32 - 1.0);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(lw - 1), (y0 + 1).min(lh - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let channel = |shift: u32| {
                let at = |x: usize, y: usize| ((small[y * lw + x] >> shift) & 255) as f32;
                let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * tx;
                let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * tx;
                top + (bottom - top) * ty
            };
            let (r8, g8, b8) = (channel(16), channel(8), channel(0));
            let l = 0.3 * r8 + 0.59 * g8 + 0.11 * b8;
            let finish = |v: f32| {
                let saturated = (l + (v - l) * 1.3).clamp(0.0, 255.0);
                (saturated * (1.0 - wash) + toward * wash).round() as u32
            };
            out[py * w + px] = 0xff000000 | finish(r8) << 16 | finish(g8) << 8 | finish(b8);
        }
    }
    out
}
fn box_blur(src: &[u32], dst: &mut [u32], w: usize, h: usize, radius: usize, horizontal: bool) {
    let (lines, length) = if horizontal { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| {
        if horizontal {
            line * w + i
        } else {
            i * w + line
        }
    };
    let window = (2 * radius + 1) as u32;
    for line in 0..lines {
        let mut sum = [0u32; 3];
        let sample = |i: isize| src[at(line, i.clamp(0, length as isize - 1) as usize)];
        for i in -(radius as isize)..=radius as isize {
            let c = sample(i);
            for (k, shift) in [16, 8, 0].into_iter().enumerate() {
                sum[k] += (c >> shift) & 255;
            }
        }
        for i in 0..length {
            dst[at(line, i)] =
                0xff000000 | (sum[0] / window) << 16 | (sum[1] / window) << 8 | sum[2] / window;
            let (out, inn) = (
                sample(i as isize - radius as isize),
                sample(i as isize + radius as isize + 1),
            );
            for (k, shift) in [16, 8, 0].into_iter().enumerate() {
                sum[k] = sum[k] + ((inn >> shift) & 255) - ((out >> shift) & 255);
            }
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
pub const ANIMATION_MS: u64 = 460;
pub const PETAL_GAP: f64 = 8.0;
const STAGGER_MS: u64 = 28;
const BUBBLE_MS: f32 = 300.0;

/// Linear 0..1 reveal of one petal; `bubble` shapes it.
pub fn petal_progress(elapsed: Option<u64>, index: usize) -> f32 {
    let Some(elapsed) = elapsed else {
        return 1.0;
    };
    let delay = index as u64 * STAGGER_MS;
    (elapsed.saturating_sub(delay) as f32 / BUBBLE_MS).clamp(0.0, 1.0)
}
/// Ease-out with a ~6% overshoot: the soft pop of a bubble settling.
fn bubble(t: f32) -> f32 {
    let (c1, c3) = (1.3, 2.3);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}
/// Scales whatever is drawn next around the menu's center.
unsafe fn grow_from_center(g: *mut GpGraphics, s: f32) {
    GdipTranslateWorldTransform(g, 200.0, 200.0, MatrixOrderPrepend);
    GdipScaleWorldTransform(g, s, s, MatrixOrderPrepend);
    GdipTranslateWorldTransform(g, -200.0, -200.0, MatrixOrderPrepend);
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
                accent: raw,
            }
        } else {
            Self {
                surface: 0xfffafafa,
                center: 0xfff3f3f3,
                hover: 0,
                border: 0xffdcdcdc,
                text: 0xff202020,
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
unsafe fn native_font(size: f32, bold: bool) -> HFONT {
    let mut font: LOGFONTW = zeroed();
    font.lfHeight = -(size as i32);
    font.lfWeight = if bold { FW_BOLD } else { FW_NORMAL } as i32;
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
    style: Style,
) -> bool {
    let font = native_font(size, style.bold);
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
        if style.left { DT_LEFT } else { DT_CENTER }
            | DT_VCENTER
            | DT_SINGLELINE
            | DT_END_ELLIPSIS
            | DT_NOPREFIX,
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
    styled(
        surface,
        value,
        layout,
        size,
        color,
        Style {
            left: false,
            bold: true,
        },
    );
}
#[derive(Clone, Copy)]
struct Style {
    left: bool,
    bold: bool,
}
unsafe fn styled(
    surface: &Surface,
    value: &str,
    layout: RectF,
    size: f32,
    color: u32,
    style: Style,
) {
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
    if color >> 24 == 255 && native_text(surface, value, layout, size, color, style) {
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
    GdipCreateFont(
        family,
        size,
        if style.bold {
            FontStyleBold
        } else {
            FontStyleRegular
        },
        UnitPixel,
        &mut font,
    );
    GdipCreateStringFormat(StringFormatFlagsNoWrap, 0, &mut format);
    GdipSetStringFormatAlign(
        format,
        if style.left {
            StringAlignmentNear
        } else {
            StringAlignmentCenter
        },
    );
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
/// The liquid glass as a GDI+ texture brush in menu (world) coordinates.
struct Glass {
    image: *mut GpBitmap,
    brush: *mut GpTexture,
    dark: bool,
}
impl Glass {
    unsafe fn new(pixels: &[u32], surface: &Surface, scale: f32, dark: bool) -> Option<Self> {
        if pixels.len() != (surface.width * surface.height) as usize {
            return None;
        }
        let mut glass = Self {
            image: null_mut(),
            brush: null_mut(),
            dark,
        };
        // PixelFormat32bppARGB; the glass is fully opaque, so straight == premultiplied.
        let format = 10
            | (32 << 8)
            | PixelFormatAlpha as i32
            | PixelFormatGDI as i32
            | PixelFormatCanonical as i32;
        GdipCreateBitmapFromScan0(
            surface.width,
            surface.height,
            surface.width * 4,
            format,
            pixels.as_ptr() as *mut u8,
            &mut glass.image,
        );
        if glass.image.is_null()
            || GdipCreateTexture(glass.image as _, WrapModeClamp, &mut glass.brush) != 0
        {
            return None;
        }
        GdipScaleTextureTransform(glass.brush, 1.0 / scale, 1.0 / scale, MatrixOrderPrepend);
        Some(glass)
    }
    /// Glass body, a white sheen that lights it up on hover (`glow` 0..1), then the polished rim.
    /// The glass stays colourless: hover brightens it instead of painting a hue over the backdrop.
    unsafe fn pane(&self, g: *mut GpGraphics, path: *mut GpPath, glow: f32) {
        GdipFillPath(g, self.brush as _, path);
        let mut bounds = rect(0.0, 0.0, 0.0, 0.0);
        GdipGetPathWorldBounds(path, &mut bounds, null(), null());
        let (top, bottom) = (
            PointF {
                X: 0.0,
                Y: bounds.Y,
            },
            PointF {
                X: 0.0,
                Y: bounds.Y + bounds.Height,
            },
        );
        let line = |from: u32, to: u32| {
            let mut brush = null_mut();
            GdipCreateLineBrush(&top, &bottom, from, to, WrapModeTileFlipXY, &mut brush);
            brush
        };
        if glow > 0.0 {
            let (high, low) = if self.dark {
                (0.24, 0.08)
            } else {
                (0.62, 0.30)
            };
            let sheen = line(
                with_alpha(0xffffffff, high * glow),
                with_alpha(0xffffffff, low * glow),
            );
            GdipFillPath(g, sheen as _, path);
            GdipDeleteBrush(sheen as _);
        }
        let light = line(
            with_alpha(0xffffffff, 0.65 + 0.35 * glow),
            with_alpha(0xffffffff, 0.12 + 0.3 * glow),
        );
        let mut pen = null_mut();
        GdipCreatePen2(light as _, 1.2 + 0.6 * glow, UnitPixel, &mut pen);
        GdipDrawPath(g, pen, path);
        GdipDeletePen(pen);
        GdipDeleteBrush(light as _);
    }
}
impl Drop for Glass {
    fn drop(&mut self) {
        unsafe {
            if !self.brush.is_null() {
                GdipDeleteBrush(self.brush as _);
            }
            if !self.image.is_null() {
                GdipDisposeImage(self.image as _);
            }
        }
    }
}
fn with_alpha(color: u32, alpha: f32) -> u32 {
    (color & 0x00ffffff) | (((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24)
}
unsafe fn circle_path(radius: f32) -> *mut GpPath {
    let mut path = null_mut();
    GdipCreatePath(FillModeWinding, &mut path);
    GdipAddPathEllipse(
        path,
        200.0 - radius,
        200.0 - radius,
        radius * 2.0,
        radius * 2.0,
    );
    path
}
unsafe fn draw(surface: &Surface, scale: f32, dark: bool, elapsed: Option<u64>, visual: &Visual) {
    let g = surface.graphics;
    let p = Palette::new(dark, visual.accent);
    let glass = visual
        .glass
        .and_then(|pixels| Glass::new(pixels, surface, scale, dark));
    if !matches!(visual.feedback, Feedback::Menu) {
        let path = circle_path(FEEDBACK_RADIUS as f32);
        if let Some(glass) = &glass {
            glass.pane(g, path, 0.0);
        } else {
            fill(g, path, p.center);
            outline(g, path, p.border);
        }
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
    // A translated, graduated shadow that grows with the bubble burst.
    let mut saved = 0;
    GdipSaveGraphics(g, &mut saved);
    grow_from_center(g, 0.4 + 0.6 * bubble(petal_progress(elapsed, 3)));
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
    GdipRestoreGraphics(g, saved);
    let labels = visual.formats.map(|index| crate::convert::FORMATS[index].0);
    for (i, label) in labels.iter().enumerate() {
        let progress = petal_progress(elapsed, i);
        if progress == 0.0 {
            continue;
        }
        // Petals bubble out from under the center disc, overshoot slightly and settle.
        let size = 0.4 + 0.6 * bubble(progress);
        let fade = (progress * 2.5).min(1.0);
        let strength = visual.hover[i];
        let (hover_x, hover_y) = hover_offset(i, strength, visual.motion);
        let mut saved = 0;
        GdipSaveGraphics(g, &mut saved);
        GdipTranslateWorldTransform(g, hover_x, hover_y, MatrixOrderPrepend);
        grow_from_center(g, size);
        let path = sector_path(i);
        if let Some(glass) = &glass {
            glass.pane(g, path, strength);
        } else {
            fill(
                g,
                path,
                faded(mix(p.surface, visual.accent, strength), fade),
            );
            outline(g, path, faded(mix(p.border, visual.accent, strength), fade));
        }
        GdipDeletePath(path);
        let (x, y) = polar(134.0, -90.0 + i as f32 * 60.0);
        text(
            surface,
            label,
            rect(x - 42.0, y - 16.0, 84.0, 32.0),
            21.0,
            if glass.is_some() {
                p.text
            } else {
                faded(mix(p.text, 0xff000000, strength), fade)
            },
        );
        GdipRestoreGraphics(g, saved);
    }
    for spread in (1..=6).rev() {
        let r = 86.0 + spread as f32;
        ellipse(g, 200.0 - r, 203.0 - r, r * 2.0, r * 2.0, 0x04000000);
    }
    // The center pops first and leads the bubble burst.
    let pop = elapsed.map_or(1.0, |t| bubble((t as f32 / 220.0).min(1.0)));
    let mut saved = 0;
    GdipSaveGraphics(g, &mut saved);
    grow_from_center(g, 0.85 + 0.15 * pop);
    let path = circle_path(86.0);
    if let Some(glass) = &glass {
        glass.pane(g, path, visual.hover[6]);
    } else {
        fill(g, path, mix(p.center, visual.accent, visual.hover[6]));
        outline(g, path, mix(p.border, visual.accent, visual.hover[6]));
    }
    GdipDeletePath(path);
    // Only the Windows logo: the center opens the original context menu.
    let logo = if glass.is_some() { p.text } else { p.accent };
    let b = brush(if glass.is_some() {
        logo
    } else {
        mix(logo, 0xff000000, visual.hover[6])
    });
    for (x, y) in [
        (183.0, 183.0),
        (201.0, 183.0),
        (183.0, 201.0),
        (201.0, 201.0),
    ] {
        GdipFillRectangle(g, b as _, x, y, 16.0, 16.0);
    }
    GdipDeleteBrush(b as _);
    GdipRestoreGraphics(g, saved);
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
    surface_visual(scale, dark, elapsed, &visual)
}
unsafe fn surface_visual(
    scale: f64,
    dark: bool,
    elapsed: Option<u64>,
    visual: &Visual,
) -> Result<Surface, String> {
    let s = new_surface(WIDTH, HEIGHT, scale)?;
    draw(&s, scale as f32, dark, elapsed, visual);
    GdipFlush(s.graphics, FlushIntentionSync);
    Result::Ok(s)
}
/// A cleared premultiplied ARGB surface of `width`×`height` logical pixels at `scale`.
unsafe fn new_surface(width: i32, height: i32, scale: f64) -> Result<Surface, String> {
    if !initialize() {
        return Err("Could not initialize GDI+".into());
    }
    let width = (width as f64 * scale).round() as i32;
    let height = (height as f64 * scale).round() as i32;
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
    Result::Ok(s)
}
/// Shows a finished surface on a layered window, faded by `alpha` (0..=255).
unsafe fn present(hwnd: HWND, s: &Surface, alpha: u8) {
    let source = POINT { x: 0, y: 0 };
    let size = SIZE {
        cx: s.width,
        cy: s.height,
    };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
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
pub const PANEL_W: i32 = 340;
pub const PANEL_H: i32 = 300;
pub const PANEL_ROWS: [(&str, &str); 3] = [
    ("Liquid glass", "Frosted, refracting menu"),
    ("Animations", "Bubble opening and hover motion"),
    ("Sound", "A chime when a conversion finishes"),
];
const ROW_TOP: f32 = 76.0;
const ROW_STEP: f32 = 64.0;
/// What the settings panel shows this frame.
pub struct Panel<'a> {
    pub accent: u32,
    pub glass: Option<&'a [u32]>,
    /// Knob position of each switch, 0 = off .. 1 = on (animated by the caller).
    pub switches: [f32; 3],
    /// Hovered or keyboard-focused target: 0..=2 rows, 3 = close, 4 = all settings, -1 none.
    pub hover: i32,
    /// Open animation 0..1 (eased here) and fade-out alpha.
    pub open: f32,
    pub alpha: u8,
}
/// Settings panel target under a logical point: 0..=2 rows, 3 = close, 4 = all settings, -1 none.
pub fn panel_hit(x: f32, y: f32) -> i32 {
    if (x - PANEL_W as f32 / 2.0).abs() <= 70.0
        && (PANEL_H as f32 - 42.0..=PANEL_H as f32 - 14.0).contains(&y)
    {
        return 4;
    }
    if (x - (PANEL_W as f32 - 34.0)).hypot(y - 30.0) <= 15.0 {
        return 3;
    }
    if !(16.0..=PANEL_W as f32 - 16.0).contains(&x) {
        return -1;
    }
    (0..3)
        .find(|&i| (0.0..56.0).contains(&(y - ROW_TOP - i as f32 * ROW_STEP)))
        .map_or(-1, |i| i as i32)
}
unsafe fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> *mut GpPath {
    let mut path = null_mut();
    GdipCreatePath(FillModeWinding, &mut path);
    let d = r * 2.0;
    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    GdipClosePathFigure(path);
    path
}
pub unsafe fn paint_panel(hwnd: HWND, scale: f64, dark: bool, panel: &Panel) {
    let mut ps = zeroed();
    BeginPaint(hwnd, &mut ps);
    if let Result::Ok(s) = new_surface(PANEL_W, PANEL_H, scale) {
        draw_panel(&s, scale as f32, dark, panel);
        GdipFlush(s.graphics, FlushIntentionSync);
        present(hwnd, &s, panel.alpha);
    }
    EndPaint(hwnd, &ps);
}
unsafe fn draw_panel(s: &Surface, scale: f32, dark: bool, panel: &Panel) {
    let g = s.graphics;
    let p = Palette::new(dark, panel.accent);
    let (w, h) = (PANEL_W as f32, PANEL_H as f32);
    let glass = panel
        .glass
        .and_then(|pixels| Glass::new(pixels, s, scale, dark));
    // The panel pops like the menu's bubble: a short overshoot around its center.
    let size = 0.92 + 0.08 * bubble(panel.open);
    GdipTranslateWorldTransform(g, w / 2.0, h / 2.0, MatrixOrderPrepend);
    GdipScaleWorldTransform(g, size, size, MatrixOrderPrepend);
    GdipTranslateWorldTransform(g, -w / 2.0, -h / 2.0, MatrixOrderPrepend);
    let body = rounded(1.0, 1.0, w - 2.0, h - 2.0, 20.0);
    match &glass {
        Some(glass) => glass.pane(g, body, 0.0),
        None => {
            fill(g, body, p.surface);
            outline(g, body, p.border);
        }
    }
    GdipDeletePath(body);
    let left = Style {
        left: true,
        bold: true,
    };
    let quiet = Style {
        left: true,
        bold: false,
    };
    let secondary = mix(p.text, if dark { 0xff000000 } else { 0xffffffff }, 0.35);
    // Glass is colourless (the hue comes from behind it): controls use monochrome ink, classic keeps the accent.
    let (ink, knob_on) = match (glass.is_some(), dark) {
        (false, _) => (p.accent, 0xffffffff),
        (true, true) => (0xebffffff, 0xff1c1c1c),
        (true, false) => (p.text, 0xffffffff),
    };
    // Header: the brand's four-petal mark, title and subtitle.
    for (i, (dx, dy)) in [(0.0, 0.0), (12.0, 0.0), (0.0, 12.0), (12.0, 12.0)]
        .into_iter()
        .enumerate()
    {
        let path = rounded(
            24.0 + dx,
            20.0 + dy,
            10.0,
            10.0,
            if i == 0 { 5.0 } else { 3.0 },
        );
        fill(
            g,
            path,
            if i == 0 {
                ink
            } else {
                with_alpha(p.text, 0.85)
            },
        );
        GdipDeletePath(path);
    }
    styled(
        s,
        "Petal",
        rect(56.0, 14.0, 180.0, 24.0),
        18.0,
        p.text,
        left,
    );
    styled(
        s,
        "Settings",
        rect(56.0, 36.0, 180.0, 18.0),
        12.5,
        secondary,
        quiet,
    );
    // Close button.
    let (cx, cy) = (w - 34.0, 30.0);
    if panel.hover == 3 {
        ellipse(
            g,
            cx - 14.0,
            cy - 14.0,
            28.0,
            28.0,
            with_alpha(p.text, 0.12),
        );
    }
    let mut pen = null_mut();
    GdipCreatePen1(p.text, 1.6, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipDrawLine(g, pen, cx - 5.0, cy - 5.0, cx + 5.0, cy + 5.0);
    GdipDrawLine(g, pen, cx + 5.0, cy - 5.0, cx - 5.0, cy + 5.0);
    GdipDeletePen(pen);
    for (i, (title, detail)) in PANEL_ROWS.iter().enumerate() {
        let y = ROW_TOP + i as f32 * ROW_STEP;
        if panel.hover == i as i32 {
            let row = rounded(16.0, y, w - 32.0, 56.0, 14.0);
            fill(
                g,
                row,
                if glass.is_some() {
                    with_alpha(0xffffffff, if dark { 0.10 } else { 0.38 })
                } else {
                    with_alpha(p.text, 0.08)
                },
            );
            GdipDeletePath(row);
        }
        styled(
            s,
            title,
            rect(32.0, y + 8.0, 210.0, 22.0),
            15.0,
            p.text,
            left,
        );
        styled(
            s,
            detail,
            rect(32.0, y + 30.0, 220.0, 18.0),
            12.0,
            secondary,
            quiet,
        );
        // Switch: the track fills with ink, the knob glides across.
        let on = panel.switches[i];
        let (tx, ty, tw, th) = (w - 32.0 - 46.0, y + 16.0, 46.0, 24.0);
        let track = rounded(tx, ty, tw, th, th / 2.0);
        fill(g, track, mix(with_alpha(p.text, 0.18), ink, on));
        GdipDeletePath(track);
        let knob = 16.0 + 2.0 * (on * PI).sin();
        let kx = tx + 4.0 + on * (tw - 8.0 - 16.0) + (16.0 - knob) / 2.0;
        ellipse(
            g,
            kx,
            ty + (th - knob) / 2.0,
            knob,
            knob,
            mix(0xfff2f2f2, knob_on, on),
        );
    }
    // Link to the full settings window (shortcut, startup, custom sound).
    let link = rounded(w / 2.0 - 70.0, h - 42.0, 140.0, 28.0, 14.0);
    if panel.hover == 4 {
        fill(
            g,
            link,
            if glass.is_some() {
                with_alpha(0xffffffff, if dark { 0.14 } else { 0.45 })
            } else {
                with_alpha(p.text, 0.08)
            },
        );
    }
    GdipDeletePath(link);
    let footer = Style {
        left: false,
        bold: true,
    };
    styled(
        s,
        "All settings…",
        rect(0.0, h - 37.0, w, 18.0),
        12.5,
        if panel.hover == 4 { p.text } else { secondary },
        footer,
    );
}
pub unsafe fn paint(hwnd: HWND, scale: f64, dark: bool, elapsed: Option<u64>, visual: &Visual) {
    let mut ps = zeroed();
    BeginPaint(hwnd, &mut ps);
    if let Result::Ok(s) = surface_visual(scale, dark, elapsed, visual) {
        present(hwnd, &s, 255);
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
            let font = native_font(16.0, true);
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
                0xffffffff,
                Style {
                    left: false,
                    bold: true
                }
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
                let s = surface_visual(0.75, dark, None, &visual).unwrap();
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
                        let s = surface_visual(scale, dark, None, &visual).unwrap();
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
                    save(&surface_visual(scale, dark, None, &visual).unwrap(), &path);
                }
                assert_eq!(image::open(path).unwrap().to_rgba8(), frames[3]);
            }
        }
    }
    #[test]
    fn reveal_is_ordered_and_finishes() {
        assert_eq!(petal_progress(Some(0), 0), 0.0);
        for index in 0..6 {
            assert_eq!(petal_progress(Some(index as u64 * STAGGER_MS), index), 0.0);
            assert!(petal_progress(Some(index as u64 * STAGGER_MS + 20), index) > 0.0);
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
    /// Soft coloured glows on a deep or pale base: a synthetic wallpaper for the glass to
    /// refract, so README media never contains anyone's real desktop.
    fn wallpaper(w: usize, h: usize, dark: bool) -> Vec<u32> {
        let base = if dark {
            [11.0, 16.0, 32.0]
        } else {
            [233.0, 238.0, 247.0]
        };
        let glows = [
            ((0.18, 0.22), [124.0, 77.0, 255.0]),
            ((0.80, 0.18), [255.0, 84.0, 170.0]),
            ((0.62, 0.88), [32.0, 196.0, 230.0]),
            ((0.10, 0.92), [255.0, 150.0, 60.0]),
        ];
        let aspect = w as f32 / h as f32;
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
                let mut c = base;
                for ((gx, gy), glow) in glows {
                    let d2 = ((x - gx) * aspect).powi(2) + (y - gy).powi(2);
                    let k = (-d2 / 0.12).exp() * if dark { 0.8 } else { 0.5 };
                    for j in 0..3 {
                        c[j] += (glow[j] - c[j]) * k;
                    }
                }
                0xff000000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
            })
            .collect()
    }
    fn crop(src: &[u32], width: usize, x: usize, y: usize, w: usize, h: usize) -> Vec<u32> {
        (0..h)
            .flat_map(|row| src[(y + row) * width + x..][..w].iter().copied())
            .collect()
    }
    /// An opaque canvas surface holding `background`.
    unsafe fn canvas(background: &[u32], w: i32, h: i32) -> Surface {
        let s = new_surface(w, h, 1.0).unwrap();
        std::ptr::copy_nonoverlapping(background.as_ptr(), s.pixels, background.len());
        s
    }
    /// Composites a premultiplied surface onto an opaque canvas.
    unsafe fn place(dst: &Surface, src: &Surface, x: i32, y: i32) {
        GdipFlush(dst.graphics, FlushIntentionSync);
        GdipFlush(src.graphics, FlushIntentionSync);
        for sy in 0..src.height {
            for sx in 0..src.width {
                let (dx, dy) = (x + sx, y + sy);
                if dx < 0 || dy < 0 || dx >= dst.width || dy >= dst.height {
                    continue;
                }
                let s = *src.pixels.add((sy * src.width + sx) as usize);
                let a = s >> 24;
                if a == 0 {
                    continue;
                }
                let d = dst.pixels.add((dy * dst.width + dx) as usize);
                let mut out = 0xff000000;
                for shift in [0, 8, 16] {
                    let (sc, dc) = ((s >> shift) & 255, (*d >> shift) & 255);
                    out |= (sc + dc * (255 - a) / 255).min(255) << shift;
                }
                *d = out;
            }
        }
    }
    /// The radial menu in liquid glass (or classic), refracting the canvas behind it.
    unsafe fn menu_on(
        dst: &Surface,
        background: &[u32],
        (x, y, scale): (i32, i32, f64),
        dark: bool,
        glassy: bool,
        visual: &Visual,
        elapsed: Option<u64>,
    ) {
        let side = (400.0 * scale).round() as usize;
        let behind = crop(
            background,
            dst.width as usize,
            x as usize,
            y as usize,
            side,
            side,
        );
        let glass = glass_from(&behind, side, side, scale, dark, menu_bend);
        let lit = Visual {
            glass: glassy.then_some(&glass[..]),
            ..*visual
        };
        let menu = surface_visual(scale, dark, elapsed, &lit).unwrap();
        place(dst, &menu, x, y);
    }
    unsafe fn write(
        s: &Surface,
        value: &str,
        (x, y): (f32, f32),
        size: f32,
        color: u32,
        bold: bool,
    ) {
        let layout = rect(x, y, 900.0, size * 1.6);
        styled(s, value, layout, size, color, Style { left: true, bold });
    }
    fn pixels(s: &Surface) -> Vec<u32> {
        unsafe {
            GdipFlush(s.graphics, FlushIntentionSync);
            std::slice::from_raw_parts(s.pixels, (s.width * s.height) as usize).to_vec()
        }
    }
    /// Animated GIF that stores only the changed rectangle of each frame, with unchanged
    /// pixels transparent, so a mostly static background costs almost nothing.
    fn write_gif(path: &Path, w: u16, h: u16, frames: &[Vec<u32>], delay_ms: u16) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = gif::Encoder::new(file, w, h, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        let mut previous: Option<&Vec<u32>> = None;
        for frame in frames {
            let width = w as usize;
            let changed = |i: usize| previous.is_none_or(|p| p[i] != frame[i]);
            let (mut x0, mut y0, mut x1, mut y1) = (width, h as usize, 0, 0);
            for i in (0..frame.len()).filter(|&i| changed(i)) {
                let (x, y) = (i % width, i / width);
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
            if x0 >= x1 {
                (x0, y0, x1, y1) = (0, 0, 1, 1);
            }
            let mut rgba = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * width + x;
                    let c = frame[i];
                    let alpha = if changed(i) { 255 } else { 0 };
                    rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, alpha]);
                }
            }
            let mut out =
                gif::Frame::from_rgba_speed((x1 - x0) as u16, (y1 - y0) as u16, &mut rgba, 10);
            out.left = x0 as u16;
            out.top = y0 as u16;
            out.delay = delay_ms / 10;
            out.dispose = gif::DisposalMethod::Keep;
            encoder.write_frame(&out).unwrap();
            previous = Some(frame);
        }
    }
    #[test]
    #[ignore = "Writes the README media into docs/media; run explicitly when updating documentation"]
    fn export_readme_media() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/media");
        std::fs::create_dir_all(&out).unwrap();
        let formats = crate::convert::menu_formats(Path::new("sample.jpg"));
        let (white, muted, accent) = (0xfff7faff, 0xffa7b3c3, 0xffc9b8ff);
        unsafe {
            // Hero: the glass wheel with one petal lit, next to the pitch.
            let (w, h) = (1440, 640);
            let back = wallpaper(w, h, true);
            let hero = canvas(&back, w as i32, h as i32);
            let mut visual = Visual::menu(1, 0xff0078d4);
            visual.formats = formats;
            menu_on(&hero, &back, (150, 70, 1.25), true, true, &visual, None);
            let logo = image::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/petal.png"))
                .unwrap()
                .resize(64, 64, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let mark = new_surface(64, 64, 1.0).unwrap();
            for (x, y, p) in logo.enumerate_pixels() {
                let a = p[3] as u32;
                *mark.pixels.add((y * 64 + x) as usize) = a << 24
                    | (p[0] as u32 * a / 255) << 16
                    | (p[1] as u32 * a / 255) << 8
                    | p[2] as u32 * a / 255;
            }
            place(&hero, &mark, 780, 132);
            write(&hero, "Petal", (862.0, 124.0), 64.0, white, true);
            write(
                &hero,
                "Convert at a click.",
                (782.0, 246.0),
                42.0,
                white,
                true,
            );
            write(
                &hero,
                "Images, media and documents,",
                (784.0, 322.0),
                25.0,
                muted,
                false,
            );
            let pitch = "in a liquid glass wheel right where you work.";
            write(&hero, pitch, (784.0, 360.0), 25.0, muted, false);
            write(
                &hero,
                "WINDOWS 11   /   RUST   /   MIT",
                (786.0, 470.0),
                17.0,
                accent,
                true,
            );
            save(&hero, &out.join("hero.png"));

            // Themes: liquid glass and classic, dark and light.
            let (cw, ch) = (560usize, 480usize);
            let (w, h) = (cw * 2, ch * 2);
            let cells = [
                (0, 0, true, true, "Liquid glass · dark"),
                (1, 0, false, true, "Liquid glass · light"),
                (0, 1, true, false, "Classic · dark"),
                (1, 1, false, false, "Classic · light"),
            ];
            let mut back = vec![0u32; w * h];
            for (col, row, dark, glassy, _) in cells {
                let cell = if glassy {
                    wallpaper(cw, ch, dark)
                } else {
                    vec![if dark { 0xff0c1118 } else { 0xfff1f3f6 }; cw * ch]
                };
                for y in 0..ch {
                    let at = (row * ch + y) * w + col * cw;
                    back[at..at + cw].copy_from_slice(&cell[y * cw..][..cw]);
                }
            }
            let themes = canvas(&back, w as i32, h as i32);
            for (col, row, dark, glassy, label) in cells {
                let mut visual = Visual::menu(1, 0xff0078d4);
                visual.formats = formats;
                let (x, y) = ((col * cw + 100) as i32, (row * ch + 84) as i32);
                menu_on(&themes, &back, (x, y, 0.9), dark, glassy, &visual, None);
                let ink = if dark { white } else { 0xff172130 };
                let at = ((col * cw + 32) as f32, (row * ch + 26) as f32);
                write(&themes, label, at, 24.0, ink, true);
            }
            save(&themes, &out.join("themes.png"));

            // Quick settings: the glass panel opened from the tray, dark and light.
            let (pw, ph) = (PANEL_W as usize, PANEL_H as usize);
            let (w, h) = (pw * 2 + 120, ph + 80);
            let mut back = wallpaper(w, h, true);
            let light = wallpaper(w, h, false);
            for y in 0..h {
                let at = y * w + w / 2;
                back[at..at + w - w / 2].copy_from_slice(&light[at..at + w - w / 2]);
            }
            let quick = canvas(&back, w as i32, h as i32);
            for (x, dark, hover) in [(40usize, true, 0), (pw + 80, false, 4)] {
                let behind = crop(&back, w, x, 40, pw, ph);
                let glass = glass_from(&behind, pw, ph, 1.0, dark, panel_bend);
                let panel = Panel {
                    accent: 0xff0078d4,
                    glass: Some(&glass),
                    switches: [1.0, 1.0, 0.0],
                    hover,
                    open: 1.0,
                    alpha: 255,
                };
                let s = new_surface(PANEL_W, PANEL_H, 1.0).unwrap();
                draw_panel(&s, 1.0, dark, &panel);
                place(&quick, &s, x as i32, 40);
            }
            save(&quick, &out.join("quick-settings.png"));

            // Demo: bubble reveal, glowing hover, conversion and the check.
            let (w, h) = (1040usize, 520usize);
            let back = wallpaper(w, h, true);
            let mut frames = Vec::new();
            for frame in 0..96u64 {
                let mut visual = Visual::menu(-1, 0xff0078d4);
                visual.formats = formats;
                let elapsed = (frame < 14).then_some(frame * 40);
                let lit_png = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                match frame {
                    20..=25 => visual.hover = hover_weights([0.0; 7], 1, (frame - 20) * 40),
                    26..=33 => visual.hover = lit_png,
                    34..=39 => visual.hover = hover_weights(lit_png, 2, (frame - 34) * 40),
                    40..=45 => visual.hover[2] = 1.0,
                    46..=67 => {
                        visual.feedback = Feedback::Working {
                            elapsed: (frame - 46) * 40,
                            animate: true,
                        }
                    }
                    68.. => {
                        visual.feedback = Feedback::Success {
                            elapsed: ((frame - 68) * 40).min(CHECK_MS),
                        }
                    }
                    _ => (),
                }
                let s = canvas(&back, w as i32, h as i32);
                if frame > 0 {
                    menu_on(&s, &back, (76, 40, 1.1), true, true, &visual, elapsed);
                }
                let (title, note) = match frame {
                    0..=19 => ("Open the wheel", "Shift + right-click a supported file."),
                    20..=45 => ("Choose a format", "Hover lights the glass up."),
                    46..=67 => ("Converting…", "Petal checks the result before saving it."),
                    _ => (
                        "Conversion complete",
                        "A copy is saved next to the original.",
                    ),
                };
                write(&s, "PETAL", (580.0, 150.0), 17.0, accent, true);
                write(&s, title, (578.0, 186.0), 34.0, white, true);
                write(&s, note, (580.0, 250.0), 20.0, muted, false);
                let credit = "Rendered by the native Petal engine";
                write(&s, credit, (580.0, 392.0), 15.0, 0xff8391a8, false);
                frames.push(pixels(&s));
            }
            write_gif(&out.join("demo.gif"), w as u16, h as u16, &frames, 40);
        }
    }
    #[test]
    fn liquid_glass_frosts_refracts_and_renders_opaque_panes() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        for (dark, scale) in [(true, 0.75), (false, 1.5)] {
            let side = (400.0 * scale) as usize;
            // A busy striped "desktop": glass must smooth it, never pass it through raw.
            let raw: Vec<u32> = (0..side * side)
                .map(|i| {
                    if (i % side / 6 + i / side / 6) % 2 == 0 {
                        0xffe0402a
                    } else {
                        0xff2a60e0
                    }
                })
                .collect();
            let glass = glass_from(&raw, side, side, scale, dark, menu_bend);
            assert!(glass.iter().all(|c| c >> 24 == 255));
            let spread = |v: &[u32]| {
                let red: Vec<i32> = v.iter().map(|c| ((c >> 16) & 255) as i32).collect();
                red.iter().max().unwrap() - red.iter().min().unwrap()
            };
            assert!(spread(&glass) < spread(&raw) / 3, "glass must be frosted");
            for (name, elapsed) in [("rest", None), ("bubble", Some(120))] {
                let mut visual = Visual::menu(1, 0xff0078d4);
                visual.formats = crate::convert::menu_formats(std::path::Path::new("photo.jpg"));
                visual.glass = Some(&glass);
                let path = out.join(format!("glass-{name}-{dark}-{scale}.png"));
                unsafe {
                    save(
                        &surface_visual(scale, dark, elapsed, &visual).unwrap(),
                        &path,
                    );
                }
                let image = image::open(path).unwrap().to_rgba8();
                assert_eq!(image.get_pixel(0, 0)[3], 0);
                let at = |x: f64, y: f64| *image.get_pixel((x * scale) as u32, (y * scale) as u32);
                assert_eq!(at(200.0, 250.0)[3], 255, "glass center is opaque");
                if elapsed.is_none() {
                    for i in 0..6 {
                        let (x, y) = polar(160.0, -90.0 + i as f32 * 60.0);
                        let (dx, dy) = hover_offset(i, visual.hover[i], true);
                        assert_eq!(at((x + dx) as f64, (y + dy) as f64)[3], 255, "petal {i}");
                    }
                    // Hover lights the glass up (brighter, still neutral) instead of tinting it.
                    let (x, y) = polar(160.0, -30.0);
                    let (dx, dy) = hover_offset(1, 1.0, true);
                    let p = at((x + dx) as f64, (y + dy) as f64);
                    let rest = at(polar(160.0, 150.0).0 as f64, polar(160.0, 150.0).1 as f64);
                    let sum = |c: image::Rgba<u8>| c.0[..3].iter().map(|&v| v as i32).sum::<i32>();
                    assert!(
                        sum(p) > sum(rest) + 30,
                        "hover glow missing: {p:?} vs {rest:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn settings_panel_renders_and_hits_its_controls() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/visual-checks");
        std::fs::create_dir_all(&out).unwrap();
        assert_eq!(panel_hit(PANEL_W as f32 - 34.0, 30.0), 3);
        for row in 0..3 {
            assert_eq!(
                panel_hit(100.0, ROW_TOP + row as f32 * ROW_STEP + 28.0),
                row
            );
        }
        assert_eq!(panel_hit(5.0, 100.0), -1);
        assert_eq!(panel_hit(PANEL_W as f32 / 2.0, PANEL_H as f32 - 28.0), 4);
        for (dark, scale, glassy) in [(true, 1.0, true), (false, 1.5, true), (true, 1.25, false)] {
            let (w, h) = (
                (PANEL_W as f64 * scale) as usize,
                (PANEL_H as f64 * scale) as usize,
            );
            let raw: Vec<u32> = (0..w * h)
                .map(|i| {
                    0xff000000
                        | ((i % w * 255 / w) as u32) << 16
                        | ((i / w * 255 / h) as u32) << 8
                        | 0x90
                })
                .collect();
            let glass = glass_from(&raw, w, h, scale, dark, panel_bend);
            let panel = Panel {
                accent: 0xff0078d4,
                glass: glassy.then_some(&glass[..]),
                switches: [1.0, 0.5, 0.0],
                hover: 0,
                open: 1.0,
                alpha: 255,
            };
            let path = out.join(format!("panel-{dark}-{scale}-{glassy}.png"));
            unsafe {
                let s = new_surface(PANEL_W, PANEL_H, scale).unwrap();
                draw_panel(&s, scale as f32, dark, &panel);
                GdipFlush(s.graphics, FlushIntentionSync);
                save(&s, &path);
            }
            let image = image::open(path).unwrap().to_rgba8();
            assert_eq!(
                image.get_pixel(0, 0)[3],
                0,
                "rounded corner stays transparent"
            );
            assert_eq!(image.get_pixel(w as u32 / 2, h as u32 / 2)[3], 255);
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
                        *image.get_pixel((190.0 * scale) as u32, (190.0 * scale) as u32),
                        image::Rgba([0, 0, 0, 255])
                    );
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

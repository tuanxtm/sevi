use super::dpi::{self, Scale};
use crate::wide;
use std::result::Result::Ok;
use std::{cell::RefCell, collections::BTreeMap, ptr};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::*,
            Gdi::*,
            GdiPlus::*,
        },
        UI::WindowsAndMessaging::*,
    },
    core::{PCWSTR, w},
};
pub const BG: u32 = 0x00f6f6f6;
pub const GROUP: u32 = 0x00ffffff;
pub const BORDER: u32 = 0x00e2e2e2;
pub const SEP: u32 = 0x00ececec;
pub const TEXT: u32 = 0;
pub const SECONDARY: u32 = 0x00736e6e;
pub const TERTIARY: u32 = 0x00afaaaa;
pub const ACCENT: u32 = 0x00ff7c0a;
pub const GREEN: u32 = 0x0059c734;
pub const OFF: u32 = 0x00ebe9e9;
pub const BRAND: u32 = 0x00e5464f;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Font {
    Body,
    Bold,
    Small,
    Wordmark,
    Title,
    Mono,
    Title2,
    Callout,
    MonoBold,
    Wordmark24,
}
thread_local! {static FONTS:RefCell<BTreeMap<(Font,u32),HFONT>>=const{RefCell::new(BTreeMap::new())};static TOKEN:usize={let mut token=0;unsafe{GdiplusStartup(&mut token,&GdiplusStartupInput{GdiplusVersion:1,..Default::default()},ptr::null_mut());}token};static DWRITE:Option<IDWriteFactory>=unsafe{DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()};static D2D:Option<ID2D1Factory>=unsafe{D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,None).ok()};}
pub fn font(role: Font) -> HFONT {
    font_for_dpi(role, dpi::current())
}
pub fn font_for_dpi(role: Font, scale: Scale) -> HFONT {
    FONTS.with(|fonts| {
        *fonts
            .borrow_mut()
            .entry((role, scale.dpi()))
            .or_insert_with(|| {
                let (size, weight, mono) = match role {
                    Font::Body => (15, 400, false),
                    Font::Bold => (15, 600, false),
                    Font::Small => (13, 400, false),
                    Font::Wordmark => (24, 700, false),
                    Font::Title => (17, 600, false),
                    Font::Mono => (13, 400, true),
                    Font::Title2 => (20, 600, false),
                    Font::Callout => (14, 400, false),
                    Font::MonoBold => (15, 700, true),
                    Font::Wordmark24 => (26, 700, false),
                };
                let face = wide(if mono { "Consolas" } else { "Segoe UI" });
                unsafe {
                    CreateFontW(
                        -scale.pixels(size),
                        0,
                        0,
                        0,
                        weight,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        FONT_OUTPUT_PRECISION(0),
                        FONT_CLIP_PRECISION(0),
                        CLEARTYPE_QUALITY,
                        0,
                        PCWSTR(face.as_ptr()),
                    )
                }
            })
    })
}
pub fn rgb(color: u32, alpha: u8) -> u32 {
    ((alpha as u32) << 24) | ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255)
}
pub fn background_brush(color: u32) -> HBRUSH {
    thread_local! {static BRUSHES:RefCell<BTreeMap<u32,HBRUSH>>=const{RefCell::new(BTreeMap::new())};}
    BRUSHES.with(|b| {
        *b.borrow_mut()
            .entry(color)
            .or_insert_with(|| unsafe { CreateSolidBrush(COLORREF(color)) })
    })
}
pub struct Paint {
    pub dc: HDC,
    pub rect: RECT,
    window: HWND,
    screen: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    ps: PAINTSTRUCT,
    graphics: *mut GpGraphics,
    physical: RECT,
    _context: dpi::Context,
}
impl Paint {
    pub fn begin(window: HWND, color: u32) -> Self {
        let context = dpi::Context::for_window(window);
        let scale = dpi::current();
        TOKEN.with(|_| ());
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            let screen = BeginPaint(window, &mut ps);
            let mut rect = RECT::default();
            let _ = GetClientRect(window, &mut rect);
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, rect.right.max(1), rect.bottom.max(1));
            let old = SelectObject(dc, HGDIOBJ(bitmap.0));
            let brush = CreateSolidBrush(COLORREF(color));
            FillRect(dc, &rect, brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));
            let mut graphics = ptr::null_mut();
            GdipCreateFromHDC(dc, &mut graphics);
            if !graphics.is_null() {
                GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
                GdipSetPageUnit(graphics, UnitPixel);
                GdipScaleWorldTransform(
                    graphics,
                    scale.factor(),
                    scale.factor(),
                    MatrixOrderPrepend,
                );
            }
            Self {
                dc,
                rect: RECT {
                    left: 0,
                    top: 0,
                    right: scale.logical(rect.right),
                    bottom: scale.logical(rect.bottom),
                },
                window,
                screen,
                bitmap,
                old,
                ps,
                graphics,
                physical: rect,
                _context: context,
            }
        }
    }
    #[allow(clippy::too_many_arguments)] // Geometry and the two native paint styles.
    pub fn round(&self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: u32, stroke: Option<u32>) {
        if self.graphics.is_null() {
            return;
        }
        unsafe {
            let mut path = ptr::null_mut();
            GdipCreatePath(FillModeAlternate, &mut path);
            if path.is_null() {
                return;
            }
            let d = r * 2.0;
            for (ax, ay, start) in [
                (x, y, 180.0),
                (x + w - d, y, 270.0),
                (x + w - d, y + h - d, 0.0),
                (x, y + h - d, 90.0),
            ] {
                GdipAddPathArc(path, ax, ay, d, d, start, 90.0);
            }
            GdipClosePathFigure(path);
            let mut brush = ptr::null_mut();
            GdipCreateSolidFill(fill, &mut brush);
            if !brush.is_null() {
                GdipFillPath(self.graphics, brush.cast(), path);
                GdipDeleteBrush(brush.cast());
            }
            if let Some(stroke) = stroke {
                let mut pen = ptr::null_mut();
                GdipCreatePen1(stroke, 1.0, UnitPixel, &mut pen);
                if !pen.is_null() {
                    GdipDrawPath(self.graphics, pen, path);
                    GdipDeletePen(pen);
                }
            }
            GdipDeletePath(path);
        }
    }
    pub fn ellipse(&self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        unsafe {
            if self.graphics.is_null() {
                return;
            }
            let mut brush = ptr::null_mut();
            GdipCreateSolidFill(color, &mut brush);
            if !brush.is_null() {
                GdipFillEllipse(self.graphics, brush.cast(), x, y, w, h);
                GdipDeleteBrush(brush.cast());
            }
        }
    }
    pub fn line(&self, x: i32, y: i32, width: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(COLORREF(color));
            FillRect(
                self.dc,
                &dpi::rect_to_pixels(
                    RECT {
                        left: x,
                        top: y,
                        right: x + width,
                        bottom: y + 1,
                    },
                    dpi::current(),
                ),
                brush,
            );
            let _ = DeleteObject(HGDIOBJ(brush.0));
        }
    }
    pub fn text(&self, text: &str, role: Font, color: u32, rect: RECT, flags: DRAW_TEXT_FORMAT) {
        text_at(self.dc, text, role, color, rect, flags)
    }
}
impl Drop for Paint {
    fn drop(&mut self) {
        unsafe {
            if !self.graphics.is_null() {
                GdipDeleteGraphics(self.graphics);
            }
            let _ = BitBlt(
                self.screen,
                0,
                0,
                self.physical.right,
                self.physical.bottom,
                Some(self.dc),
                0,
                0,
                SRCCOPY,
            );
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
            let _ = EndPaint(self.window, &self.ps);
        }
    }
}
pub fn text_at(
    dc: HDC,
    text: &str,
    role: Font,
    color: u32,
    mut rect: RECT,
    flags: DRAW_TEXT_FORMAT,
) {
    rect = dpi::rect_to_pixels(rect, dpi::current());
    let mut text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        SelectObject(dc, HGDIOBJ(font(role).0));
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(color));
        DrawTextW(dc, &mut text, &mut rect, flags | DT_NOPREFIX);
    }
}
pub fn width(dc: HDC, text: &str, role: Font) -> i32 {
    let text: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        SelectObject(dc, HGDIOBJ(font(role).0));
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &text, &mut size);
        dpi::current().logical(size.cx)
    }
}
pub fn height(text: &str, role: Font, width: i32) -> i32 {
    unsafe {
        let dc = GetDC(None);
        let mut r = RECT {
            left: 0,
            top: 0,
            right: dpi::current().pixels(width),
            bottom: 0,
        };
        let mut text: Vec<u16> = text.encode_utf16().collect();
        SelectObject(dc, HGDIOBJ(font(role).0));
        DrawTextW(
            dc,
            &mut text,
            &mut r,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
        );
        ReleaseDC(None, dc);
        dpi::current().logical(r.bottom)
    }
}
pub fn wordmark(dc: HDC, role: Font, x: i32, y: i32) {
    text_at(
        dc,
        "Se",
        role,
        TEXT,
        RECT {
            left: x,
            top: y,
            right: x + 100,
            bottom: y + 40,
        },
        DT_SINGLELINE,
    );
    let offset = width(dc, "Se", role);
    text_at(
        dc,
        "vi",
        role,
        BRAND,
        RECT {
            left: x + offset,
            top: y,
            right: x + 140,
            bottom: y + 40,
        },
        DT_SINGLELINE,
    );
}
pub fn color_text(
    dc: HDC,
    text: &str,
    rect: RECT,
    size: f32,
    color: u32,
    face: &str,
    center: bool,
) {
    let scale = dpi::current();
    let physical = dpi::rect_to_pixels(rect, scale);
    let result = D2D.with(|d2d| {
        DWRITE.with(|dw| unsafe {
            let d2d = d2d.as_ref()?;
            let dw = dw.as_ref()?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let target = d2d.CreateDCRenderTarget(&props).ok()?;
            target.BindDC(dc, &physical).ok()?;
            let face = wide(face);
            let format = dw
                .CreateTextFormat(
                    PCWSTR(face.as_ptr()),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size * scale.factor(),
                    w!("vi"),
                )
                .ok()?;
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            if center {
                let _ = format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            }
            let brush = target
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: (color & 255) as f32 / 255.0,
                        g: ((color >> 8) & 255) as f32 / 255.0,
                        b: ((color >> 16) & 255) as f32 / 255.0,
                        a: 1.0,
                    },
                    None,
                )
                .ok()?;
            target.BeginDraw();
            let units: Vec<_> = text.encode_utf16().collect();
            target.DrawText(
                &units,
                &format,
                &D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: (physical.right - physical.left) as f32,
                    bottom: (physical.bottom - physical.top) as f32,
                },
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT | D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            target.EndDraw(None, None).ok()?;
            Some(())
        })
    });
    if result.is_none() {
        text_at(dc, text, Font::Body, color, rect, DT_WORDBREAK);
    }
}
pub fn icon(dc: HDC, x: i32, y: i32, size: i32) {
    let scale = dpi::current();
    let (x, y, size) = (scale.pixels(x), scale.pixels(y), scale.pixels(size));
    unsafe {
        let instance = windows::Win32::Foundation::HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        );
        if let Ok(icon) = LoadImageW(
            Some(instance),
            PCWSTR(101usize as *const u16),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        ) {
            let _ = DrawIconEx(dc, x, y, HICON(icon.0), size, size, 0, None, DI_NORMAL);
            let _ = DestroyIcon(HICON(icon.0));
        }
    }
}

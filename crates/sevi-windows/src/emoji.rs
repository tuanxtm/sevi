//! Native non-activating Windows emoji panel, with the existing nine-column grid.
use crate::{host, prefs::RegistryPreferences, ui, wide};
use sevi_runtime::{
    emoji::{EmojiIndex, convert_telex, frequent, record_usage},
    prefs::Preferences,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dwm::*,
            Gdi::*,
        },
        System::{DataExchange::*, Memory::*},
        UI::{HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
use windows_numerics::Vector2;
const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 404.0;
const CELL: f32 = 44.0;
const SEARCH: f32 = 46.0;
const FOOTER: f32 = 44.0;
const PAD: f32 = 8.0;
const COLS: usize = 9;
const VI: [&str; 10] = [
    "Mặt cười & cảm xúc",
    "Người & cơ thể",
    "",
    "Động vật & thiên nhiên",
    "Đồ ăn & thức uống",
    "Du lịch & địa điểm",
    "Hoạt động",
    "Đồ vật",
    "Biểu tượng",
    "Cờ",
];
const EN: [&str; 10] = [
    "Smileys & Emotion",
    "People & Body",
    "",
    "Animals & Nature",
    "Food & Drink",
    "Travel & Places",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];
const TONES: [&str; 6] = ["✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"];
struct Catalog {
    index: EmojiIndex,
    allowed: Vec<usize>,
}
static CATALOG: OnceLock<Arc<Catalog>> = OnceLock::new();
fn catalog() -> Arc<Catalog> {
    CATALOG
        .get_or_init(|| {
            let index = EmojiIndex::default();
            let glyphs = Glyphs::new();
            let allowed = index
                .all
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    let first = e.text.chars().next().unwrap_or('\0');
                    !('\u{1f1e6}'..='\u{1f1ff}').contains(&first)
                        && !e
                            .text
                            .chars()
                            .any(|c| ('\u{e0000}'..='\u{e0fff}').contains(&c))
                        && glyphs.drawable(&e.text)
                })
                .map(|(i, _)| i)
                .collect();
            Arc::new(Catalog { index, allowed })
        })
        .clone()
}
struct Glyphs {
    factory: Option<IDWriteFactory>,
    format: Option<IDWriteTextFormat>,
    face: Option<IDWriteFontFace>,
}
impl Glyphs {
    fn new() -> Self {
        let factory: Option<IDWriteFactory> =
            unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok() };
        let mut format = None;
        let mut face = None;
        if let Some(f) = &factory {
            unsafe {
                format = f
                    .CreateTextFormat(
                        w!("Segoe UI Emoji"),
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        28.0,
                        w!(""),
                    )
                    .ok();
                let mut collection = None;
                if f.GetSystemFontCollection(&mut collection, false).is_ok()
                    && let Some(c) = collection
                {
                    let (mut index, mut exists) = (0, windows::core::BOOL(0));
                    if c.FindFamilyName(w!("Segoe UI Emoji"), &mut index, &mut exists)
                        .is_ok()
                        && exists.as_bool()
                    {
                        face = c
                            .GetFontFamily(index)
                            .and_then(|f| {
                                f.GetFirstMatchingFont(
                                    DWRITE_FONT_WEIGHT_NORMAL,
                                    DWRITE_FONT_STRETCH_NORMAL,
                                    DWRITE_FONT_STYLE_NORMAL,
                                )
                            })
                            .and_then(|f| f.CreateFontFace())
                            .ok();
                    }
                }
            }
        }
        Self {
            factory,
            format,
            face,
        }
    }
    fn drawable(&self, text: &str) -> bool {
        let (Some(factory), Some(format), Some(face)) = (&self.factory, &self.format, &self.face)
        else {
            return true;
        };
        let Some(first) = text.chars().next() else {
            return false;
        };
        unsafe {
            let cp = first as u32;
            let mut glyph = 0;
            if face.GetGlyphIndices(&cp, 1, &mut glyph).is_err() || glyph == 0 {
                return false;
            }
            let units: Vec<_> = text.encode_utf16().collect();
            let Ok(layout) = factory.CreateTextLayout(&units, format, 1000.0, 100.0) else {
                return true;
            };
            let mut count = 0;
            let _ = layout.GetClusterMetrics(None, &mut count);
            count <= 1
        }
    }
}
#[derive(Clone)]
struct Row {
    title: String,
    start: usize,
    count: usize,
    y: f32,
    height: f32,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Font {
    Search,
    Header,
    Footer,
    Emoji,
    EmojiFooter,
    Tone,
    Wordmark,
    Badge,
    Empty,
}
struct Canvas {
    target: ID2D1HwndRenderTarget,
    factory: IDWriteFactory,
    formats: BTreeMap<Font, IDWriteTextFormat>,
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
fn color(argb: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((argb >> 16) & 255) as f32 / 255.0,
        g: ((argb >> 8) & 255) as f32 / 255.0,
        b: (argb & 255) as f32 / 255.0,
        a: (argb >> 24) as f32 / 255.0,
    }
}
impl Canvas {
    fn magnifier(&self) {
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&color(0xff6e6e73), None) {
                self.target.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2 { X: 21.0, Y: 24.0 },
                        radiusX: 5.5,
                        radiusY: 5.5,
                    },
                    &brush,
                    1.6,
                    None,
                );
                self.target.DrawLine(
                    Vector2 { X: 25.0, Y: 28.0 },
                    Vector2 { X: 30.0, Y: 33.0 },
                    &brush,
                    1.8,
                    None,
                );
            }
        }
    }
    fn icon(&self, x: f32, y: f32) {
        unsafe {
            let instance = HINSTANCE(
                windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                    .unwrap_or_default()
                    .0,
            );
            let Ok(handle) = LoadImageW(
                Some(instance),
                PCWSTR(101usize as *const u16),
                IMAGE_ICON,
                16,
                16,
                LR_DEFAULTCOLOR,
            ) else {
                return;
            };
            let icon = HICON(handle.0);
            let mut info = ICONINFO::default();
            if GetIconInfo(icon, &mut info).is_ok() {
                let mut bitmap = BITMAP::default();
                GetObjectW(
                    HGDIOBJ(info.hbmColor.0),
                    size_of::<BITMAP>() as i32,
                    Some((&mut bitmap as *mut BITMAP).cast()),
                );
                if bitmap.bmWidth > 0 && bitmap.bmHeight > 0 {
                    let mut pixels = vec![0u32; (bitmap.bmWidth * bitmap.bmHeight) as usize];
                    let mut bi = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: bitmap.bmWidth,
                            biHeight: -bitmap.bmHeight,
                            biPlanes: 1,
                            biBitCount: 32,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let dc = GetDC(None);
                    GetDIBits(
                        dc,
                        info.hbmColor,
                        0,
                        bitmap.bmHeight as u32,
                        Some(pixels.as_mut_ptr().cast()),
                        &mut bi,
                        DIB_RGB_COLORS,
                    );
                    ReleaseDC(None, dc);
                    for pixel in &mut pixels {
                        let alpha = *pixel >> 24;
                        if alpha > 0 && alpha < 255 {
                            let r = ((*pixel >> 16) & 255) * alpha / 255;
                            let g = ((*pixel >> 8) & 255) * alpha / 255;
                            let b = (*pixel & 255) * alpha / 255;
                            *pixel = (alpha << 24) | (r << 16) | (g << 8) | b;
                        }
                    }
                    if let Ok(bitmap)=self.target.CreateBitmap(D2D_SIZE_U{width:bitmap.bmWidth as u32,height:bitmap.bmHeight as u32},Some(pixels.as_ptr().cast()),bitmap.bmWidth as u32*4,&D2D1_BITMAP_PROPERTIES{pixelFormat:D2D1_PIXEL_FORMAT{format:windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,alphaMode:D2D1_ALPHA_MODE_PREMULTIPLIED},..Default::default()}){self.target.DrawBitmap(&bitmap,Some(&rect(x,y,16.0,16.0)),0.85,D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,None);}
                }
                let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
                let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
            }
            let _ = DestroyIcon(icon);
        }
    }
    fn new(window: HWND, dpi: f32) -> windows::core::Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let mut client = RECT::default();
            GetClientRect(window, &mut client)?;
            let target = factory.CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES::default(),
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: window,
                    pixelSize: D2D_SIZE_U {
                        width: client.right as u32,
                        height: client.bottom as u32,
                    },
                    ..Default::default()
                },
            )?;
            target.SetDpi(dpi, dpi);
            Ok(Self {
                target,
                factory: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
                formats: BTreeMap::new(),
            })
        }
    }
    fn format(&mut self, role: Font) -> Option<IDWriteTextFormat> {
        if let Some(f) = self.formats.get(&role) {
            return Some(f.clone());
        }
        let (face, size, weight, center) = match role {
            Font::Search => ("Segoe UI", 17.0, DWRITE_FONT_WEIGHT_NORMAL, false),
            Font::Header => ("Segoe UI", 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, false),
            Font::Footer => ("Segoe UI", 13.0, DWRITE_FONT_WEIGHT_NORMAL, false),
            Font::Emoji => ("Segoe UI Emoji", 28.0, DWRITE_FONT_WEIGHT_NORMAL, true),
            Font::EmojiFooter => ("Segoe UI Emoji", 22.0, DWRITE_FONT_WEIGHT_NORMAL, false),
            Font::Tone => ("Segoe UI Emoji", 18.0, DWRITE_FONT_WEIGHT_NORMAL, true),
            Font::Wordmark => ("Segoe UI", 13.0, DWRITE_FONT_WEIGHT_BOLD, false),
            Font::Badge => ("Segoe UI", 11.0, DWRITE_FONT_WEIGHT_SEMI_BOLD, true),
            Font::Empty => ("Segoe UI", 15.0, DWRITE_FONT_WEIGHT_NORMAL, true),
        };
        let face = wide(face);
        let f = unsafe {
            self.factory
                .CreateTextFormat(
                    PCWSTR(face.as_ptr()),
                    None,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!(""),
                )
                .ok()?
        };
        unsafe {
            let _ = f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
            let _ = f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
            if center {
                let _ = f.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
            }
            if role == Font::Footer
                && let Ok(sign) = self.factory.CreateEllipsisTrimmingSign(&f)
            {
                let _ = f.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        ..Default::default()
                    },
                    &sign,
                );
            }
        }
        self.formats.insert(role, f.clone());
        Some(f)
    }
    fn text(&mut self, text: &str, font: Font, bounds: D2D_RECT_F, argb: u32) {
        let Some(font) = self.format(font) else {
            return;
        };
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&color(argb), None) {
                let units: Vec<_> = text.encode_utf16().collect();
                self.target.DrawText(
                    &units,
                    &font,
                    &bounds,
                    &brush,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT | D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }
    fn width(&mut self, text: &str, font: Font) -> f32 {
        let Some(font) = self.format(font) else {
            return 0.0;
        };
        unsafe {
            let units: Vec<_> = text.encode_utf16().collect();
            let Ok(layout) = self.factory.CreateTextLayout(&units, &font, 10000.0, 100.0) else {
                return 0.0;
            };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            let _ = layout.GetMetrics(&mut metrics);
            metrics.widthIncludingTrailingWhitespace
        }
    }
    fn fill(&self, bounds: D2D_RECT_F, radius: f32, argb: u32) {
        unsafe {
            if let Ok(brush) = self.target.CreateSolidColorBrush(&color(argb), None) {
                if radius == 0.0 {
                    self.target.FillRectangle(&bounds, &brush)
                } else {
                    self.target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: bounds,
                            radiusX: radius,
                            radiusY: radius,
                        },
                        &brush,
                    )
                }
            }
        }
    }
}
struct Picker {
    window: HWND,
    open: bool,
    caret: bool,
    dpi: f32,
    canvas: Option<Canvas>,
    catalog: Arc<Catalog>,
    raw: String,
    all_selected: bool,
    vietnamese: bool,
    visible: Vec<usize>,
    recents: Vec<usize>,
    rows: Vec<Row>,
    content: f32,
    scroll: f32,
    selected: usize,
    tone: usize,
    last_mouse: POINT,
    packet_high: Option<u16>,
}
thread_local! {static PICKER:RefCell<Option<Picker>>=const{RefCell::new(None)};}
fn state<T: Default>(f: impl FnOnce(&mut Picker) -> T) -> T {
    PICKER.with(|p| {
        p.try_borrow_mut()
            .ok()
            .and_then(|mut p| p.as_mut().map(f))
            .unwrap_or_default()
    })
}
fn usage() -> BTreeMap<String, f64> {
    RegistryPreferences
        .string("EmojiUsage")
        .lines()
        .filter_map(|line| {
            let (k, v) = line.split_once('\t')?;
            Some((k.to_owned(), v.parse().ok()?))
        })
        .collect()
}
fn record(text: &str) {
    let mut map = usage();
    record_usage(&mut map, text);
    let value = map
        .iter()
        .map(|(text, n)| format!("{text}\t{n:.4}\n"))
        .collect::<String>();
    let _ = RegistryPreferences.set("EmojiUsage", value.into());
}
fn viewport() -> f32 {
    HEIGHT - SEARCH - 1.0 - FOOTER - 1.0 - 2.0 * PAD
}
impl Picker {
    fn layout(&mut self) {
        self.visible.clear();
        self.rows.clear();
        let mut groups = vec![];
        if !self.raw.trim_matches(' ').is_empty() {
            groups.push((
                String::new(),
                self.catalog
                    .index
                    .search(&self.raw, 400, Some(&self.catalog.allowed)),
            ))
        } else {
            groups.push((
                if self.vietnamese {
                    "Dùng thường xuyên"
                } else {
                    "Frequently Used"
                }
                .into(),
                self.recents.clone(),
            ));
            let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
            for &i in &self.catalog.allowed {
                grouped
                    .entry(self.catalog.index.all[i].group)
                    .or_default()
                    .push(i)
            }
            for (g, list) in grouped {
                if g < 10 {
                    groups.push(((if self.vietnamese { VI } else { EN })[g].into(), list));
                }
            }
        }
        let mut y = 0.0;
        for (title, list) in groups {
            if list.is_empty() {
                continue;
            }
            if !title.is_empty() {
                let height = if self.rows.is_empty() { 20.0 } else { 28.0 };
                self.rows.push(Row {
                    title,
                    start: 0,
                    count: 0,
                    y,
                    height,
                });
                y += height
            }
            let start = self.visible.len();
            let n = list.len();
            self.visible.extend(list);
            for i in (0..n).step_by(COLS) {
                self.rows.push(Row {
                    title: String::new(),
                    start: start + i,
                    count: COLS.min(n - i),
                    y,
                    height: CELL,
                });
                y += CELL
            }
        }
        self.content = y;
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
        self.scroll = self.scroll.clamp(0.0, (self.content - viewport()).max(0.0));
    }
    fn search(&mut self, raw: String) {
        self.raw = raw;
        self.all_selected = false;
        self.selected = 0;
        self.scroll = 0.0;
        self.layout();
        ui::invalidate(self.window);
    }
    fn type_text(&mut self, text: &str) {
        self.search(if self.all_selected {
            text.into()
        } else {
            self.raw.clone() + text
        });
    }
    fn move_to_row(&mut self, delta: isize) {
        let rows: Vec<_> = self.rows.iter().filter(|r| r.count > 0).collect();
        if let Some(index) = rows
            .iter()
            .position(|r| (r.start..r.start + r.count).contains(&self.selected))
        {
            let next = index as isize + delta;
            if next >= 0 && (next as usize) < rows.len() {
                let old = rows[index];
                let new = rows[next as usize];
                self.selected = new.start + (self.selected - old.start).min(new.count - 1);
            }
        }
        self.scroll_selected();
    }
    fn scroll_selected(&mut self) {
        if let Some(row) = self
            .rows
            .iter()
            .find(|r| r.count > 0 && (r.start..r.start + r.count).contains(&self.selected))
        {
            if self
                .rows
                .iter()
                .find(|r| r.count > 0)
                .is_some_and(|first| first.start == row.start)
            {
                self.scroll = 0.0;
            } else if row.y < self.scroll {
                self.scroll = row.y;
            } else if row.y + row.height > self.scroll + viewport() {
                self.scroll = row.y + row.height - viewport();
            }
        }
        self.scroll = self.scroll.clamp(0.0, (self.content - viewport()).max(0.0));
    }
    fn cell_at(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (x as f32 * 96.0 / self.dpi, y as f32 * 96.0 / self.dpi);
        if !(SEARCH + 1.0..=HEIGHT - FOOTER - 1.0).contains(&y) || x < 12.0 {
            return None;
        }
        let col = ((x - 12.0) / CELL) as usize;
        self.rows
            .iter()
            .find(|r| {
                r.count > 0
                    && y >= SEARCH + 1.0 + PAD + r.y - self.scroll
                    && y < SEARCH + 1.0 + PAD + r.y - self.scroll + r.height
            })
            .and_then(|r| {
                if col < r.count {
                    Some(r.start + col)
                } else {
                    None
                }
            })
    }
    fn close(&mut self) {
        if self.open {
            self.open = false;
            host::set_picker(HWND::default());
            unsafe {
                let _ = KillTimer(Some(self.window), 1);
                let _ = ShowWindow(self.window, SW_HIDE);
            }
        }
    }
    fn choose(&mut self) {
        let Some(&index) = self.visible.get(self.selected) else {
            return;
        };
        let emoji = &self.catalog.index.all[index];
        let text = emoji.with_skin(self.tone).to_owned();
        record(&emoji.text);
        self.close();
        host::insert_text(&text);
    }
    fn paint(&mut self) {
        if self.canvas.is_none() {
            self.canvas = Canvas::new(self.window, self.dpi).ok()
        }
        let Some(mut c) = self.canvas.take() else {
            return;
        };
        unsafe {
            c.target.BeginDraw();
            c.target.Clear(Some(&color(0xfff7f7f8)));
        }
        let shown = if self.vietnamese {
            convert_telex(&self.raw)
        } else {
            self.raw.clone()
        };
        let width = c.width(&shown, Font::Search);
        let badge = rect(WIDTH - 40.0, 18.0, 26.0, 16.0);
        if self.all_selected && !shown.is_empty() {
            c.fill(rect(39.0, 15.0, width + 2.0, 22.0), 3.0, 0x4d4f46e5);
        }
        c.magnifier();
        c.text(
            &shown,
            Font::Search,
            rect(40.0, 14.0, badge.left - 48.0, 24.0),
            0xff000000,
        );
        if self.caret && !self.all_selected {
            c.fill(rect(41.0 + width, 17.0, 2.0, 18.0), 0.0, 0xff4f46e5)
        }
        if self.raw.is_empty() {
            c.text(
                if self.vietnamese {
                    "Tìm emoji: ngựa, horse…"
                } else {
                    "Search emoji: horse, ngựa…"
                },
                Font::Search,
                rect(44.0, 14.0, badge.left - 52.0, 24.0),
                0xffa0a0a5,
            )
        }
        c.fill(badge, 5.0, 0x12000000);
        c.text(
            if self.vietnamese { "VI" } else { "EN" },
            Font::Badge,
            badge,
            0xff6e6e73,
        );
        c.fill(rect(0.0, SEARCH, WIDTH, 1.0), 0.0, 0x1a000000);
        unsafe {
            c.target.PushAxisAlignedClip(
                &rect(0.0, SEARCH + 1.0 + PAD, WIDTH, viewport()),
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
        }
        if !self.raw.trim_matches(' ').is_empty() && self.visible.is_empty() {
            c.text(
                if self.vietnamese {
                    "Không tìm thấy emoji nào"
                } else {
                    "No emoji found"
                },
                Font::Empty,
                rect(0.0, SEARCH + 101.0, WIDTH, 40.0),
                0xff6e6e73,
            );
        }
        for row in &self.rows {
            let y = SEARCH + 1.0 + PAD + row.y - self.scroll;
            if y + row.height < SEARCH + 1.0 || y > HEIGHT - FOOTER - 1.0 {
                continue;
            }
            if row.count == 0 {
                c.text(
                    &row.title,
                    Font::Header,
                    rect(12.0, y + row.height - 20.0, WIDTH - 24.0, 14.0),
                    0xff6e6e73,
                );
                continue;
            }
            for col in 0..row.count {
                let i = row.start + col;
                let bounds = rect(12.0 + col as f32 * CELL, y, CELL, CELL);
                if i == self.selected {
                    c.fill(bounds, 8.0, 0x384f46e5)
                }
                let emoji = &self.catalog.index.all[self.visible[i]];
                c.text(emoji.with_skin(self.tone), Font::Emoji, bounds, 0xff000000);
            }
        }
        unsafe {
            c.target.PopAxisAlignedClip();
        }
        let fy = HEIGHT - FOOTER;
        c.fill(rect(0.0, fy - 1.0, WIDTH, 1.0), 0.0, 0x1a000000);
        if let Some(&i) = self.visible.get(self.selected) {
            let e = &self.catalog.index.all[i];
            c.text(
                e.with_skin(self.tone),
                Font::EmojiFooter,
                rect(14.0, fy, 32.0, FOOTER),
                0xff000000,
            );
            c.text(
                if self.vietnamese {
                    &e.name_vi
                } else {
                    &e.name_en
                },
                Font::Footer,
                rect(52.0, fy, WIDTH - 192.0, FOOTER),
                0xff6e6e73,
            );
        }
        c.text(
            TONES[self.tone],
            Font::Tone,
            rect(WIDTH - 42.0, fy + 8.0, 28.0, 28.0),
            0xff000000,
        );
        c.fill(rect(WIDTH - 52.0, fy + 14.0, 1.0, 16.0), 0.0, 0x1a000000);
        let suffix_w = c.width("vi", Font::Wordmark);
        let prefix_w = c.width("Se", Font::Wordmark);
        c.text(
            "vi",
            Font::Wordmark,
            rect(WIDTH - 62.0 - suffix_w, fy, suffix_w, FOOTER),
            0xff4f46e5,
        );
        c.text(
            "Se",
            Font::Wordmark,
            rect(WIDTH - 62.0 - suffix_w - prefix_w, fy, prefix_w, FOOTER),
            0xff000000,
        );
        c.icon(WIDTH - 62.0 - suffix_w - prefix_w - 21.0, fy + 14.0);
        if unsafe { c.target.EndDraw(None, None) }.is_ok() {
            self.canvas = Some(c)
        }
    }
    fn key(&mut self, vk: u32, flags: u32) {
        let (ctrl, alt, super_) = (flags & 2 != 0, flags & 4 != 0, flags & 8 != 0);
        match vk {
            0x1b => {
                if self.raw.is_empty() {
                    self.close()
                } else {
                    self.search(String::new())
                }
            }
            0x0d => self.choose(),
            0x25 => {
                self.all_selected = false;
                self.selected = self.selected.saturating_sub(1);
                self.scroll_selected();
            }
            0x27 => {
                self.all_selected = false;
                self.selected = (self.selected + 1).min(self.visible.len().saturating_sub(1));
                self.scroll_selected();
            }
            0x26 => {
                self.all_selected = false;
                self.move_to_row(-1)
            }
            0x28 => {
                self.all_selected = false;
                self.move_to_row(1)
            }
            0x08 => {
                if ctrl || self.all_selected {
                    self.search(String::new())
                } else {
                    let mut raw = self.raw.clone();
                    if alt {
                        raw = raw.trim_end_matches(' ').into();
                        while !raw.is_empty() && !raw.ends_with(' ') {
                            raw.pop();
                        }
                    } else {
                        raw.pop();
                    }
                    self.search(raw);
                }
            }
            _ => {
                if ctrl && !alt {
                    match vk {
                        0x56 => {
                            if let Some(text) = clipboard_read() {
                                self.type_text(&text)
                            }
                        }
                        0x41 if !self.raw.is_empty() => self.all_selected = true,
                        0x43 | 0x58 if self.all_selected && !self.raw.is_empty() => {
                            let shown = if self.vietnamese {
                                convert_telex(&self.raw)
                            } else {
                                self.raw.clone()
                            };
                            clipboard_write(&shown);
                            if vk == 0x58 {
                                self.search(String::new())
                            }
                        }
                        0x57 => self.close(),
                        _ => (),
                    }
                } else if !alt
                    && !super_
                    && let Some(c) = character(vk, flags)
                {
                    self.type_text(&c.to_string())
                }
            }
        }
        ui::invalidate(self.window);
    }
}
fn clipboard_read() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;
        let result = (|| {
            let handle = GetClipboardData(13).ok()?;
            let text = GlobalLock(HGLOBAL(handle.0)) as *const u16;
            if text.is_null() {
                return None;
            }
            let mut n = 0;
            while *text.add(n) != 0 {
                n += 1
            }
            let value = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
            let _ = GlobalUnlock(HGLOBAL(handle.0));
            Some(value)
        })();
        let _ = CloseClipboard();
        result
    }
}
fn clipboard_write(text: &str) {
    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let units = wide(text);
        if EmptyClipboard().is_ok()
            && let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2)
        {
            let dest = GlobalLock(memory) as *mut u16;
            if !dest.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), dest, units.len());
                let _ = GlobalUnlock(memory);
                if SetClipboardData(13, Some(HANDLE(memory.0))).is_err() {
                    let _ = GlobalFree(Some(memory));
                }
            } else {
                let _ = GlobalFree(Some(memory));
            }
        }
        let _ = CloseClipboard();
    }
}
fn character(vk: u32, flags: u32) -> Option<char> {
    unsafe {
        let mut state = [0u8; 256];
        if flags & 1 != 0 {
            state[VK_SHIFT.0 as usize] = 0x80
        }
        if flags & 16 != 0 {
            state[VK_CAPITAL.0 as usize] = 1
        }
        let layout = GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), None));
        let mut units = [0u16; 8];
        let count = ToUnicodeEx(
            vk,
            MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC, Some(layout)),
            &state,
            &mut units,
            4,
            Some(layout),
        );
        if count == 1 && units[0] >= 32 {
            char::from_u32(units[0] as u32)
        } else {
            None
        }
    }
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    ui::callback(|| unsafe {
        match m {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(h, &mut ps);
                state(Picker::paint);
                let _ = EndPaint(h, &ps);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            host::WM_EMOJI_KEY => {
                state(|s| s.key(w.0 as u32, l.0 as u32));
                return LRESULT(0);
            }
            host::WM_EMOJI_CLOSE => {
                state(Picker::close);
                return LRESULT(0);
            }
            host::WM_EMOJI_PACKET => {
                state(|s| {
                    let unit = w.0 as u16;
                    if (0xd800..=0xdbff).contains(&unit) {
                        s.packet_high = Some(unit)
                    } else if (0xdc00..=0xdfff).contains(&unit) {
                        if let Some(high) = s.packet_high.take() {
                            let cp =
                                0x10000 + ((high as u32 - 0xd800) << 10) + (unit as u32 - 0xdc00);
                            if let Some(c) = char::from_u32(cp) {
                                s.type_text(&c.to_string());
                            }
                        }
                    } else {
                        if let Some(c) = char::from_u32(unit as u32).filter(|c| *c >= ' ') {
                            s.type_text(&c.to_string())
                        }
                    }
                });
                return LRESULT(0);
            }
            WM_TIMER => {
                state(|s| {
                    s.caret = !s.caret;
                    ui::invalidate(h);
                });
                return LRESULT(0);
            }
            WM_MOUSEMOVE => {
                state(|s| {
                    let mut point = POINT::default();
                    let _ = GetCursorPos(&mut point);
                    if point == s.last_mouse {
                        return;
                    }
                    s.last_mouse = point;
                    if let Some(index) =
                        s.cell_at(l.0 as u16 as i16 as i32, (l.0 >> 16) as u16 as i16 as i32)
                        && s.selected != index
                    {
                        s.selected = index;
                        ui::invalidate(h);
                    }
                });
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                state(|s| {
                    let x = l.0 as u16 as i16 as i32;
                    let y = (l.0 >> 16) as u16 as i16 as i32;
                    if x as f32 * 96.0 / s.dpi >= WIDTH - 46.0
                        && y as f32 * 96.0 / s.dpi >= HEIGHT - FOOTER + 4.0
                    {
                        s.tone = (s.tone + 1) % 6;
                        let _ = RegistryPreferences.set_int("EmojiSkinTone", s.tone as i32);
                        ui::invalidate(h);
                    } else if let Some(index) = s.cell_at(x, y) {
                        s.selected = index;
                        s.choose();
                    }
                });
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                state(|s| {
                    s.scroll -= (w.0 >> 16) as u16 as i16 as f32 / 120.0 * CELL;
                    s.scroll = s.scroll.clamp(0.0, (s.content - viewport()).max(0.0));
                    ui::invalidate(h);
                });
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                state(|s| {
                    s.dpi = (w.0 & 65535) as f32;
                    s.canvas = None;
                });
                ui::dpi::apply_suggested(h, l);
                ui::invalidate(h);
                return LRESULT(0);
            }
            WM_DESTROY => {
                PICKER.with(|s| {
                    s.borrow_mut().take();
                });
                return LRESULT(0);
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
fn prepare() {
    if !state(|s| s.window).is_invalid() {
        return;
    }
    ui::register("SeviEmoji", Some(proc), None);
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            w!("SeviEmoji"),
            w!("Sevi Emoji"),
            WS_POPUP,
            0,
            0,
            WIDTH as i32,
            HEIGHT as i32,
            None,
            None,
            None,
            None,
        )
    }
    .unwrap_or_default();
    if window.is_invalid() {
        return;
    }
    unsafe {
        let round = 2u32;
        let _ = DwmSetWindowAttribute(
            window,
            DWMWINDOWATTRIBUTE(33),
            (&round as *const u32).cast(),
            4,
        );
        let border = 0x00dad6d6u32;
        let _ = DwmSetWindowAttribute(
            window,
            DWMWINDOWATTRIBUTE(34),
            (&border as *const u32).cast(),
            4,
        );
    }
    PICKER.with(|s| {
        *s.borrow_mut() = Some(Picker {
            window,
            open: false,
            caret: true,
            dpi: 96.0,
            canvas: None,
            catalog: catalog(),
            raw: String::new(),
            all_selected: false,
            vietnamese: true,
            visible: vec![],
            recents: vec![],
            rows: vec![],
            content: 0.0,
            scroll: 0.0,
            selected: 0,
            tone: 0,
            last_mouse: POINT::default(),
            packet_high: None,
        })
    });
}
pub fn install() {
    std::thread::spawn(|| {
        catalog();
    });
    settings_changed();
}
pub fn settings_changed() {
    if RegistryPreferences.int("EmojiPicker", 0) != 0 {
        prepare()
    }
}
pub fn toggle() {
    prepare();
    state(|s| {
        if s.open {
            s.close();
            return;
        }
        s.vietnamese = host::language() == sevi_engine::Language::Vietnamese;
        s.raw.clear();
        s.all_selected = false;
        s.selected = 0;
        s.scroll = 0.0;
        s.packet_high = None;
        s.tone = RegistryPreferences.int("EmojiSkinTone", 0).clamp(0, 5) as usize;
        let by_char: BTreeMap<_, _> = s
            .catalog
            .allowed
            .iter()
            .map(|&i| (s.catalog.index.all[i].text.as_str(), i))
            .collect();
        s.recents = frequent(&usage(), 27)
            .iter()
            .filter_map(|text| by_char.get(text.as_str()).copied())
            .collect();
        s.layout();
        unsafe {
            let _ = GetCursorPos(&mut s.last_mouse);
            let mut monitor = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(
                MonitorFromPoint(s.last_mouse, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            );
            let area = monitor.rcWork;
            // Query the destination window DPI after moving its hidden window onto that monitor.
            let _ = SetWindowPos(
                s.window,
                None,
                area.left,
                area.top,
                1,
                1,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            s.dpi = GetDpiForWindow(s.window) as f32;
            let scale = s.dpi / 96.0;
            let (width, height) = ((WIDTH * scale) as i32, (HEIGHT * scale) as i32);
            let x = (area.left + area.right - width) / 2;
            let y = (area.top + area.bottom - height) / 2
                - ((area.bottom - area.top) as f32 * 0.08) as i32;
            s.canvas = None;
            let _ = SetWindowPos(
                s.window,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            host::set_picker(s.window);
            s.open = true;
            s.caret = true;
            SetTimer(Some(s.window), 1, 530, None);
            ui::invalidate(s.window);
        }
    });
}
pub fn language_changed() {
    state(|s| {
        let vi = host::language() == sevi_engine::Language::Vietnamese;
        if vi != s.vietnamese {
            s.vietnamese = vi;
            if s.open {
                s.selected = 0;
                s.scroll = 0.0;
                s.layout();
                ui::invalidate(s.window);
            }
        }
    });
}

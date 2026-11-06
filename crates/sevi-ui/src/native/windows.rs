use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use sevi_ui_protocol::PickerInput;
use std::{ptr, sync::mpsc};
use windows::Win32::UI::Shell::{
    DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Direct2D::Common::*, Direct2D::*, DirectWrite::*, Dxgi::Common::*, Imaging::*},
        System::{Com::*, LibraryLoader::GetModuleHandleW},
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::w,
};
const INPUT: u32 = WM_APP + 10;
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
pub(super) fn filter(index: &mut sevi_runtime::emoji::EmojiIndex) {
    let glyphs = Glyphs::new();
    index.all.retain(|e| {
        let first = e.text.chars().next().unwrap_or('\0');
        !('\u{1f1e6}'..='\u{1f1ff}').contains(&first)
            && !e
                .text
                .chars()
                .any(|c| ('\u{e0000}'..='\u{e0fff}').contains(&c))
            && glyphs.drawable(&e.text)
    });
}
const CLOSE: u32 = WM_APP + 11;
const PACKET: u32 = WM_APP + 12;
struct Data {
    events: mpsc::Sender<PickerInput>,
    ctx: egui::Context,
    high: Option<u16>,
}
unsafe extern "system" fn proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    let state = unsafe { &mut *(data as *mut Data) };
    let mut input = None;
    if msg == CLOSE {
        input = Some(PickerInput::Escape);
    }
    if msg == PACKET {
        let unit = wp.0 as u16;
        if (0xd800..=0xdbff).contains(&unit) {
            state.high = Some(unit);
        } else {
            let units: Vec<u16> = state
                .high
                .take()
                .into_iter()
                .chain(std::iter::once(unit))
                .collect();
            input = Some(PickerInput::Text(String::from_utf16_lossy(&units)));
        }
    }
    if msg == INPUT {
        let vk = wp.0 as u16;
        let flags = lp.0 as u32;
        let ctrl = flags & 2 != 0;
        let alt = flags & 4 != 0;
        input = match vk {
            0x1b => Some(PickerInput::Escape),
            0x0d => Some(PickerInput::Enter),
            0x08 => Some(PickerInput::Backspace {
                word: ctrl || alt,
                all: false,
            }),
            0x25 => Some(PickerInput::Move { dx: -1, dy: 0 }),
            0x27 => Some(PickerInput::Move { dx: 1, dy: 0 }),
            0x26 => Some(PickerInput::Move { dx: 0, dy: -1 }),
            0x28 => Some(PickerInput::Move { dx: 0, dy: 1 }),
            0x41 if ctrl => Some(PickerInput::SelectAll),
            0x56 if ctrl => Some(PickerInput::Paste),
            _ if !ctrl && !alt && flags & 8 == 0 => unsafe {
                let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
                let layout = GetKeyboardLayout(thread);
                let mut keys = [0u8; 256];
                if flags & 1 != 0 {
                    keys[VK_SHIFT.0 as usize] = 0x80;
                }
                if flags & 16 != 0 {
                    keys[VK_CAPITAL.0 as usize] = 1;
                }
                let mut text = [0u16; 8];
                let n = ToUnicodeEx(
                    u32::from(vk),
                    MapVirtualKeyExW(u32::from(vk), MAPVK_VK_TO_VSC, Some(layout)),
                    &keys,
                    &mut text,
                    4,
                    Some(layout),
                );
                (n > 0).then(|| PickerInput::Text(String::from_utf16_lossy(&text[..n as usize])))
            },
            _ => None,
        };
    }
    if let Some(input) = input {
        let _ = state.events.send(input);
        state.ctx.request_repaint();
        return LRESULT(0);
    }
    if msg == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(proc), 1);
            drop(Box::from_raw(data as *mut Data));
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}
pub(super) fn configure(
    frame: &eframe::Frame,
    picker: bool,
    ctx: &egui::Context,
    events: mpsc::Sender<PickerInput>,
) -> u64 {
    let Ok(handle) = frame.window_handle() else {
        return 0;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return 0;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let style = if picker {
            style | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0
        } else {
            style & !WS_EX_NOACTIVATE.0 & !WS_EX_TOOLWINDOW.0
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style as isize);
        let module = GetModuleHandleW(None).ok();
        if let Some(module) = module
            && let Ok(icon) = LoadIconW(
                Some(HINSTANCE(module.0)),
                windows::core::PCWSTR(101usize as *const u16),
            )
        {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL as usize)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
        if picker {
            let data = Box::new(Data {
                events,
                ctx: ctx.clone(),
                high: None,
            });
            let raw = Box::into_raw(data);
            if !SetWindowSubclass(hwnd, Some(proc), 1, raw as usize).as_bool() {
                drop(Box::from_raw(raw));
            }
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
            );
        } else {
            let mut data = 0;
            if GetWindowSubclass(hwnd, Some(proc), 1, Some(&mut data)).as_bool() {
                let _ = RemoveWindowSubclass(hwnd, Some(proc), 1);
                drop(Box::from_raw(data as *mut Data));
            }
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_NOTOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
            );
        }
    }
    handle.hwnd.get() as u64
}
pub(super) fn glyph(text: &str, pixels: u32) -> Option<egui::ColorImage> {
    unsafe {
        thread_local! {static COM:bool=unsafe{CoInitializeEx(None,COINIT_APARTMENTTHREADED).is_ok()};}
        COM.with(|_| ());
        let wic: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let bitmap = wic
            .CreateBitmap(
                pixels,
                pixels,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )
            .ok()?;
        let factory: ID2D1Factory =
            D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).ok()?;
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.,
            dpiY: 96.,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let target = factory.CreateWicBitmapRenderTarget(&bitmap, &props).ok()?;
        let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let format = write
            .CreateTextFormat(
                w!("Segoe UI Emoji"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                pixels as f32 * 0.72,
                w!("en-us"),
            )
            .ok()?;
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER).ok()?;
        format
            .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
            .ok()?;
        let brush = target
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 1.,
                },
                None,
            )
            .ok()?;
        target.BeginDraw();
        target.Clear(Some(&D2D1_COLOR_F {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 0.,
        }));
        let units: Vec<u16> = text.encode_utf16().collect();
        target.DrawText(
            &units,
            &format,
            &D2D_RECT_F {
                left: 0.,
                top: 0.,
                right: pixels as f32,
                bottom: pixels as f32,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None).ok()?;
        let mut bytes = vec![0u8; (pixels * pixels * 4) as usize];
        bitmap
            .CopyPixels(ptr::null(), pixels * 4, &mut bytes)
            .ok()?;
        Some(super::bgra(&bytes, pixels as usize))
    }
}
pub(super) fn clipboard() -> Option<String> {
    use windows::Win32::System::{DataExchange::*, Memory::*};
    unsafe {
        OpenClipboard(None).ok()?;
        let result = (|| {
            let handle = GetClipboardData(13).ok()?;
            let memory = HGLOBAL(handle.0);
            let ptr = GlobalLock(memory).cast::<u16>();
            if ptr.is_null() {
                return None;
            }
            let units = std::slice::from_raw_parts(ptr, (GlobalSize(memory) / 2).min(4096));
            let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
            let text = String::from_utf16_lossy(&units[..end]);
            let _ = GlobalUnlock(memory);
            Some(text)
        })();
        let _ = CloseClipboard();
        result
    }
}

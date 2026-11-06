use super::{
    draw::{self, Font},
    *,
};
pub use crate::dpi::Scale;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};
use windows::Win32::UI::{
    Controls::*,
    HiDpi::*,
    Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
};
const SUBCLASS: usize = 0x564b4450;
#[derive(Clone)]
struct Control {
    bounds: Option<RECT>,
    font: Font,
    columns: Vec<(i32, i32)>,
}
struct Window {
    controls: BTreeMap<usize, Control>,
}
thread_local! {
    static WINDOWS: RefCell<BTreeMap<usize,Window>> = const { RefCell::new(BTreeMap::new()) };
    static CURRENT: Cell<u32> = const { Cell::new(96) };
}
pub struct Context(u32);
impl Context {
    pub fn for_window(hwnd: HWND) -> Self {
        Self(CURRENT.with(|s| s.replace(window(hwnd).dpi())))
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        CURRENT.with(|s| s.set(self.0));
    }
}
pub fn current() -> Scale {
    Scale::new(CURRENT.with(Cell::get))
}
pub fn window(hwnd: HWND) -> Scale {
    Scale::new(unsafe { GetDpiForWindow(hwnd) })
}
pub fn rect_to_pixels(rect: RECT, scale: Scale) -> RECT {
    RECT {
        left: scale.pixels(rect.left),
        top: scale.pixels(rect.top),
        right: scale.pixels(rect.right),
        bottom: scale.pixels(rect.bottom),
    }
}
pub fn client_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    }
    let s = window(hwnd);
    RECT {
        left: 0,
        top: 0,
        right: s.logical(rect.right),
        bottom: s.logical(rect.bottom),
    }
}
pub fn track_window(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    WINDOWS.with(|w| {
        w.borrow_mut().insert(
            hwnd.0 as usize,
            Window {
                controls: BTreeMap::new(),
            },
        );
    });
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS, 0);
    }
}
pub fn track_control(parent: HWND, child: HWND, bounds: Option<RECT>, font: Font) {
    if child.is_invalid() {
        return;
    }
    WINDOWS.with(|w| {
        if let Some(w) = w.borrow_mut().get_mut(&(parent.0 as usize)) {
            w.controls.insert(
                child.0 as usize,
                Control {
                    bounds,
                    font,
                    columns: Vec::new(),
                },
            );
        }
    });
    unsafe {
        SendMessageW(
            child,
            WM_SETFONT,
            Some(WPARAM(draw::font_for_dpi(font, window(parent)).0 as usize)),
            Some(LPARAM(1)),
        );
        if bounds.is_none() {
            SendMessageW(
                child,
                TTM_SETMAXTIPWIDTH,
                None,
                Some(LPARAM(window(parent).pixels(300) as isize)),
            );
        }
    }
}
pub fn forget_children(parent: HWND) {
    WINDOWS.with(|w| {
        if let Some(w) = w.borrow_mut().get_mut(&(parent.0 as usize)) {
            w.controls.clear();
        }
    });
}
pub fn column(list: HWND, index: i32, width: i32) -> i32 {
    let parent = unsafe { GetParent(list) }.unwrap_or_default();
    WINDOWS.with(|w| {
        if let Some(c) = w
            .borrow_mut()
            .get_mut(&(parent.0 as usize))
            .and_then(|w| w.controls.get_mut(&(list.0 as usize)))
        {
            c.columns.push((index, width));
        }
    });
    window(list).pixels(width)
}
pub fn move_control(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    let parent = unsafe { GetParent(hwnd) }.unwrap_or_default();
    let rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    WINDOWS.with(|w| {
        if let Some(c) = w
            .borrow_mut()
            .get_mut(&(parent.0 as usize))
            .and_then(|w| w.controls.get_mut(&(hwnd.0 as usize)))
        {
            c.bounds = Some(rect);
        }
    });
    let r = rect_to_pixels(rect, window(parent));
    unsafe {
        let _ = MoveWindow(
            hwnd,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            true,
        );
    }
}
pub fn set_client_size(hwnd: HWND, width: i32, height: i32) {
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongPtrW(hwnd, GWL_STYLE) as u32);
        let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
        let mut rect = rect_to_pixels(
            RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            },
            window(hwnd),
        );
        let _ = AdjustWindowRectExForDpi(
            &mut rect,
            style,
            !GetMenu(hwnd).is_invalid(),
            ex,
            window(hwnd).dpi(),
        );
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
/// The recommended rectangle is in physical screen coordinates.
///
/// # Safety
/// A nonzero `lparam` must point to the valid RECT supplied with WM_DPICHANGED.
pub unsafe fn apply_suggested(hwnd: HWND, lparam: LPARAM) {
    if lparam.0 != 0 {
        let rect = unsafe { *(lparam.0 as *const RECT) };
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
unsafe extern "system" fn window_proc(
    h: HWND,
    m: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    callback(|| unsafe {
        if m == WM_DPICHANGED {
            let scale = Scale::new((w.0 & 65535) as u32);
            let controls = WINDOWS
                .with(|windows| {
                    windows
                        .borrow()
                        .get(&(h.0 as usize))
                        .map(|w| w.controls.clone())
                })
                .unwrap_or_default();
            // Release the layout borrow before invoking Win32: moving children can send synchronous messages.
            for (handle, control) in controls {
                let child = HWND(handle as *mut _);
                if !IsWindow(Some(child)).as_bool() {
                    continue;
                }
                if let Some(bounds) = control.bounds {
                    let r = rect_to_pixels(bounds, scale);
                    let _ = MoveWindow(
                        child,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        true,
                    );
                } else {
                    SendMessageW(
                        child,
                        TTM_SETMAXTIPWIDTH,
                        None,
                        Some(LPARAM(scale.pixels(300) as isize)),
                    );
                }
                let font = WPARAM(draw::font_for_dpi(control.font, scale).0 as usize);
                SendMessageW(child, WM_SETFONT, Some(font), Some(LPARAM(1)));
                if !control.columns.is_empty() {
                    let header = HWND(SendMessageW(child, LVM_GETHEADER, None, None).0 as *mut _);
                    SendMessageW(header, WM_SETFONT, Some(font), Some(LPARAM(1)));
                    for (index, width) in control.columns {
                        SendMessageW(
                            child,
                            LVM_SETCOLUMNWIDTH,
                            Some(WPARAM(index as usize)),
                            Some(LPARAM(scale.pixels(width) as isize)),
                        );
                    }
                }
            }
            apply_suggested(h, l);
            let _ = RedrawWindow(
                Some(h),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
            );
            return LRESULT(0);
        }
        if m == WM_NCDESTROY {
            WINDOWS.with(|windows| windows.borrow_mut().remove(&(h.0 as usize)));
            let _ = RemoveWindowSubclass(h, Some(window_proc), SUBCLASS);
        }
        DefSubclassProc(h, m, w, l)
    })
}

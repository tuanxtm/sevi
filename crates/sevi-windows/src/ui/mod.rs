//! Native Win32 windows and controls, retaining the original layout and copy.
pub mod controls;
pub mod dpi;
pub mod draw;
pub mod guide;
pub mod info;
pub mod settings;
use crate::{host, prefs::RegistryPreferences, wide};
use sevi_runtime::prefs::Preferences;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
pub const WM_REBUILD: u32 = WM_APP + 20;
pub fn callback(f: impl FnOnce() -> LRESULT) -> LRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(LRESULT(0))
}
pub fn open_uri(uri: &str) {
    let uri = wide(uri);
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(uri.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}
pub fn hotkey() -> u32 {
    let h = RegistryPreferences.int("SwitchKeyStatus", 0) as u32;
    if h == 0 { 0x7a000206 } else { h }
}
pub fn hotkey_label(h: u32) -> String {
    let mut out = String::new();
    for (mask, label) in [
        (0x100, "Ctrl+"),
        (0x200, "Alt+"),
        (0x800, "Shift+"),
        (0x400, "Win+"),
    ] {
        if h & mask != 0 {
            out.push_str(label)
        }
    }
    let k = h & 255;
    if k == 254 {
        out.pop();
    } else {
        match k {
            49 => out.push_str("Space"),
            36 => out.push_str("Enter"),
            48 => out.push_str("Tab"),
            _ => out.push(
                char::from_u32(sevi_engine::data::key_character(
                    k | sevi_engine::keys::CAPS_MASK,
                ))
                .filter(|c| *c != '\0')
                .unwrap_or('?'),
            ),
        }
    }
    if out.is_empty() { "—".into() } else { out }
}
pub fn title(hwnd: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut units = vec![0; n + 1];
        let got = GetWindowTextW(hwnd, &mut units).max(0) as usize;
        String::from_utf16_lossy(&units[..got])
    }
}
pub fn set_title(hwnd: HWND, text: &str) {
    let t = wide(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(t.as_ptr()));
    }
}
pub fn invalidate(hwnd: HWND) {
    if !hwnd.is_invalid() {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}
pub fn register(name: &str, proc: WNDPROC, background: Option<HBRUSH>) {
    let class_style = if name == "SeviEmoji" {
        CS_DROPSHADOW
    } else {
        WNDCLASS_STYLES(0)
    };
    let name = wide(name);
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let wc = WNDCLASSW {
            style: class_style,
            lpfnWndProc: proc,
            hInstance: instance,
            lpszClassName: PCWSTR(name.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: LoadIconW(Some(instance), PCWSTR(101usize as *const u16)).unwrap_or_default(),
            hbrBackground: background.unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    }
}
#[allow(clippy::too_many_arguments)] // Keeps CreateWindow's native placement and style explicit.
pub fn child(
    parent: HWND,
    class: &str,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
    font: draw::Font,
) -> HWND {
    let class = wide(class);
    let text = wide(text);
    let bounds = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let physical = dpi::rect_to_pixels(bounds, dpi::window(parent));
    unsafe {
        let child = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            physical.left,
            physical.top,
            physical.right - physical.left,
            physical.bottom - physical.top,
            Some(parent),
            Some(HMENU(id as usize as *mut _)),
            None,
            None,
        )
        .unwrap_or_default();
        dpi::track_control(parent, child, Some(bounds), font);
        child
    }
}
pub fn make_window(class: &str, title: &str, width: i32, height: i32, resize: bool) -> HWND {
    let class = wide(class);
    let title = wide(title);
    let style = if resize {
        WS_OVERLAPPEDWINDOW
    } else {
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
    };
    unsafe {
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let mut monitor = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(
            MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        // Create on the intended monitor before asking Windows for this window's DPI.
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            style | WS_CLIPCHILDREN,
            monitor.rcWork.left,
            monitor.rcWork.top,
            1,
            1,
            None,
            None,
            None,
            None,
        )
        .unwrap_or_default();
        if !window.is_invalid() {
            dpi::track_window(window);
            dpi::set_client_size(window, width, height);
        }
        window
    }
}
pub fn center(hwnd: HWND) {
    unsafe {
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let mut monitor = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(
            MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let work = monitor.rcWork;
        let x = ((work.left + work.right - (rect.right - rect.left)) / 2).max(work.left);
        let y = ((work.top + work.bottom - (rect.bottom - rect.top)) / 2).max(work.top);
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}
pub fn bring_front(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}
pub fn destroy_children(hwnd: HWND) {
    dpi::forget_children(hwnd);
    unsafe {
        let mut list = vec![];
        let mut child = GetWindow(hwnd, GW_CHILD).unwrap_or_default();
        while !child.is_invalid() {
            list.push(child);
            child = GetWindow(child, GW_HWNDNEXT).unwrap_or_default();
        }
        for child in list {
            let _ = DestroyWindow(child);
        }
    }
}
pub fn language_changed() {
    settings::language_changed();
    guide::language_changed();
    crate::emoji::language_changed();
}
pub fn changed_hotkey() {
    host::reload_settings();
    settings::schedule_rebuild();
    guide::schedule_rebuild();
}

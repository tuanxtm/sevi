use super::{
    draw::{self, Font},
    *,
};
use crate::{host, keys};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
const TOGGLE: &str = "SeviToggle";
const BUTTON: &str = "SeviButton";
const SEGMENT: &str = "SeviSegmented";
const RECORDER: &str = "SeviHotkey";
pub fn init() {
    for (name, proc) in [
        (
            TOGGLE,
            Some(toggle_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT),
        ),
        (BUTTON, Some(button_proc)),
        (SEGMENT, Some(segment_proc)),
        (RECORDER, Some(recorder_proc)),
        ("SeviTip", Some(tip_proc)),
    ] {
        register(name, proc, None);
    }
}
pub fn background(hwnd: HWND, color: u32) {
    unsafe {
        let _ = SetPropW(
            hwnd,
            w!("vkbg"),
            Some(HANDLE((color as usize + 1) as *mut _)),
        );
    }
}
fn background_color(hwnd: HWND) -> u32 {
    let value = unsafe { GetPropW(hwnd, w!("vkbg")) }.0 as usize;
    if value == 0 {
        draw::GROUP
    } else {
        value as u32 - 1
    }
}
pub fn checked(hwnd: HWND) -> bool {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) != 0 }
}
pub fn set_checked(hwnd: HWND, on: bool) {
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, isize::from(on));
    }
    invalidate(hwnd);
}
pub fn toggle(parent: HWND, x: i32, y: i32, id: i32, on: bool) -> HWND {
    let h = child(parent, TOGGLE, "", WS_TABSTOP, x, y, 40, 23, id, Font::Body);
    set_checked(h, on);
    h
}
#[allow(clippy::too_many_arguments)] // Mirrors the native control's placement and style.
pub fn button(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
    prominent: bool,
    bg: u32,
) -> HWND {
    let h = child(
        parent,
        BUTTON,
        text,
        WS_TABSTOP,
        x,
        y,
        width,
        height,
        id,
        Font::Body,
    );
    set_checked(h, prominent);
    background(h, bg);
    h
}
pub fn segment(parent: HWND, x: i32, y: i32, width: i32, id: i32) -> HWND {
    child(
        parent,
        SEGMENT,
        "",
        WINDOW_STYLE(0),
        x,
        y,
        width,
        30,
        id,
        Font::Body,
    )
}
fn notify(hwnd: HWND) {
    unsafe {
        let parent = GetParent(hwnd).unwrap_or_default();
        SendMessageW(
            parent,
            WM_COMMAND,
            Some(WPARAM(GetDlgCtrlID(hwnd) as usize)),
            Some(LPARAM(hwnd.0 as isize)),
        );
    }
}
unsafe extern "system" fn toggle_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                let p = draw::Paint::begin(h, background_color(h));
                let on = checked(h);
                let enabled = IsWindowEnabled(h).as_bool();
                p.round(
                    0.5,
                    0.5,
                    39.0,
                    22.0,
                    11.0,
                    draw::rgb(
                        if on { draw::ACCENT } else { draw::OFF },
                        if on && !enabled { 110 } else { 255 },
                    ),
                    None,
                );
                let x = if on { 19.5 } else { 2.5 };
                p.ellipse(x, 3.0, 18.0, 18.0, 0x28000000);
                p.ellipse(x, 2.5, 18.0, 18.0, 0xffffffff);
                return LRESULT(0);
            }
            WM_LBUTTONUP | WM_KEYUP => {
                if m == WM_KEYUP && w.0 != VK_SPACE.0 as usize {
                    return DefWindowProcW(h, m, w, l);
                }
                if IsWindowEnabled(h).as_bool() {
                    set_checked(h, !checked(h));
                    notify(h)
                }
                return LRESULT(0);
            }
            WM_ENABLE => invalidate(h),
            WM_ERASEBKGND => return LRESULT(1),
            WM_SETCURSOR => {
                SetCursor(LoadCursorW(None, IDC_HAND).ok());
                return LRESULT(1);
            }
            WM_NCDESTROY => {
                let _ = RemovePropW(h, w!("vkbg"));
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
unsafe extern "system" fn segment_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                let p = draw::Paint::begin(h, draw::BG);
                let half = p.rect.right as f32 / 2.0;
                let index = GetWindowLongPtrW(h, GWLP_USERDATA);
                p.round(
                    0.5,
                    0.5,
                    p.rect.right as f32 - 1.0,
                    p.rect.bottom as f32 - 1.0,
                    8.0,
                    draw::rgb(draw::OFF, 255),
                    None,
                );
                p.round(
                    index as f32 * half + 2.0,
                    2.0,
                    half - 4.0,
                    p.rect.bottom as f32 - 4.0,
                    7.0,
                    draw::rgb(draw::ACCENT, 255),
                    None,
                );
                for (i, text) in ["Tiếng Việt", "English"].iter().enumerate() {
                    p.text(
                        text,
                        Font::Body,
                        if index == i as isize {
                            draw::GROUP
                        } else {
                            draw::TEXT
                        },
                        RECT {
                            left: (i as f32 * half) as i32,
                            top: 0,
                            right: ((i + 1) as f32 * half) as i32,
                            bottom: p.rect.bottom,
                        },
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                }
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let mut rect = RECT::default();
                let _ = GetClientRect(h, &mut rect);
                let i = if (l.0 as u16 as i16 as i32) < rect.right / 2 {
                    0
                } else {
                    1
                };
                if i != GetWindowLongPtrW(h, GWLP_USERDATA) {
                    SetWindowLongPtrW(h, GWLP_USERDATA, i);
                    invalidate(h);
                    notify(h)
                }
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_SETCURSOR => {
                SetCursor(LoadCursorW(None, IDC_HAND).ok());
                return LRESULT(1);
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
unsafe extern "system" fn button_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                let p = draw::Paint::begin(h, background_color(h));
                let prominent = checked(h);
                let enabled = IsWindowEnabled(h).as_bool();
                let down = !GetPropW(h, w!("vkdown")).is_invalid();
                let fill = if prominent {
                    draw::rgb(
                        draw::ACCENT,
                        if !enabled {
                            90
                        } else if down {
                            210
                        } else {
                            255
                        },
                    )
                } else {
                    draw::rgb(if down { 0x00dedada } else { 0x00ece8e8 }, 255)
                };
                p.round(
                    0.5,
                    0.5,
                    p.rect.right as f32 - 1.0,
                    p.rect.bottom as f32 - 1.0,
                    7.0,
                    fill,
                    if prominent {
                        None
                    } else {
                        Some(draw::rgb(0x00dad6d6, 255))
                    },
                );
                p.text(
                    &title(h),
                    Font::Body,
                    if prominent {
                        draw::GROUP
                    } else if enabled {
                        draw::TEXT
                    } else {
                        draw::TERTIARY
                    },
                    p.rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                let _ = SetPropW(h, w!("vkdown"), Some(HANDLE(std::ptr::dangling_mut())));
                SetCapture(h);
                invalidate(h);
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let down = !GetPropW(h, w!("vkdown")).is_invalid();
                let _ = RemovePropW(h, w!("vkdown"));
                let _ = ReleaseCapture();
                invalidate(h);
                let mut rect = RECT::default();
                let _ = GetClientRect(h, &mut rect);
                let x = l.0 as u16 as i16 as i32;
                let y = (l.0 >> 16) as u16 as i16 as i32;
                if down
                    && x >= 0
                    && x < rect.right
                    && y >= 0
                    && y < rect.bottom
                    && IsWindowEnabled(h).as_bool()
                {
                    notify(h)
                }
                return LRESULT(0);
            }
            WM_ENABLE => invalidate(h),
            WM_ERASEBKGND => return LRESULT(1),
            WM_NCDESTROY => {
                let _ = RemovePropW(h, w!("vkbg"));
                let _ = RemovePropW(h, w!("vkdown"));
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
struct Recorder {
    pref: &'static str,
    mods_only: bool,
    recording: bool,
    value: u32,
}
fn recorder_state(hwnd: HWND) -> Option<&'static mut Recorder> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Recorder;
    unsafe { ptr.as_mut() }
}
#[allow(clippy::too_many_arguments)] // Native placement plus the persisted recorder policy.
pub fn recorder(
    parent: HWND,
    pref: &'static str,
    mods_only: bool,
    value: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
    bg: u32,
) -> HWND {
    let h = child(
        parent,
        RECORDER,
        "",
        WS_TABSTOP,
        x,
        y,
        width,
        height,
        id,
        Font::Body,
    );
    background(h, bg);
    let state = Box::new(Recorder {
        pref,
        mods_only,
        recording: false,
        value,
    });
    unsafe {
        SetWindowLongPtrW(h, GWLP_USERDATA, Box::into_raw(state) as isize);
    }
    h
}
pub fn start_recording(hwnd: HWND) {
    if let Some(r) = recorder_state(hwnd) {
        let _ = unsafe { SetFocus(Some(hwnd)) };
        r.recording = true;
        invalidate(hwnd)
    }
}
fn commit(hwnd: HWND, value: u32) {
    let Some(r) = recorder_state(hwnd) else {
        return;
    };
    if !r.mods_only {
        if value & 0x700 == 0 || value & !0x8000 == hotkey() & !0x8000 {
            return;
        }
        r.value = value & !0x8000;
    } else {
        r.value = value | (hotkey() & 0x8000)
    }
    r.recording = false;
    let _ = RegistryPreferences.set_int(r.pref, r.value as i32);
    host::reload_settings();
    invalidate(hwnd);
    notify(hwnd);
    changed_hotkey();
}
unsafe extern "system" fn recorder_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                let p = draw::Paint::begin(h, background_color(h));
                let r = recorder_state(h);
                let recording = r.as_ref().is_some_and(|r| r.recording);
                p.round(
                    0.5,
                    0.5,
                    p.rect.right as f32 - 1.0,
                    p.rect.bottom as f32 - 1.0,
                    7.0,
                    draw::rgb(0x00e6e6e6, 255),
                    if recording {
                        Some(draw::rgb(draw::ACCENT, 255))
                    } else {
                        None
                    },
                );
                let text = if recording {
                    "Bấm phím tắt…".into()
                } else if let Some(r) = r {
                    if r.value == 0 {
                        "Chưa đặt".into()
                    } else {
                        hotkey_label(r.value)
                    }
                } else {
                    String::new()
                };
                p.text(
                    &text,
                    Font::Body,
                    if recording {
                        draw::SECONDARY
                    } else {
                        draw::TEXT
                    },
                    p.rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                if let Some(r) = recorder_state(h) {
                    if r.recording {
                        r.recording = false
                    } else {
                        let _ = SetFocus(Some(h));
                        r.recording = true
                    }
                    invalidate(h)
                }
                return LRESULT(0);
            }
            WM_KILLFOCUS => {
                if let Some(r) = recorder_state(h) {
                    r.recording = false
                }
                invalidate(h);
                return LRESULT(0);
            }
            WM_GETDLGCODE => return LRESULT(DLGC_WANTALLKEYS as isize),
            WM_CHAR | WM_SYSCHAR => return LRESULT(0),
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if !recorder_state(h).is_some_and(|r| r.recording) {
                    return LRESULT(0);
                }
                let vk = w.0 as u32;
                let flags = host::modifiers().0;
                let mods = ((flags & 2) << 7)
                    | ((flags & 4) << 7)
                    | ((flags & 8) << 7)
                    | ((flags & 1) << 11);
                if vk == VK_ESCAPE.0 as u32 && mods == 0 {
                    if let Some(r) = recorder_state(h) {
                        r.recording = false
                    }
                    invalidate(h);
                    return LRESULT(0);
                }
                if keys::modifier(vk).is_none()
                    && mods != 0
                    && let Some(key) = keys::engine_key(vk)
                {
                    commit(h, key | mods)
                }
                return LRESULT(0);
            }
            WM_KEYUP | WM_SYSKEYUP => {
                if !recorder_state(h).is_some_and(|r| r.recording && r.mods_only) {
                    return LRESULT(0);
                }
                if let Some(bit) = keys::modifier(w.0 as u32) {
                    let flags = host::modifiers().0 | bit.0;
                    let mods = ((flags & 2) << 7)
                        | ((flags & 4) << 7)
                        | ((flags & 8) << 7)
                        | ((flags & 1) << 11);
                    if mods.count_ones() >= 2 {
                        commit(h, 0xfe | mods)
                    }
                }
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Recorder;
                if !ptr.is_null() {
                    SetWindowLongPtrW(h, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                let _ = RemovePropW(h, w!("vkbg"));
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
unsafe extern "system" fn tip_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                let p = draw::Paint::begin(h, draw::GROUP);
                p.round(
                    1.5,
                    1.5,
                    13.0,
                    13.0,
                    6.5,
                    0,
                    Some(draw::rgb(draw::TERTIARY, 255)),
                );
                p.text(
                    "?",
                    Font::Small,
                    draw::TERTIARY,
                    RECT {
                        left: 0,
                        top: -1,
                        right: 16,
                        bottom: 16,
                    },
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let tip = wide(&title(h));
                MessageBoxW(
                    GetParent(h).ok(),
                    PCWSTR(tip.as_ptr()),
                    w!("Sevi"),
                    MB_OK | MB_ICONINFORMATION,
                );
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}

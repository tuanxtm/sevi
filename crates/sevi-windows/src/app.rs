//! Tray icon, menu, singleton handoff, and the Win32 message loop.
use crate::{
    host,
    prefs::{self, RegistryPreferences},
    shared_ui, site, ui, updater, wide,
};
use sevi_engine::Language;
use sevi_runtime::prefs::Preferences;
use std::cell::RefCell;
use windows::{
    Win32::{
        Foundation::*,
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{CreateMutexW, ReleaseMutex},
        },
        UI::{Controls::*, HiDpi::*, Shell::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
const WM_TRAY: u32 = WM_APP + 1;
const WM_SET_LANGUAGE: u32 = WM_APP + 2;
const VIET: usize = 100;
const SETTINGS: usize = 102;
const AUTO: usize = 103;
const UPDATE: usize = 104;
const ABOUT: usize = 107;
const QUIT: usize = 108;
const GUIDE: usize = 110;
struct App {
    window: HWND,
    notification: NOTIFYICONDATAW,
    vi: HICON,
    en: HICON,
    taskbar_created: u32,
}
thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};}
fn state<T: Default>(f: impl FnOnce(&mut App) -> T) -> T {
    APP.with(|a| {
        a.try_borrow_mut()
            .ok()
            .and_then(|mut a| a.as_mut().map(f))
            .unwrap_or_default()
    })
}
// These IDs match IDI_TRAY_VI/IDI_TRAY_EN in Windows/App/resource.h.
fn tray_icons(instance: HINSTANCE) -> windows::core::Result<(HICON, HICON)> {
    unsafe {
        let vi = LoadIconMetric(Some(instance), PCWSTR(102usize as *const u16), LIM_SMALL)?;
        match LoadIconMetric(Some(instance), PCWSTR(103usize as *const u16), LIM_SMALL) {
            Ok(en) => Ok((vi, en)),
            Err(error) => {
                let _ = DestroyIcon(vi);
                Err(error)
            }
        }
    }
}
fn update_tray(language: Language) {
    state(|a| {
        a.notification.hIcon = if language == Language::Vietnamese {
            a.vi
        } else {
            a.en
        };
        a.notification.szTip.fill(0);
        let tip = wide(if language == Language::Vietnamese {
            "Sevi – Vietnamese"
        } else {
            "Sevi – English"
        });
        let len = tip.len().min(a.notification.szTip.len());
        a.notification.szTip[..len].copy_from_slice(&tip[..len]);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &a.notification);
        }
    });
}
fn toggle(key: &str, fallback: i32) {
    let value = i32::from(RegistryPreferences.int(key, fallback) == 0);
    let _ = RegistryPreferences.set_int(key, value);
    host::reload_settings();
}
fn menu() {
    let window = state(|a| a.window);
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let add = |flags: MENU_ITEM_FLAGS, id: usize, text: &str| {
            let text = wide(text);
            let _ = AppendMenuW(menu, flags, id, PCWSTR(text.as_ptr()));
        };
        let hk = host::session(|s| s.hotkey);
        add(
            MF_STRING
                | if host::language() == Language::Vietnamese {
                    MF_CHECKED
                } else {
                    MENU_ITEM_FLAGS(0)
                },
            VIET,
            &format!("Vietnamese\t{}", ui::hotkey_label(hk)),
        );
        add(
            MF_STRING
                | if RegistryPreferences.int("AutoSwitchByTyping", 1) != 0 {
                    MF_CHECKED
                } else {
                    MENU_ITEM_FLAGS(0)
                },
            AUTO,
            "Detect English / Vietnamese",
        );
        add(MF_SEPARATOR, 0, "");
        add(MF_STRING, SETTINGS, "Open Sevi…");
        add(MF_SEPARATOR, 0, "");
        for (id, text) in [
            (GUIDE, "Help…"),
            (UPDATE, "Check for updates…"),
            (ABOUT, "About Sevi"),
        ] {
            add(MF_STRING, id, text);
        }
        add(MF_STRING, QUIT, "Quit Sevi");
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(window);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            Some(0),
            window,
            None,
        );
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    ui::callback(|| unsafe {
        if m != 0 && state(|a| a.taskbar_created == m) {
            state(|a| {
                let _ = Shell_NotifyIconW(NIM_ADD, &a.notification);
            });
            return LRESULT(0);
        }
        match m {
            WM_TRAY => {
                match (l.0 & 65535) as u32 {
                    WM_LBUTTONUP => shared_ui::open(sevi_ui_protocol::View::Settings),
                    WM_RBUTTONUP | WM_CONTEXTMENU => menu(),
                    _ => (),
                }
                return LRESULT(0);
            }
            WM_COMMAND => {
                match w.0 & 65535 {
                    VIET => host::set_language(host::language().toggled(), true),
                    AUTO => toggle("AutoSwitchByTyping", 1),
                    SETTINGS => shared_ui::open(sevi_ui_protocol::View::Settings),
                    UPDATE => updater::check_now(h),
                    ABOUT => shared_ui::open(sevi_ui_protocol::View::About),
                    GUIDE => shared_ui::open(sevi_ui_protocol::View::Help),
                    QUIT => {
                        updater::install_pending(false);
                        let _ = DestroyWindow(h);
                    }
                    _ => (),
                }
                return LRESULT(0);
            }
            WM_SET_LANGUAGE => {
                host::set_language(
                    if w.0 == 0 {
                        Language::English
                    } else {
                        Language::Vietnamese
                    },
                    false,
                );
                return LRESULT(1);
            }
            host::WM_LANGUAGE => {
                update_tray(if w.0 == 0 {
                    Language::English
                } else {
                    Language::Vietnamese
                });
                shared_ui::language(w.0 != 0);
                return LRESULT(0);
            }
            host::WM_EMOJI => {
                shared_ui::open(sevi_ui_protocol::View::Emoji);
                return LRESULT(0);
            }
            updater::WM_UPDATER => {
                updater::on_event(w.0);
                return LRESULT(0);
            }
            site::WM_SITE => {
                let (window, host) = site::last_host();
                if window == GetForegroundWindow() {
                    host::site_changed(&host)
                }
                return LRESULT(0);
            }
            host::WM_FLUSH_LOG => {
                host::poll_log();
                return LRESULT(0);
            }
            WM_TIMER if w.0 == 1 => {
                host::poll_log();
                return LRESULT(0);
            }
            shared_ui::WM_UI => {
                shared_ui::tick();
                return LRESULT(0);
            }
            WM_TIMER if w.0 == 2 => {
                shared_ui::tick();
                return LRESULT(0);
            }
            WM_SETTINGCHANGE | WM_DISPLAYCHANGE | WM_DPICHANGED => {
                // Refresh system-metric sizes after display/settings changes.
                // Keep the existing pair if loading either replacement fails.
                let previous = state(|a| {
                    let instance = HINSTANCE(GetModuleHandleW(None).ok()?.0);
                    let (vi, en) = tray_icons(instance).ok()?;
                    Some((
                        std::mem::replace(&mut a.vi, vi),
                        std::mem::replace(&mut a.en, en),
                    ))
                });
                if let Some((vi, en)) = previous {
                    update_tray(host::language());
                    let _ = DestroyIcon(vi);
                    let _ = DestroyIcon(en);
                }
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = DestroyWindow(h);
                return LRESULT(0);
            }
            WM_DESTROY => {
                shared_ui::shutdown();
                host::stop();
                state(|a| {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &a.notification);
                });
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ if m == WM_APP + 3 => {
                shared_ui::open(sevi_ui_protocol::View::Settings);
                return LRESULT(0);
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
struct Singleton(HANDLE);
impl Drop for Singleton {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
pub fn run() -> windows::core::Result<()> {
    unsafe {
        let singleton = Singleton(CreateMutexW(None, true, w!("Local\\SeviRunning"))?);
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(window) = FindWindowW(w!("SeviTray"), None) {
                let mut pid = 0;
                GetWindowThreadProcessId(window, Some(&mut pid));
                let _ = AllowSetForegroundWindow(pid);
                let _ = PostMessageW(Some(window), WM_APP + 3, WPARAM(0), LPARAM(0));
            }
            return Ok(());
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if updater::install_pending(true) {
            return Ok(());
        }
        let _ = prefs::migrate_from_open_key();
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_LISTVIEW_CLASSES | ICC_LINK_CLASS | ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&controls);
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: w!("SeviTray"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("SeviTray"),
            w!("Sevi"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )?;
        let (vi, en) = tray_icons(instance)?;
        let mut notification = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: vi,
            ..Default::default()
        };
        let tip = wide("Sevi");
        notification.szTip[..tip.len()].copy_from_slice(&tip);
        let _ = Shell_NotifyIconW(NIM_ADD, &notification);
        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                window,
                notification,
                vi,
                en,
                taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            })
        });
        if let Err(e) = host::start(window) {
            MessageBoxW(
                None,
                w!("Could not install the keyboard hook (SetWindowsHookEx)."),
                w!("Sevi"),
                MB_ICONERROR,
            );
            return Err(e);
        }
        update_tray(host::language());
        shared_ui::initialize(window);
        if RegistryPreferences.get("FirstRunDone").is_none() {
            let _ = RegistryPreferences.set_int("FirstRunDone", 1);
            shared_ui::open(sevi_ui_protocol::View::Settings);
        }
        SetTimer(Some(window), 1, 10000, None);
        updater::start(window);
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0).0;
            if status <= 0 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        APP.with(|a| {
            if let Some(a) = a.borrow_mut().take() {
                let _ = DestroyIcon(a.vi);
                let _ = DestroyIcon(a.en);
            }
        });
        drop(singleton);
        Ok(())
    }
}

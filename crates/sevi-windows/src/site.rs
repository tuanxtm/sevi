//! UI Automation stays on two independent STA workers, away from keyboard hooks.
use crate::{
    browser::{self, FocusGuard, FocusKey},
    prefs::RegistryPreferences,
};
use sevi_runtime::prefs::Preferences;
use std::{
    ptr,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            Ole::{
                SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
                SafeArrayUnaccessData,
            },
            Threading::{
                CreateEventW, GetCurrentProcessId, OpenProcess, PROCESS_NAME_FORMAT,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, SetEvent,
            },
            Variant::VARIANT,
        },
        UI::{Accessibility::*, WindowsAndMessaging::*},
    },
    core::Interface,
};
pub const WM_SITE: u32 = WM_APP + 6;
static FOCUS: LazyLock<FocusGuard> = LazyLock::new(FocusGuard::default);
static WAKE: AtomicIsize = AtomicIsize::new(0);
static RUNNING: AtomicBool = AtomicBool::new(false);
static HOST: Mutex<(isize, String)> = Mutex::new((0, String::new()));
pub fn secure() -> bool {
    FOCUS.secure()
}
pub fn poll_password(window: HWND, object: i32, child: i32) {
    FOCUS.event(FocusKey {
        window: window.0 as isize,
        object,
        child,
    });
    let wake = WAKE.load(Ordering::Relaxed);
    if wake != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(wake as *mut _));
        }
    }
}
pub fn front_exe(window: HWND) -> String {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut path = [0u16; 260];
        let mut n = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(path.as_mut_ptr()),
            &mut n,
        );
        let _ = CloseHandle(process);
        if result.is_err() {
            return String::new();
        }
        String::from_utf16_lossy(&path[..n as usize])
            .rsplit('\\')
            .next()
            .unwrap_or("")
            .to_owned()
    }
}
fn own_window(window: HWND) -> bool {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        pid == GetCurrentProcessId() || crate::host::own_ui(window)
    }
}
struct Apartment;
impl Apartment {
    fn new() -> windows::core::Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
fn client() -> Option<IUIAutomation> {
    unsafe {
        if let Ok(client) =
            CoCreateInstance::<_, IUIAutomation2>(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
        {
            let _ = client.SetConnectionTimeout(1000);
            let _ = client.SetTransactionTimeout(2000);
            return client.cast().ok();
        }
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    }
}
fn wait(wake: Option<HANDLE>, ms: u32) {
    let end = Instant::now() + Duration::from_millis(ms as u64);
    loop {
        let left = end.saturating_duration_since(Instant::now()).as_millis() as u32;
        let handles = wake.map(|h| [h]);
        unsafe {
            let r = MsgWaitForMultipleObjects(
                handles.as_ref().map(|h| h.as_slice()),
                false,
                left,
                QS_ALLINPUT,
            );
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&message);
            }
            if (wake.is_some() && r.0 == 0) || Instant::now() >= end || r.0 == u32::MAX {
                return;
            }
        }
    }
}
fn runtime_id(element: &IUIAutomationElement) -> Vec<i32> {
    unsafe {
        let Ok(array) = element.GetRuntimeId() else {
            return vec![];
        };
        if array.is_null() {
            return vec![];
        }
        let result = (|| {
            let lo = SafeArrayGetLBound(array, 1).ok()?;
            let hi = SafeArrayGetUBound(array, 1).ok()?;
            let len = hi.checked_sub(lo)?.checked_add(1)? as usize;
            if len == 0 {
                return Some(vec![]);
            }
            let mut data = ptr::null_mut();
            SafeArrayAccessData(array, &mut data).ok()?;
            let id = if data.is_null() {
                vec![]
            } else {
                std::slice::from_raw_parts(data as *const i32, len).to_vec()
            };
            let _ = SafeArrayUnaccessData(array);
            Some(id)
        })();
        let _ = SafeArrayDestroy(array);
        result.unwrap_or_default()
    }
}
fn current_host(client: &IUIAutomation, window: HWND) -> String {
    if !browser::is_browser(&front_exe(window)) {
        return String::new();
    }
    let result = unsafe {
        (move || -> windows::core::Result<String> {
            let root = client.ElementFromHandle(window)?;
            let condition = client.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_EditControlTypeId.0),
            )?;
            let elements = root.FindAll(TreeScope_Descendants, &condition)?;
            let count = elements.Length()?;
            if count >= 200 {
                return Ok(String::new());
            }
            for i in 0..count {
                let Ok(element) = elements.GetElement(i) else {
                    continue;
                };
                let Ok(pattern) =
                    element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                else {
                    continue;
                };
                if let Ok(value) = pattern.CurrentValue() {
                    let value = value.to_string();
                    if browser::looks_like_url(&value) {
                        return Ok(browser::host_of(&value));
                    }
                }
            }
            Ok(String::new())
        })()
    };
    result.unwrap_or_default()
}
fn password_worker() {
    let Ok(_apartment) = Apartment::new() else {
        return;
    };
    let mut client_cache = None;
    let (mut plain_key, mut plain_id) = (FocusKey::default(), Vec::<i32>::new());
    while RUNNING.load(Ordering::Relaxed) {
        let wake = WAKE.load(Ordering::Relaxed);
        wait(
            if wake == 0 {
                None
            } else {
                Some(HANDLE(wake as *mut _))
            },
            1000,
        );
        if !RUNNING.load(Ordering::Relaxed) {
            break;
        }
        let seen = FOCUS.seen();
        if client_cache.is_none() {
            client_cache = client()
        }
        let Some(client) = &client_cache else {
            FOCUS.publish(seen, browser::ASSUMED_PLAIN);
            continue;
        };
        if own_window(unsafe { GetForegroundWindow() }) {
            FOCUS.publish(seen, browser::ASSUMED_PLAIN);
            continue;
        }
        let Ok(element) = (unsafe { client.GetFocusedElement() }) else {
            FOCUS.publish(seen, browser::ASSUMED_PLAIN);
            continue;
        };
        let key = FOCUS.key();
        let password = unsafe { element.CurrentIsPassword() }
            .map(|p| p.as_bool())
            .unwrap_or(false);
        let id = runtime_id(&element);
        if (FOCUS.seen() ^ seen) & !3 == 0 && !id.is_empty() {
            if key == plain_key && !plain_id.is_empty() && id != plain_id {
                FOCUS.ambiguous(seen, key)
            }
            if !password {
                plain_key = key;
                plain_id = id;
            }
        }
        FOCUS.publish(
            seen,
            if password {
                browser::PASSWORD
            } else {
                browser::PLAIN
            },
        );
    }
}
fn site_worker(owner: isize) {
    let Ok(_apartment) = Apartment::new() else {
        return;
    };
    let mut client_cache = None;
    while RUNNING.load(Ordering::Relaxed) {
        wait(None, 1000);
        if !RUNNING.load(Ordering::Relaxed) {
            break;
        }
        if client_cache.is_none() {
            client_cache = client()
        }
        let Some(client) = &client_cache else {
            continue;
        };
        let window = unsafe { GetForegroundWindow() };
        if own_window(window) {
            continue;
        }
        let host = if RegistryPreferences.int("SmartSwitchPerSite", 1) != 0 {
            current_host(client, window)
        } else {
            String::new()
        };
        *HOST.lock().unwrap() = (window.0 as isize, host);
        unsafe {
            let _ = PostMessageW(
                Some(HWND(owner as *mut _)),
                WM_SITE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }
    }
}
pub fn last_host() -> (HWND, String) {
    let host = HOST.lock().unwrap();
    (HWND(host.0 as *mut _), host.1.clone())
}
pub fn start(owner: HWND) -> windows::core::Result<()> {
    if RUNNING.swap(true, Ordering::Relaxed) {
        return Ok(());
    }
    let event = match unsafe { CreateEventW(None, false, true, None) } {
        Ok(event) => event,
        Err(e) => {
            RUNNING.store(false, Ordering::Relaxed);
            return Err(e);
        }
    };
    WAKE.store(event.0 as isize, Ordering::Relaxed);
    std::thread::spawn(password_worker);
    let owner = owner.0 as isize;
    std::thread::spawn(move || site_worker(owner));
    Ok(())
}
pub fn stop() {
    RUNNING.store(false, Ordering::Relaxed);
    let event = WAKE.load(Ordering::Relaxed);
    if event != 0 {
        unsafe {
            let _ = SetEvent(HANDLE(event as *mut _));
        }
    }
}

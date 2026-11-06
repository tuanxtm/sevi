//! Win32 hooks and SendInput output. Slow UI work is posted to the owner window.
use crate::{
    insertion::{self, Outcome},
    keys,
    prefs::{self, RegistryPreferences},
    site,
    storage::{BackgroundPreferences, Tasks},
};
use sevi_engine::Language;
use sevi_runtime::{DesktopSession, Effect, EventResult, Modifiers, Platform};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{cell::RefCell, fs, io::Write, ptr, sync::OnceLock, time::Instant};
static UI_PROCESS: AtomicU32 = AtomicU32::new(0);
pub fn set_ui_process(pid: u32) {
    UI_PROCESS.store(pid, Ordering::Relaxed);
}
pub fn own_ui(window: HWND) -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    pid != 0 && pid == UI_PROCESS.load(Ordering::Relaxed)
}
pub fn apply_ui_setting(setting: sevi_ui_protocol::Setting, value: i32) -> std::io::Result<()> {
    with_host(|h| {
        Some((|| {
            sevi_runtime::ui::set(&mut *h.session.preferences, setting, value)?;
            let result = h.session.apply_cached_settings(now());
            h.effects(result);
            Ok(())
        })())
    })
    .unwrap_or_else(|| Err(std::io::Error::other("IME host is unavailable")))
}
use windows::{
    Win32::{
        Foundation::*,
        Media::Audio::*,
        System::{Diagnostics::Debug::MessageBeep, LibraryLoader::GetModuleHandleW},
        UI::{Accessibility::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::PCWSTR,
};
pub const WM_LANGUAGE: u32 = WM_APP + 30;
pub const WM_FLUSH_LOG: u32 = WM_APP + 31;
pub const WM_EMOJI: u32 = WM_APP + 5;
pub const WM_EMOJI_KEY: u32 = WM_APP + 10;
pub const WM_EMOJI_CLOSE: u32 = WM_APP + 11;
pub const WM_EMOJI_PACKET: u32 = WM_APP + 12;
const EVENT_TAG: usize = 0x0f341991;
struct Host {
    session: DesktopSession,
    owner: HWND,
    picker: HWND,
    keyboard: HHOOK,
    mouse: HHOOK,
    foreground: HWINEVENTHOOK,
    focus: HWINEVENTHOOK,
    current_key: Option<(u16, bool)>,
    last_foreground: HWND,
    storage: Tasks,
    log_flush_posted: bool,
}
impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            if !self.keyboard.is_invalid() {
                let _ = UnhookWindowsHookEx(self.keyboard);
            }
            if !self.mouse.is_invalid() {
                let _ = UnhookWindowsHookEx(self.mouse);
            }
            if !self.foreground.is_invalid() {
                let _ = UnhookWinEvent(self.foreground);
            }
            if !self.focus.is_invalid() {
                let _ = UnhookWinEvent(self.focus);
            }
        }
    }
}
thread_local! {static HOST:RefCell<Option<Host>>=const{RefCell::new(None)};}
fn with_host<T: Default>(f: impl FnOnce(&mut Host) -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        HOST.with(|h| {
            h.try_borrow_mut()
                .ok()
                .and_then(|mut h| h.as_mut().map(f))
                .unwrap_or_default()
        })
    }))
    .unwrap_or_default()
}
pub fn now() -> f64 {
    static CLOCK: OnceLock<Instant> = OnceLock::new();
    CLOCK.get_or_init(Instant::now).elapsed().as_secs_f64()
}
pub fn session<T: Default>(f: impl FnOnce(&mut DesktopSession) -> T) -> T {
    with_host(|h| f(&mut h.session))
}
pub fn language() -> Language {
    session(|s| s.language())
}
pub fn set_language(language: Language, user: bool) {
    with_host(|h| {
        let result = if user {
            h.session.set_language_user(language, now(), "menu", false)
        } else {
            h.session.set_language_quiet(language)
        };
        h.effects(result);
    });
}
pub fn reload_settings() {
    with_host(|h| {
        let r = h.session.reload_settings(now());
        h.effects(r);
    });
}
pub fn set_picker(picker: HWND) {
    with_host(|h| {
        h.picker = picker;
        h.session.emoji_open = !picker.is_invalid();
    });
}
pub fn reset_typing() {
    session(DesktopSession::reset_typing);
}
pub fn insert_text(text: &str) {
    with_host(|h| {
        let r = h.session.insert_text(text);
        h.effects(r);
    });
}
pub fn site_changed(host: &str) {
    with_host(|h| {
        let r = h.session.site_changed(host);
        h.effects(r);
    });
}
pub fn flush_log() {
    with_host(|h| {
        h.queue_log();
        let _ = h.session.preferences.flush();
    });
}
pub fn poll_log() {
    with_host(Host::queue_log);
}
pub fn modifiers() -> Modifiers {
    unsafe {
        Modifiers(
            u32::from(GetAsyncKeyState(VK_SHIFT.0 as i32) < 0)
                | (u32::from(GetAsyncKeyState(VK_CONTROL.0 as i32) < 0) * 2)
                | (u32::from(GetAsyncKeyState(VK_MENU.0 as i32) < 0) * 4)
                | (u32::from(
                    GetAsyncKeyState(VK_LWIN.0 as i32) < 0
                        || GetAsyncKeyState(VK_RWIN.0 as i32) < 0,
                ) * 8)
                | (u32::from(GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0) * 16),
        )
    }
}
fn input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: EVENT_TAG,
            },
        },
    }
}
fn key_pair(batch: &mut Vec<INPUT>, vk: u16) {
    batch.push(input(vk, 0, KEYBD_EVENT_FLAGS(0)));
    batch.push(input(vk, 0, KEYEVENTF_KEYUP));
}
fn text_batch(batch: &mut Vec<INPUT>, text: &str) {
    for ch in text.encode_utf16() {
        batch.push(input(0, ch, KEYEVENTF_UNICODE));
        batch.push(input(0, ch, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
}
impl Host {
    fn update_foreground(&mut self, window: HWND) {
        if self.last_foreground != window {
            self.last_foreground = window;
            if self.session.insertion_blocked {
                self.session.insertion_blocked = false;
                self.session.reset_typing();
            }
        }
    }
    fn effects(&mut self, result: EventResult) -> Outcome {
        let mut batch = vec![];
        for effect in result.effects {
            match effect {
                Effect::MacRewrite { .. } => (),
                Effect::Backspaces(n) => {
                    for _ in 0..n {
                        key_pair(&mut batch, VK_BACK.0)
                    }
                }
                Effect::Text(s) => text_batch(&mut batch, &s),
                Effect::ControlKey { key, .. } => {
                    if let Some((vk, extended)) = keys::forwarded_key(self.current_key, key) {
                        // Physical modifiers are still held; synthesizing Shift-up releases them.
                        let flags = if extended {
                            KEYEVENTF_EXTENDEDKEY
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        };
                        batch.push(input(vk, 0, flags));
                        batch.push(input(vk, 0, flags | KEYEVENTF_KEYUP));
                    }
                }
                Effect::LanguageChanged(language) => unsafe {
                    let _ = PostMessageW(
                        Some(self.owner),
                        WM_LANGUAGE,
                        WPARAM(language as usize),
                        LPARAM(0),
                    );
                },
                Effect::OpenEmoji => unsafe {
                    let _ = PostMessageW(Some(self.owner), WM_EMOJI, WPARAM(0), LPARAM(0));
                },
                Effect::SwitchSound(language) => {
                    let bytes: &'static [u8] = if language == Language::Vietnamese {
                        include_bytes!("../../../Windows/App/Sounds/SwitchToVietnamese.wav")
                    } else {
                        include_bytes!("../../../Windows/App/Sounds/SwitchToEnglish.wav")
                    };
                    unsafe {
                        if !PlaySoundW(
                            PCWSTR(bytes.as_ptr().cast()),
                            None,
                            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
                        )
                        .as_bool()
                        {
                            let _ = MessageBeep(MB_OK);
                        }
                    }
                }
            }
        }
        let mut outcome = Outcome::NotNeeded;
        if !batch.is_empty() {
            self.update_foreground(unsafe { GetForegroundWindow() });
            outcome = if self.session.insertion_blocked {
                Outcome::Blocked
            } else {
                insertion::send_pairs(
                    &batch,
                    |events| unsafe { SendInput(events, size_of::<INPUT>() as i32) },
                    |event| {
                        let key = unsafe { event.Anonymous.ki };
                        input(key.wVk.0, key.wScan, key.dwFlags | KEYEVENTF_KEYUP)
                    },
                )
            };
            if outcome.failed() {
                self.session.discard_failed_output();
            }
        }
        // Constant-time check. Serialize and persist only after returning from the hook.
        if self.session.log.pending.len() >= 64 && !self.log_flush_posted {
            self.log_flush_posted = unsafe {
                PostMessageW(Some(self.owner), WM_FLUSH_LOG, WPARAM(0), LPARAM(0)).is_ok()
            };
        }
        outcome
    }
    fn queue_log(&mut self) {
        self.log_flush_posted = false;
        let events = std::mem::take(&mut self.session.log.pending);
        if events.is_empty() {
            return;
        }
        let _ = self.storage.run(move || write_log(events));
    }
    fn foreground(&mut self) {
        let window = unsafe { GetForegroundWindow() };
        self.update_foreground(window);
        let app = site::front_exe(window);
        let r = self
            .session
            .focus_changed(if app.is_empty() { "UnknownApp" } else { &app }, now());
        self.effects(r);
    }
}
fn write_log(events: Vec<serde_json::Value>) {
    let mut text = String::new();
    for event in events {
        if let Ok(line) = serde_json::to_string(&event) {
            text.push_str(&line);
            text.push('\n');
        }
    }
    let Ok(dir) = prefs::data_dir() else { return };
    let path = dir.join("typing-log.jsonl");
    if fs::metadata(&path).is_ok_and(|m| m.len() > 20 * 1024 * 1024) {
        let old = dir.join("typing-log.1.jsonl");
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
        let _ = fs::rename(&path, old);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(text.as_bytes());
    }
}
unsafe extern "system" fn keyboard(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 || lp.0 == 0 {
        return unsafe { CallNextHookEx(None, code, wp, lp) };
    }
    let event = unsafe { &*(lp.0 as *const KBDLLHOOKSTRUCT) };
    if event.dwExtraInfo == EVENT_TAG {
        return unsafe { CallNextHookEx(None, code, wp, lp) };
    }
    let down = wp.0 as u32 == WM_KEYDOWN || wp.0 as u32 == WM_SYSKEYDOWN;
    let consumed = with_host(|h| {
        let foreground = unsafe { GetForegroundWindow() };
        h.update_foreground(foreground);
        if own_ui(foreground) {
            h.session.synchronize_modifiers(modifiers());
            h.session.unmodeled_key(false);
            return false;
        }
        let flags = modifiers();
        h.session.synchronize_modifiers(flags);
        if let Some(bit) = keys::modifier(event.vkCode) {
            let r = h.session.modifier(bit, down, now());
            h.effects(r);
            return false;
        }
        if !down {
            return false;
        }
        let key = keys::engine_key(event.vkCode);
        if !h.picker.is_invalid() {
            let hk = h.session.emoji_hotkey;
            let hk_flags = ((hk & 0x100) >> 7)
                | ((hk & 0x200) >> 7)
                | ((hk & 0x400) >> 7)
                | ((hk & 0x800) >> 11);
            unsafe {
                if hk != 0 && key == Some(hk & 255) && flags.0 & 15 == hk_flags {
                    let _ = PostMessageW(Some(h.owner), WM_EMOJI, WPARAM(0), LPARAM(0));
                } else if event.vkCode == VK_PACKET.0 as u32 {
                    let _ = PostMessageW(
                        Some(h.picker),
                        WM_EMOJI_PACKET,
                        WPARAM(event.scanCode as usize),
                        LPARAM(0),
                    );
                } else {
                    let _ = PostMessageW(
                        Some(h.picker),
                        WM_EMOJI_KEY,
                        WPARAM(event.vkCode as usize),
                        LPARAM(flags.0 as isize),
                    );
                }
            }
            return true;
        }
        let Some(key) = key else {
            h.session
                .unmodeled_key(keys::unmodeled_ends_word(event.vkCode));
            return false;
        };
        h.current_key = Some((event.vkCode as u16, event.flags.contains(LLKHF_EXTENDED)));
        h.session.log.secure = site::secure();
        let thread = unsafe { GetWindowThreadProcessId(h.last_foreground, None) };
        let layout = unsafe { GetKeyboardLayout(thread) };
        let primary = layout.0 as usize & 0x3ff;
        h.session.other_language = ![0, 0x09, 0x2a].contains(&primary);
        let r = h.session.key_down(key, flags.shift(), flags, now());
        let consumed = r.consumed;
        let outcome = h.effects(r);
        h.current_key = None;
        outcome.consumed(consumed)
    });
    if consumed {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, code, wp, lp) }
    }
}
unsafe extern "system" fn mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && [WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MBUTTONDOWN].contains(&(wp.0 as u32))
    {
        with_host(|h| {
            if !h.picker.is_invalid() && lp.0 != 0 {
                unsafe {
                    let event = &*(lp.0 as *const MSLLHOOKSTRUCT);
                    let mut rect = RECT::default();
                    let _ = GetWindowRect(h.picker, &mut rect);
                    if event.pt.x < rect.left
                        || event.pt.x >= rect.right
                        || event.pt.y < rect.top
                        || event.pt.y >= rect.bottom
                    {
                        let _ = PostMessageW(Some(h.picker), WM_EMOJI_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
            }
            let r = h.session.click();
            h.effects(r);
        });
    }
    unsafe { CallNextHookEx(None, code, wp, lp) }
}
unsafe extern "system" fn focus(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        with_host(Host::foreground)
    } else {
        site::poll_password(window, object, child)
    }
}
pub fn start(owner: HWND) -> windows::core::Result<()> {
    let preferences = BackgroundPreferences::new(RegistryPreferences)
        .map_err(|e| windows::core::Error::new(E_FAIL, e.to_string()))?;
    let storage = preferences.tasks();
    let session = DesktopSession::new(Platform::Windows, Box::new(preferences));
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
    let keyboard = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), Some(instance), 0) }?;
    let mouse = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), Some(instance), 0) }
        .unwrap_or_default();
    let foreground = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(focus),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    let focus = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_FOCUS,
            EVENT_OBJECT_FOCUS,
            None,
            Some(focus),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    HOST.with(|h| {
        *h.borrow_mut() = Some(Host {
            session,
            owner,
            picker: HWND(ptr::null_mut()),
            keyboard,
            mouse,
            foreground,
            focus,
            current_key: None,
            last_foreground: HWND::default(),
            storage,
            log_flush_posted: false,
        })
    });
    with_host(Host::foreground);
    site::start(owner)?;
    Ok(())
}
pub fn stop() {
    flush_log();
    site::stop();
    HOST.with(|h| {
        h.borrow_mut().take();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sevi_runtime::prefs::MemoryPreferences;

    #[test]
    fn failed_insertion_stays_blocked_until_foreground_really_changes() {
        let window = HWND(ptr::without_provenance_mut(1));
        let preferences = BackgroundPreferences::new(MemoryPreferences::default()).unwrap();
        let mut host = Host {
            session: DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default()),
            owner: HWND::default(),
            picker: HWND::default(),
            keyboard: HHOOK::default(),
            mouse: HHOOK::default(),
            foreground: HWINEVENTHOOK::default(),
            focus: HWINEVENTHOOK::default(),
            current_key: None,
            last_foreground: window,
            storage: preferences.tasks(),
            log_flush_posted: false,
        };
        host.session.discard_failed_output();
        host.update_foreground(window);
        assert!(host.session.insertion_blocked);
        host.update_foreground(HWND(ptr::without_provenance_mut(2)));
        assert!(!host.session.insertion_blocked);
        host.update_foreground(window);
        assert!(!host.session.insertion_blocked);
    }
}

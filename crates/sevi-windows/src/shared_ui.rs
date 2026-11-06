//! The Win32 host owns settings and text insertion; egui runs in a private child.
use crate::{
    host,
    prefs::{self, RegistryPreferences},
    wide,
};
use sevi_runtime::prefs::Preferences;
use sevi_ui_protocol::{host::UiHost, *};
use std::{cell::RefCell, process::Command};
use windows::{
    Win32::{
        Foundation::*,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
pub const WM_UI: u32 = WM_APP + 32;
struct Bridge {
    owner: HWND,
    client: UiHost,
    target: HWND,
}
thread_local! {static BRIDGE:RefCell<Option<Bridge>>=const{RefCell::new(None)};}
fn with(f: impl FnOnce(&mut Bridge)) {
    BRIDGE.with(|b| {
        if let Ok(mut b) = b.try_borrow_mut()
            && let Some(b) = b.as_mut()
        {
            f(b);
        }
    });
}
pub fn initialize(owner: HWND) {
    BRIDGE.with(|b| {
        *b.borrow_mut() = Some(Bridge {
            owner,
            client: UiHost::default(),
            target: HWND::default(),
        })
    });
}
pub fn open(view: View) {
    with(|b| b.open(view));
}
pub fn tick() {
    with(Bridge::tick);
}
pub fn language(vi: bool) {
    with(|b| b.client.send(UiEvent::Language(vi)));
}
pub fn notice(message: &str) {
    with(|b| {
        b.open(View::About);
        b.client.send(UiEvent::Error(message.into()));
    });
}
pub fn offer(version: String, notes: String) {
    with(|b| {
        b.open(View::About);
        b.client.send(UiEvent::UpdateOffer { version, notes });
    });
}
pub fn shutdown() {
    with(|b| b.client.close());
}
impl Bridge {
    fn snapshot(&self) -> UiSnapshot {
        let mut s = host::session(|session| {
            let mut s = sevi_runtime::ui::snapshot(&*session.preferences, Platform::Windows);
            s.language_vi = session.language() == sevi_engine::Language::Vietnamese;
            s.settings
                .insert(Setting::SwitchKeyStatus, session.hotkey as i32);
            s.capabilities.updates = true;
            s
        });
        s.login = prefs::launch_at_login();
        s
    }
    fn open(&mut self, view: View) {
        if view != View::Emoji
            && let Some(pid) = self.client.pid()
        {
            unsafe {
                let _ = AllowSetForegroundWindow(pid);
            }
        }
        if view == View::Emoji {
            self.target = unsafe { GetForegroundWindow() };
        }
        let snapshot = self.snapshot();
        let owner = self.owner.0 as isize;
        let command = match std::env::current_exe() {
            Ok(p) => Command::new(p),
            Err(e) => {
                eprintln!("Sevi UI: {e}");
                return;
            }
        };
        if let Err(e) = self.client.open(command, view, snapshot, move || unsafe {
            let _ = PostMessageW(Some(HWND(owner as *mut _)), WM_UI, WPARAM(0), LPARAM(0));
        }) {
            eprintln!("Sevi UI: {e}");
        }
        host::set_ui_process(self.client.pid().unwrap_or(0));
        if view != View::Emoji
            && let Some(pid) = self.client.pid()
        {
            unsafe {
                let _ = AllowSetForegroundWindow(pid);
            }
        }
        unsafe {
            SetTimer(Some(self.owner), 2, 200, None);
        }
    }
    fn apply(&mut self, request: UiCommand) -> std::io::Result<()> {
        match request {
            UiCommand::Set { setting, value } => host::apply_ui_setting(setting, value)?,
            UiCommand::SetLanguage(vi) => host::set_language(
                if vi {
                    sevi_engine::Language::Vietnamese
                } else {
                    sevi_engine::Language::English
                },
                true,
            ),
            UiCommand::SetLogin(on) => prefs::set_launch_at_login(on)?,
            UiCommand::OpenLink(url) if url.starts_with("https://") => {
                let uri = wide(&url);
                unsafe {
                    ShellExecuteW(None, w!("open"), PCWSTR(uri.as_ptr()), None, None, SW_SHOW);
                }
            }
            UiCommand::CheckUpdates => crate::updater::check_now(self.owner),
            UiCommand::AcceptUpdate => crate::updater::accept_update(),
            UiCommand::PickerReady(window) => {
                let hwnd = HWND(window as usize as *mut _);
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                }
                if self.client.picker_pid() == Some(pid)
                    && unsafe { GetForegroundWindow() } == self.target
                {
                    host::set_picker(hwnd);
                } else {
                    self.client.send(UiEvent::Input(PickerInput::Escape));
                }
            }
            UiCommand::PickerClosed => host::set_picker(HWND::default()),
            UiCommand::InsertEmoji { text, base } => {
                host::set_picker(HWND::default());
                if unsafe { GetForegroundWindow() } == self.target
                    && text.len() <= 256
                    && !text.contains(['\0', '\n', '\r'])
                {
                    host::insert_text(&text);
                    let mut scores = usage();
                    sevi_runtime::emoji::record_usage(&mut scores, &base);
                    let value = scores
                        .iter()
                        .map(|(e, n)| format!("{e}\t{n:.4}\n"))
                        .collect::<String>();
                    host::session(|s| {
                        let _ = s.preferences.set("EmojiUsage", value.into());
                    });
                }
            }
            UiCommand::Quit => unsafe {
                let _ = PostMessageW(Some(self.owner), WM_CLOSE, WPARAM(0), LPARAM(0));
            },
            _ => (),
        }
        Ok(())
    }
    fn tick(&mut self) {
        for request in self.client.requests() {
            if let Err(e) = self.apply(request) {
                self.client.send(UiEvent::Error(e.to_string()));
            }
            self.client
                .send(UiEvent::Snapshot(Box::new(self.snapshot())));
        }
        host::set_ui_process(self.client.pid().unwrap_or(0));
        if self.client.picker_pid().is_none() {
            host::set_picker(HWND::default());
        }
        if !self.client.connected() {
            unsafe {
                let _ = KillTimer(Some(self.owner), 2);
            }
            host::set_ui_process(0);
            host::set_picker(HWND::default());
        }
    }
}
fn usage() -> std::collections::BTreeMap<String, f64> {
    RegistryPreferences
        .string("EmojiUsage")
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('\t')?;
            Some((k.to_owned(), v.parse().ok()?))
        })
        .collect()
}

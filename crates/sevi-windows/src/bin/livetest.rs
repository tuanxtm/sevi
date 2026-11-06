#[cfg(windows)]
mod native {
    use sevi_windows::ui;
    use std::{
        cell::Cell,
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::*,
            System::LibraryLoader::LoadLibraryW,
            System::Threading::{AttachThreadInput, GetCurrentThreadId},
            UI::Controls::EM_SETSEL,
            UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
        core::w,
    };
    thread_local! {static EDIT:Cell<HWND>=Cell::new(HWND::default());}
    unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        ui::callback(|| unsafe {
            if m == WM_SETFOCUS {
                let edit = EDIT.with(Cell::get);
                if !edit.is_invalid() {
                    let _ = SetFocus(Some(edit));
                    return LRESULT(0);
                }
            }
            DefWindowProcW(h, m, w, l)
        })
    }
    fn pump(ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            unsafe {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if Instant::now() >= end {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
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
                    ..Default::default()
                },
            },
        }
    }
    fn send(inputs: &[INPUT]) -> Result<(), String> {
        let inserted = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
        if inserted as usize != inputs.len() {
            return Err(format!(
                "test input failed: inserted {inserted}/{} events",
                inputs.len()
            ));
        }
        Ok(())
    }
    fn key(vk: u16, shift: bool) -> Result<(), String> {
        let mut inputs = vec![];
        if shift {
            inputs.push(input(VK_SHIFT.0, 0, KEYBD_EVENT_FLAGS(0)));
        }
        let flags = if [VK_HOME.0, VK_END.0, VK_LEFT.0, VK_RIGHT.0, VK_DELETE.0].contains(&vk) {
            KEYEVENTF_EXTENDEDKEY
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        inputs.push(input(vk, 0, flags));
        inputs.push(input(vk, 0, flags | KEYEVENTF_KEYUP));
        if shift {
            inputs.push(input(VK_SHIFT.0, 0, KEYEVENTF_KEYUP));
        }
        send(&inputs)?;
        pump(15);
        Ok(())
    }
    fn chord(vk: u16, modifier: u16) -> Result<(), String> {
        send(&[
            input(modifier, 0, KEYBD_EVENT_FLAGS(0)),
            input(vk, 0, KEYBD_EVENT_FLAGS(0)),
            input(vk, 0, KEYEVENTF_KEYUP),
            input(modifier, 0, KEYEVENTF_KEYUP),
        ])?;
        pump(30);
        Ok(())
    }
    fn type_keys(text: &str) -> Result<(), String> {
        let mut units = text.encode_utf16();
        while let Some(c) = units.next() {
            if c == b'\\' as u16 {
                if let Some(next) = units.next() {
                    match next as u8 {
                        b'b' => key(VK_BACK.0, false)?,
                        b'n' => key(VK_RETURN.0, false)?,
                        b't' => key(VK_TAB.0, false)?,
                        _ => (),
                    }
                }
                continue;
            }
            if c == b'{' as u16 {
                let mut command = vec![];
                for c in units.by_ref() {
                    if c == b'}' as u16 {
                        break;
                    }
                    command.push(c)
                }
                match String::from_utf16_lossy(&command).as_str() {
                    "alt+z" => chord(b'Z' as u16, VK_MENU.0)?,
                    "ctrl+a" => chord(b'A' as u16, VK_CONTROL.0)?,
                    "shift+home" => key(VK_HOME.0, true)?,
                    "shift+left" => key(VK_LEFT.0, true)?,
                    "wait" => pump(1500),
                    "home" => key(VK_HOME.0, false)?,
                    "end" => key(VK_END.0, false)?,
                    "left" => key(VK_LEFT.0, false)?,
                    "right" => key(VK_RIGHT.0, false)?,
                    "delete" => key(VK_DELETE.0, false)?,
                    "add" => key(VK_ADD.0, false)?,
                    "caps" => key(VK_CAPITAL.0, false)?,
                    named
                        if named.len() == 4
                            && named.starts_with("num")
                            && named.as_bytes()[3].is_ascii_digit() =>
                    {
                        key(VK_NUMPAD0.0 + u16::from(named.as_bytes()[3] - b'0'), false)?
                    }
                    other => return Err(format!("unknown live-test command: {other}")),
                }
                continue;
            }
            let vk = unsafe { VkKeyScanW(c) };
            if vk == -1 {
                let inputs = [
                    input(0, c, KEYEVENTF_UNICODE),
                    input(0, c, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                ];
                send(&inputs)?;
                pump(30);
            } else {
                key((vk as u16) & 255, (vk as u16 >> 8) & 1 != 0)?
            }
        }
        Ok(())
    }
    fn to_front(window: HWND, edit: HWND) {
        unsafe {
            let foreground = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let own = GetCurrentThreadId();
            if foreground != own {
                let _ = AttachThreadInput(foreground, own, true);
            }
            let _ = SetForegroundWindow(window);
            let _ = BringWindowToTop(window);
            let _ = SetActiveWindow(window);
            let _ = SetFocus(Some(edit));
            if foreground != own {
                let _ = AttachThreadInput(foreground, own, false);
            }
        }
        pump(0);
    }
    pub fn run() -> Result<u8, Box<dyn std::error::Error>> {
        let mut rich = false;
        let mut sources = vec![];
        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "--control=edit" => rich = false,
                "--control=richedit" => rich = true,
                flag if flag.starts_with("--") => {
                    return Err(format!("unknown option: {flag}").into());
                }
                _ => sources.push(std::fs::read_to_string(arg)?),
            }
        }
        let sources = if sources.is_empty() {
            vec![
                include_str!("../../../../Windows/Tests/cases.tsv").into(),
                include_str!("../../../../Windows/Tests/cases-compat.tsv").into(),
            ]
        } else {
            sources
        };
        ui::register(
            "SeviLiveTest",
            Some(proc),
            Some(ui::draw::background_brush(ui::draw::GROUP)),
        );
        let window = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST,
                w!("SeviLiveTest"),
                w!("Sevi live test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                600,
                300,
                None,
                None,
                None,
                None,
            )
        }?;
        if rich {
            unsafe { LoadLibraryW(w!("Msftedit.dll")) }?;
        }
        let edit = ui::child(
            window,
            if rich { "RICHEDIT50W" } else { "EDIT" },
            "",
            WS_BORDER | WINDOW_STYLE(ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32),
            10,
            10,
            560,
            240,
            0,
            ui::draw::Font::Body,
        );
        if edit.is_invalid() {
            return Err("could not create test control".into());
        }
        EDIT.with(|e| e.set(edit));
        for _ in 0..10 {
            to_front(window, edit);
            if unsafe { GetForegroundWindow() } == window {
                break;
            }
            pump(200);
        }
        println!(
            "foreground is {}, focus is {}",
            if unsafe { GetForegroundWindow() } == window {
                "the test window"
            } else {
                "ANOTHER window"
            },
            if unsafe { GetFocus() } == edit {
                "the edit"
            } else {
                "elsewhere"
            }
        );
        if unsafe { GetForegroundWindow() } != window || unsafe { GetFocus() } != edit {
            return Err(
                "test window is not focused; refusing to type into another application".into(),
            );
        }
        println!("control: {}", if rich { "RichEdit" } else { "Edit" });
        pump(300);
        let (mut total, mut failures) = (0, 0);
        for source in sources {
            for line in source
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
            {
                let parts: Vec<_> = line.split('\t').collect();
                if parts.len() < 4 {
                    continue;
                }
                total += 1;
                let tray = unsafe { FindWindowW(w!("SeviTray"), None) }
                    .map_err(|_| "Sevi is not running")?;
                if unsafe { GetForegroundWindow() } != window {
                    to_front(window, edit)
                }
                if unsafe { GetForegroundWindow() } != window {
                    return Err("test window lost foreground; stopping test input".into());
                }
                ui::set_title(edit, "");
                unsafe {
                    let _ = SetFocus(Some(edit));
                    SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                }
                pump(150);
                key(VK_ESCAPE.0, false)?;
                pump(50);
                unsafe {
                    SendMessageTimeoutW(
                        tray,
                        WM_APP + 2,
                        WPARAM(usize::from(parts[1] == "vi")),
                        LPARAM(0),
                        SMTO_NORMAL,
                        2000,
                        None,
                    );
                }
                pump(100);
                type_keys(parts[2])?;
                pump(200);
                let got = ui::title(edit).replace('\r', "");
                let ok = got == parts[3];
                if !ok {
                    failures += 1
                }
                println!(
                    "{} {}: got {got:?}, expected {:?}",
                    if ok { "ok  " } else { "FAIL" },
                    parts[0],
                    parts[3]
                );
                key(VK_ESCAPE.0, false)?;
            }
        }
        println!("{}/{} passed", total - failures, total);
        unsafe {
            let _ = DestroyWindow(window);
        }
        Ok(failures.min(255) as u8)
    }
}
#[cfg(windows)]
fn main() {
    match native::run() {
        Ok(code) => std::process::exit(code as i32),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(99)
        }
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows native typing tests require Windows.");
    std::process::exit(1);
}

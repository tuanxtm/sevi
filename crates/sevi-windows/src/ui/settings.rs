use super::{
    controls,
    draw::{self, Font},
    *,
};
use std::{cell::RefCell, collections::BTreeMap};
use windows::Win32::UI::{Controls::*, Input::KeyboardAndMouse::EnableWindow};
const WIDTH: i32 = 460;
const MARGIN: i32 = 20;
const ROW: i32 = 34;
const SEG: i32 = 200;
const HOTKEY: i32 = 201;
const BEEP: i32 = 202;
const AUTO: i32 = 203;
const PAUSE: i32 = 204;
const SMART: i32 = 205;
const SITE: i32 = 206;
const MARK: i32 = 207;
const SPELL: i32 = 208;
const RESTORE: i32 = 209;
const LOGIN: i32 = 210;
const UPDATE: i32 = 211;
const EMOJI: i32 = 213;
const EMOJIKEY: i32 = 214;
const STYLE: i32 = 216;
struct Main {
    window: HWND,
    hint: HWND,
    segment: HWND,
    tooltip: HWND,
    tip_texts: Vec<Vec<u16>>,
    controls: BTreeMap<i32, HWND>,
    groups: Vec<(i32, i32)>,
    titles: Vec<(i32, String)>,
    separators: Vec<i32>,
}
thread_local! {static MAIN:RefCell<Option<Main>>=const{RefCell::new(None)};}
fn state<T: Default>(f: impl FnOnce(&mut Main) -> T) -> T {
    MAIN.with(|s| {
        s.try_borrow_mut()
            .ok()
            .and_then(|mut s| s.as_mut().map(f))
            .unwrap_or_default()
    })
}
fn control_id(name: &str) -> i32 {
    match name {
        "C_HOTKEY" => HOTKEY,
        "C_BEEP" => BEEP,
        "C_AUTO" => AUTO,
        "C_PAUSE" => PAUSE,
        "C_SMART" => SMART,
        "C_SITE" => SITE,
        "C_MARK" => MARK,
        "C_SPELL" => SPELL,
        "C_RESTORE" => RESTORE,
        "C_LOGIN" => LOGIN,
        "C_UPDATE" => UPDATE,
        "C_EMOJI" => EMOJI,
        "C_EMOJIKEY" => EMOJIKEY,
        "C_STYLE" => STYLE,
        _ => 0,
    }
}
impl Main {
    fn enabled(&self) {
        for (parent, child) in [(AUTO, PAUSE), (SMART, SITE), (SPELL, RESTORE)] {
            if let (Some(p), Some(c)) = (self.controls.get(&parent), self.controls.get(&child)) {
                unsafe {
                    let _ = EnableWindow(*c, controls::checked(*p));
                }
            }
        }
    }
    fn hint(&self) {
        set_title(
            self.hint,
            &format!("Bấm {} để chuyển", hotkey_label(hotkey())),
        );
    }
    fn tip(&mut self, title: &str, tip: &str, y: i32) {
        child(
            self.window,
            "STATIC",
            title,
            WINDOW_STYLE(SS_LEFT.0 | SS_NOPREFIX.0),
            36,
            y + 7,
            300,
            20,
            0,
            Font::Body,
        );
        unsafe {
            let dc = GetDC(Some(self.window));
            let width = draw::width(dc, title, Font::Body);
            ReleaseDC(Some(self.window), dc);
            let widget = child(
                self.window,
                "SeviTip",
                tip,
                WINDOW_STYLE(0),
                36 + width + 6,
                y + 9,
                16,
                16,
                0,
                Font::Body,
            );
            if self.tooltip.is_invalid() {
                self.tooltip = CreateWindowExW(
                    WS_EX_TOPMOST,
                    TOOLTIPS_CLASSW,
                    None,
                    WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
                    0,
                    0,
                    0,
                    0,
                    Some(self.window),
                    None,
                    None,
                    None,
                )
                .unwrap_or_default();
                dpi::track_control(self.window, self.tooltip, None, Font::Small);
            }
            self.tip_texts.push(wide(tip));
            let info = TTTOOLINFOW {
                cbSize: size_of::<TTTOOLINFOW>() as u32,
                uFlags: TTF_IDISHWND | TTF_SUBCLASS,
                hwnd: self.window,
                uId: widget.0 as usize,
                lpszText: windows::core::PWSTR(self.tip_texts.last_mut().unwrap().as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                self.tooltip,
                TTM_ADDTOOLW,
                Some(WPARAM(0)),
                Some(LPARAM((&info as *const TTTOOLINFOW) as isize)),
            );
        }
    }
    fn build(&mut self) {
        let _dpi = dpi::Context::for_window(self.window);
        destroy_children(self.window);
        if !self.tooltip.is_invalid() {
            unsafe {
                let _ = DestroyWindow(self.tooltip);
            }
        }
        self.tooltip = HWND::default();
        self.tip_texts.clear();
        self.controls.clear();
        self.groups.clear();
        self.titles.clear();
        self.separators.clear();
        self.hint = child(
            self.window,
            "STATIC",
            "",
            WINDOW_STYLE(SS_LEFT.0),
            76,
            43,
            220,
            18,
            0,
            Font::Small,
        );
        self.hint();
        self.segment = controls::segment(self.window, WIDTH - MARGIN - 164, 22, 164, SEG);
        unsafe {
            SetWindowLongPtrW(
                self.segment,
                GWLP_USERDATA,
                isize::from(host::language() == sevi_engine::Language::English),
            );
        }
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("settings.json")).expect("settings data");
        let mut y = 70;
        let mut emoji_rec = None;
        for (title, ids) in [
            ("", vec![HOTKEY, BEEP]),
            ("Tự chuyển", vec![AUTO, PAUSE, SMART, SITE]),
            ("Gõ tiếng Việt", vec![STYLE, MARK, SPELL, RESTORE]),
            ("", vec![EMOJI, EMOJIKEY]),
            ("", vec![LOGIN, UPDATE]),
        ] {
            if !title.is_empty() {
                self.titles.push((y, title.into()));
                y += 26
            }
            let start = y;
            for id in ids {
                if id == EMOJIKEY && RegistryPreferences.int("EmojiPicker", 0) == 0 {
                    continue;
                }
                if y != start {
                    self.separators.push(y)
                }
                let row = rows
                    .iter()
                    .find(|r| control_id(r["id"].as_str().unwrap()) == id)
                    .unwrap();
                self.tip(
                    row["label"].as_str().unwrap(),
                    row["tip"].as_str().unwrap(),
                    y,
                );
                let key = row["key"].as_str().unwrap();
                let default = row["default"].as_i64().unwrap() as i32;
                let widget = match row["kind"].as_str().unwrap() {
                    "toggle" => controls::toggle(
                        self.window,
                        WIDTH - MARGIN - 16 - 40,
                        y + 5,
                        id,
                        RegistryPreferences.int(key, default) != 0,
                    ),
                    "login" => controls::toggle(
                        self.window,
                        WIDTH - MARGIN - 16 - 40,
                        y + 5,
                        id,
                        crate::prefs::launch_at_login(),
                    ),
                    "beep" => controls::toggle(
                        self.window,
                        WIDTH - MARGIN - 16 - 40,
                        y + 5,
                        id,
                        hotkey() & 0x8000 != 0,
                    ),
                    "hotkey" => controls::recorder(
                        self.window,
                        "SwitchKeyStatus",
                        true,
                        hotkey(),
                        WIDTH - MARGIN - 16 - 134,
                        y + 3,
                        134,
                        28,
                        id,
                        draw::GROUP,
                    ),
                    "emoji_hotkey" => {
                        let h = controls::recorder(
                            self.window,
                            "EmojiHotkey",
                            false,
                            RegistryPreferences.int("EmojiHotkey", 0) as u32,
                            WIDTH - MARGIN - 16 - 134,
                            y + 3,
                            134,
                            28,
                            id,
                            draw::GROUP,
                        );
                        emoji_rec = Some(h);
                        h
                    }
                    "style" | "mark" => {
                        let style = id == STYLE;
                        let width = if style { 150 } else { 130 };
                        let h = child(
                            self.window,
                            "COMBOBOX",
                            "",
                            WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_TABSTOP,
                            WIDTH - MARGIN - 16 - width,
                            y + 4,
                            width,
                            200,
                            id,
                            Font::Body,
                        );
                        let labels: Vec<&str> = if style {
                            vec!["Telex", "Telex đơn giản 1", "Telex đơn giản 2"]
                        } else {
                            vec!["hoà, thuý", "hòa, thúy"]
                        };
                        for label in labels {
                            let label = wide(label);
                            unsafe {
                                SendMessageW(
                                    h,
                                    CB_ADDSTRING,
                                    Some(WPARAM(0)),
                                    Some(LPARAM(label.as_ptr() as isize)),
                                );
                            }
                        }
                        let n = RegistryPreferences.int(key, default);
                        unsafe {
                            SendMessageW(
                                h,
                                CB_SETCURSEL,
                                Some(WPARAM(if style {
                                    n.clamp(0, 2) as usize
                                } else {
                                    usize::from(n == 0)
                                })),
                                Some(LPARAM(0)),
                            );
                        }
                        h
                    }
                    _ => unreachable!(),
                };
                self.controls.insert(id, widget);
                y += ROW;
            }
            self.groups.push((start, y));
            y += 12;
        }
        self.enabled();
        unsafe {
            dpi::set_client_size(self.window, WIDTH, y);
            let _ = RedrawWindow(
                Some(self.window),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
            if GetForegroundWindow() == self.window
                && RegistryPreferences.int("EmojiHotkey", 0) == 0
                && let Some(h) = emoji_rec
            {
                controls::start_recording(h)
            }
        }
    }
    fn paint(&self) {
        let p = draw::Paint::begin(self.window, draw::BG);
        for (top, bottom) in &self.groups {
            p.round(
                MARGIN as f32,
                *top as f32,
                (WIDTH - 2 * MARGIN) as f32,
                (bottom - top) as f32,
                10.0,
                draw::rgb(draw::GROUP, 255),
                Some(draw::rgb(draw::BORDER, 255)),
            );
        }
        for y in &self.separators {
            p.line(36, *y, WIDTH - 72, draw::SEP)
        }
        for (y, title) in &self.titles {
            p.text(
                title,
                Font::Bold,
                draw::TEXT,
                RECT {
                    left: 36,
                    top: y + 4,
                    right: WIDTH,
                    bottom: y + 26,
                },
                DT_LEFT | DT_SINGLELINE,
            )
        }
        draw::icon(p.dc, MARGIN, 14, 44);
        draw::wordmark(p.dc, Font::Wordmark, 76, 9);
    }
}
pub fn schedule_rebuild() {
    state(|s| unsafe {
        let _ = PostMessageW(Some(s.window), WM_REBUILD, WPARAM(0), LPARAM(0));
    });
}
pub fn language_changed() {
    state(|s| {
        unsafe {
            SetWindowLongPtrW(
                s.segment,
                GWLP_USERDATA,
                isize::from(host::language() == sevi_engine::Language::English),
            );
        }
        invalidate(s.segment);
    });
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                state(|s| s.paint());
                return LRESULT(0);
            }
            WM_CTLCOLORSTATIC => {
                let dc = HDC(w.0 as *mut _);
                SetBkMode(dc, TRANSPARENT);
                let hint = state(|s| HWND(l.0 as *mut _) == s.hint);
                SetTextColor(
                    dc,
                    COLORREF(if hint { draw::SECONDARY } else { draw::TEXT }),
                );
                return LRESULT(
                    draw::background_brush(if hint { draw::BG } else { draw::GROUP }).0 as isize,
                );
            }
            WM_COMMAND => {
                let id = (w.0 & 65535) as i32;
                let widget = HWND(l.0 as *mut _);
                let set = |key: &str| {
                    let _ = RegistryPreferences.set_int(key, i32::from(controls::checked(widget)));
                    host::reload_settings();
                    state(|s| s.enabled());
                };
                match id {
                    SEG => {
                        let index = GetWindowLongPtrW(widget, GWLP_USERDATA);
                        host::set_language(
                            if index == 0 {
                                sevi_engine::Language::Vietnamese
                            } else {
                                sevi_engine::Language::English
                            },
                            true,
                        )
                    }
                    HOTKEY => state(|s| s.hint()),
                    BEEP => {
                        let hk = (hotkey() & !0x8000)
                            | if controls::checked(widget) { 0x8000 } else { 0 };
                        let _ = RegistryPreferences.set_int("SwitchKeyStatus", hk as i32);
                        changed_hotkey();
                    }
                    AUTO => set("AutoSwitchByTyping"),
                    PAUSE => set("PauseAutoAfterManual"),
                    SMART => set("UseSmartSwitchKey"),
                    SITE => set("SmartSwitchPerSite"),
                    SPELL => set("Spelling"),
                    RESTORE => set("RestoreIfInvalidWord"),
                    STYLE | MARK if (w.0 >> 16) & 65535 == CBN_SELCHANGE as usize => {
                        let selected = SendMessageW(widget, CB_GETCURSEL, None, None).0;
                        let _ = RegistryPreferences.set_int(
                            if id == STYLE {
                                "TelexStyle"
                            } else {
                                "ModernOrthography"
                            },
                            if id == STYLE {
                                selected as i32
                            } else {
                                i32::from(selected == 0)
                            },
                        );
                        host::reload_settings();
                    }
                    EMOJI => {
                        set("EmojiPicker");
                        crate::emoji::settings_changed();
                        changed_hotkey();
                    }
                    LOGIN => {
                        let _ = crate::prefs::set_launch_at_login(controls::checked(widget));
                        controls::set_checked(widget, crate::prefs::launch_at_login());
                    }
                    UPDATE => {
                        let _ = RegistryPreferences
                            .set_int("AutoUpdate", i32::from(controls::checked(widget)));
                    }
                    _ => (),
                }
                return LRESULT(0);
            }
            WM_REBUILD => {
                state(Main::build);
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = DestroyWindow(h);
                return LRESULT(0);
            }
            WM_DESTROY => {
                MAIN.with(|s| {
                    s.borrow_mut().take();
                });
                return LRESULT(0);
            }
            _ => (),
        }
        DefWindowProcW(h, m, w, l)
    })
}
pub fn show() {
    controls::init();
    let existing = state(|s| s.window);
    if !existing.is_invalid() {
        bring_front(existing);
        return;
    }
    register("SeviMain", Some(proc), None);
    let window = make_window("SeviMain", "Sevi", WIDTH, 600, false);
    MAIN.with(|s| {
        *s.borrow_mut() = Some(Main {
            window,
            hint: HWND::default(),
            segment: HWND::default(),
            tooltip: HWND::default(),
            tip_texts: vec![],
            controls: BTreeMap::new(),
            groups: vec![],
            titles: vec![],
            separators: vec![],
        })
    });
    state(Main::build);
    center(window);
    bring_front(window);
}

use super::{
    controls,
    draw::{self, Font},
    *,
};
use std::cell::RefCell;
use unicode_normalization::UnicodeNormalization;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::{
    Controls::*,
    Input::KeyboardAndMouse::{EnableWindow, SetFocus},
};
const WIDTH: i32 = 540;
const HEIGHT: i32 = 470;
const PAD: i32 = 28;
const CONTENT: i32 = WIDTH - 2 * PAD;
const BACK: i32 = 500;
const NEXT: i32 = 501;
const REC: i32 = 502;
const BEEP: i32 = 503;
const EMOJI: i32 = 504;
const EMOJIKEY: i32 = 505;
const EDIT: i32 = 520;
const PRACTICE: [(&str, &str); 3] = [
    ("order traf suwax nhes", "order trà sữa nhé"),
    ("meeting lucs ba giowf nhes", "meeting lúc ba giờ nhé"),
    ("thanks, camr own nhieeuf", "thanks, cảm ơn nhiều"),
];
const WELCOME: &str = "Sevi chỉ hỗ trợ Telex, kiểu gõ phổ biến nhất. Bạn gõ tiếng Anh bình thường, Sevi tự nhận ra và không bỏ dấu nhầm.";
const SHORTCUT: &str = "Sevi tự chuyển khi bạn gõ lẫn tiếng Anh. Khi muốn tự chuyển, bấm phím tắt này. Bấm vào ô để đổi phím.";
const EMOJI_COPY: &str = "Bấm một phím tắt để mở bảng emoji ở giữa màn hình. Tìm bằng tiếng Việt hay tiếng Anh đều được: \"ngựa\" hay \"horse\" đều ra 🐴.";
struct Guide {
    window: HWND,
    step: usize,
    entries: [HWND; 3],
    ok: [bool; 3],
    positions: [i32; 8],
}
thread_local! {static GUIDE:RefCell<Option<Guide>>=const{RefCell::new(None)};}
fn state<T: Default>(f: impl FnOnce(&mut Guide) -> T) -> T {
    GUIDE.with(|s| {
        s.try_borrow_mut()
            .ok()
            .and_then(|mut s| s.as_mut().map(f))
            .unwrap_or_default()
    })
}
fn normalized(s: &str) -> String {
    s.trim_matches(' ').nfc().collect::<String>().to_lowercase()
}
fn practice_text() -> String {
    if host::language() == sevi_engine::Language::Vietnamese {
        "Gõ đúng các phím bên dưới, kể cả tiếng Anh. Sevi bỏ dấu cho tiếng Việt và để nguyên tiếng Anh.".into()
    } else {
        format!(
            "Bạn đang ở tiếng Anh: bấm {} để sang tiếng Việt, rồi gõ các phím bên dưới.",
            hotkey_label(hotkey())
        )
    }
}
impl Guide {
    fn build(&mut self) {
        let _dpi = dpi::Context::for_window(self.window);
        destroy_children(self.window);
        self.entries = [HWND::default(); 3];
        let mut y = 98;
        self.positions = [0; 8];
        let emoji_on = RegistryPreferences.int("EmojiPicker", 0) != 0;
        let emoji_key = RegistryPreferences.int("EmojiHotkey", 0) as u32;
        self.positions[0] = y;
        y += 42;
        self.positions[1] = y;
        match self.step {
            0 => {
                y += draw::height(WELCOME, Font::Body, CONTENT) + 14;
                self.positions[2] = y;
            }
            1 => {
                y += 52;
                for i in 0..3 {
                    self.positions[2 + i] = y;
                    let entry = child(
                        self.window,
                        "EDIT",
                        "",
                        WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                        PAD,
                        y + 22,
                        CONTENT - 34,
                        28,
                        EDIT + i as i32,
                        Font::Body,
                    );
                    unsafe {
                        let _ = SetWindowSubclass(entry, Some(edit_proc), i, 0);
                        SendMessageW(
                            entry,
                            EM_SETMARGINS,
                            Some(WPARAM((EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize)),
                            Some(LPARAM(6 | (6 << 16))),
                        );
                    }
                    self.entries[i] = entry;
                    self.ok[i] = false;
                    y += 62;
                }
                unsafe {
                    let _ = SetFocus(Some(self.entries[0]));
                }
            }
            2 => {
                y += draw::height(SHORTCUT, Font::Body, CONTENT) + 14;
                controls::recorder(
                    self.window,
                    "SwitchKeyStatus",
                    true,
                    hotkey(),
                    PAD,
                    y,
                    150,
                    34,
                    REC,
                    draw::GROUP,
                );
                self.positions[2] = y;
                y += 48;
                self.positions[3] = y;
                let dc = unsafe { GetDC(Some(self.window)) };
                let width = draw::width(dc, "Âm báo khi chuyển", Font::Body);
                unsafe {
                    ReleaseDC(Some(self.window), dc);
                }
                let t = controls::toggle(
                    self.window,
                    PAD + width + 12,
                    y,
                    BEEP,
                    hotkey() & 0x8000 != 0,
                );
                controls::background(t, draw::GROUP);
            }
            3 => {
                y += draw::height(EMOJI_COPY, Font::Body, CONTENT) + 14;
                self.positions[2] = y;
                let dc = unsafe { GetDC(Some(self.window)) };
                let width = draw::width(dc, "Bật bảng emoji", Font::Body);
                let width2 = draw::width(dc, "Phím mở bảng emoji", Font::Body);
                unsafe {
                    ReleaseDC(Some(self.window), dc);
                }
                controls::toggle(self.window, PAD + width + 12, y, EMOJI, emoji_on);
                y += 37;
                self.positions[3] = y;
                self.positions[4] = -1;
                if emoji_on {
                    let r = controls::recorder(
                        self.window,
                        "EmojiHotkey",
                        false,
                        emoji_key,
                        PAD + width2 + 12,
                        y,
                        150,
                        34,
                        EMOJIKEY,
                        draw::GROUP,
                    );
                    y += 44;
                    if emoji_key == 0 {
                        self.positions[4] = y;
                        if unsafe { GetForegroundWindow() } == self.window {
                            controls::start_recording(r)
                        }
                    }
                }
            }
            _ => (),
        }
        if self.step != 0 {
            controls::button(
                self.window,
                "Quay lại",
                PAD,
                HEIGHT - PAD - 32,
                100,
                32,
                BACK,
                false,
                draw::GROUP,
            );
        }
        let next = controls::button(
            self.window,
            if self.step == 0 {
                "Bắt đầu"
            } else if self.step == 3 {
                "Xong"
            } else {
                "Tiếp tục"
            },
            WIDTH - PAD - 110,
            HEIGHT - PAD - 32,
            110,
            32,
            NEXT,
            true,
            draw::GROUP,
        );
        if self.step == 3 && emoji_on && emoji_key == 0 {
            unsafe {
                let _ = EnableWindow(next, false);
            }
        }
        invalidate(self.window);
    }
    fn paint(&self) {
        let p = draw::Paint::begin(self.window, draw::GROUP);
        draw::icon(p.dc, PAD, PAD, 48);
        draw::wordmark(p.dc, Font::Wordmark, PAD + 62, PAD - 3);
        p.text(
            "Gõ tiếng Việt, tự chuyển Anh/Việt cho bạn.",
            Font::Body,
            draw::SECONDARY,
            RECT {
                left: PAD + 62,
                top: PAD + 27,
                right: WIDTH - PAD - 60,
                bottom: PAD + 49,
            },
            DT_SINGLELINE,
        );
        for i in 0..4 {
            p.ellipse(
                (WIDTH - PAD - (4 - i) * 13 + 6) as f32,
                (PAD + 20) as f32,
                7.0,
                7.0,
                draw::rgb(
                    if i as usize == self.step {
                        draw::BRAND
                    } else {
                        0x00d2cdcd
                    },
                    255,
                ),
            );
        }
        let title = |s: &str, y: i32| {
            p.text(
                s,
                Font::Title2,
                draw::TEXT,
                RECT {
                    left: PAD,
                    top: y,
                    right: WIDTH - PAD,
                    bottom: y + 28,
                },
                DT_LEFT | DT_SINGLELINE,
            )
        };
        let para = |s: &str, y: i32, height: i32| {
            p.text(
                s,
                Font::Body,
                draw::SECONDARY,
                RECT {
                    left: PAD,
                    top: y,
                    right: WIDTH - PAD,
                    bottom: y + height,
                },
                DT_WORDBREAK,
            )
        };
        let label = |s: &str, x: i32, y: i32, height: i32, c: u32, role: Font| {
            p.text(
                s,
                role,
                c,
                RECT {
                    left: x,
                    top: y,
                    right: WIDTH - PAD,
                    bottom: y + height,
                },
                DT_SINGLELINE | DT_VCENTER,
            )
        };
        match self.step {
            0 => {
                title("Sevi dùng kiểu gõ Telex", self.positions[0]);
                para(WELCOME, self.positions[1], 60);
                let top = self.positions[2];
                p.round(
                    PAD as f32,
                    top as f32,
                    CONTENT as f32,
                    148.0,
                    10.0,
                    draw::rgb(0x00f4f2f2, 255),
                    None,
                );
                for (i, (keys, result)) in [
                    ("aa", "â"),
                    ("aw", "ă"),
                    ("ee", "ê"),
                    ("oo", "ô"),
                    ("ow", "ơ"),
                    ("uw", "ư"),
                    ("dd", "đ"),
                    ("w", "ư"),
                    ("s", "sắc"),
                    ("f", "huyền"),
                    ("r", "hỏi"),
                    ("x", "ngã"),
                    ("j", "nặng"),
                    ("z", "xoá dấu"),
                ]
                .iter()
                .enumerate()
                {
                    let x = PAD + 14 + (i % 4) as i32 * ((CONTENT - 28) / 4);
                    let y = top + 14 + (i / 4) as i32 * 32;
                    let width = draw::width(p.dc, keys, Font::MonoBold) + 12;
                    p.round(
                        x as f32,
                        y as f32,
                        width as f32,
                        24.0,
                        5.0,
                        0xffffffff,
                        None,
                    );
                    p.text(
                        keys,
                        Font::MonoBold,
                        draw::TEXT,
                        RECT {
                            left: x,
                            top: y,
                            right: x + width,
                            bottom: y + 24,
                        },
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                    label("→", x + width + 6, y, 24, draw::TERTIARY, Font::Small);
                    label(result, x + width + 22, y, 24, draw::TEXT, Font::Body);
                }
            }
            1 => {
                title("Gõ thử", self.positions[0]);
                para(&practice_text(), self.positions[1], 40);
                for (i, (keys, result)) in PRACTICE.iter().enumerate() {
                    let y = self.positions[2 + i];
                    let mut x = PAD;
                    for (text, role, color) in [
                        ("Gõ", Font::Callout, draw::SECONDARY),
                        (*keys, Font::MonoBold, draw::TEXT),
                        ("→", Font::Small, draw::TERTIARY),
                        (*result, Font::Callout, draw::TEXT),
                    ] {
                        label(text, x, y, 20, color, role);
                        x += draw::width(p.dc, text, role) + 6;
                    }
                    let x = (WIDTH - PAD - 22) as f32;
                    let y = (y + 25) as f32;
                    if self.ok[i] {
                        p.ellipse(x, y, 22.0, 22.0, draw::rgb(draw::GREEN, 255));
                        p.text(
                            "✓",
                            Font::Bold,
                            draw::GROUP,
                            RECT {
                                left: x as i32,
                                top: y as i32,
                                right: x as i32 + 22,
                                bottom: y as i32 + 22,
                            },
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                        );
                    } else {
                        p.round(
                            x + 1.0,
                            y + 1.0,
                            20.0,
                            20.0,
                            10.0,
                            0,
                            Some(draw::rgb(0x00ccc8c8, 255)),
                        );
                    }
                }
            }
            2 => {
                title("Phím chuyển Anh/Việt", self.positions[0]);
                para(SHORTCUT, self.positions[1], 60);
                label(
                    if host::language() == sevi_engine::Language::Vietnamese {
                        "Đang gõ: Tiếng Việt"
                    } else {
                        "Đang gõ: English"
                    },
                    PAD + 162,
                    self.positions[2],
                    34,
                    draw::SECONDARY,
                    Font::Body,
                );
                label(
                    "Âm báo khi chuyển",
                    PAD,
                    self.positions[3],
                    23,
                    draw::TEXT,
                    Font::Body,
                );
            }
            3 => {
                title("Bảng emoji", self.positions[0]);
                let width = draw::width(p.dc, "Bảng emoji", Font::Title2);
                label(
                    "không bắt buộc",
                    PAD + width + 8,
                    self.positions[0] + 3,
                    26,
                    draw::SECONDARY,
                    Font::Callout,
                );
                draw::color_text(
                    p.dc,
                    EMOJI_COPY,
                    RECT {
                        left: PAD,
                        top: self.positions[1],
                        right: WIDTH - PAD,
                        bottom: self.positions[2] - 14,
                    },
                    15.0,
                    draw::SECONDARY,
                    "Segoe UI",
                    false,
                );
                label(
                    "Bật bảng emoji",
                    PAD,
                    self.positions[2],
                    23,
                    draw::TEXT,
                    Font::Body,
                );
                if RegistryPreferences.int("EmojiPicker", 0) != 0 {
                    label(
                        "Phím mở bảng emoji",
                        PAD,
                        self.positions[3],
                        34,
                        draw::TEXT,
                        Font::Body,
                    );
                }
                if self.positions[4] >= 0 {
                    label(
                        "Bấm một phím kèm Ctrl, Alt hoặc Win, ví dụ Ctrl+Alt+E.",
                        PAD,
                        self.positions[4],
                        20,
                        draw::SECONDARY,
                        Font::Callout,
                    );
                }
            }
            _ => (),
        }
    }
}
pub fn language_changed() {
    state(|s| invalidate(s.window));
}
pub fn schedule_rebuild() {
    state(|s| {
        if s.step == 1 {
            invalidate(s.window)
        } else {
            unsafe {
                let _ = PostMessageW(Some(s.window), WM_REBUILD, WPARAM(0), LPARAM(0));
            }
        }
    });
}
unsafe extern "system" fn edit_proc(
    h: HWND,
    m: u32,
    w: WPARAM,
    l: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    callback(|| unsafe {
        if m == WM_KEYDOWN && w.0 == 13 {
            host::reset_typing();
            state(|s| {
                let _ = SetFocus(Some(s.entries[(id + 1).min(2)]));
            });
            return LRESULT(0);
        }
        if m == WM_CHAR && (w.0 == 13 || w.0 == 10) {
            return LRESULT(0);
        }
        DefSubclassProc(h, m, w, l)
    })
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_PAINT => {
                state(|s| s.paint());
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_COMMAND => {
                let id = (w.0 & 65535) as i32;
                if (EDIT..EDIT + 3).contains(&id) && (w.0 >> 16) & 65535 == EN_CHANGE as usize {
                    let i = (id - EDIT) as usize;
                    let value = normalized(&title(HWND(l.0 as *mut _)));
                    state(|s| {
                        let ok = value == normalized(PRACTICE[i].1);
                        if ok != s.ok[i] {
                            s.ok[i] = ok;
                            invalidate(h);
                            if ok && i < 2 {
                                host::reset_typing();
                                let _ = SetFocus(Some(s.entries[i + 1]));
                            }
                        }
                    });
                    return LRESULT(0);
                }
                match id {
                    BACK => state(|s| {
                        s.step = s.step.saturating_sub(1);
                        s.build();
                    }),
                    NEXT => {
                        let finish = state(|s| s.step == 3);
                        if finish {
                            let _ = RegistryPreferences.set_int("OnboardingDone", 1);
                            let _ = DestroyWindow(h);
                        } else {
                            state(|s| {
                                s.step += 1;
                                s.build();
                            });
                        }
                    }
                    REC => {
                        invalidate(h);
                        settings::schedule_rebuild();
                    }
                    BEEP => {
                        let hk = (hotkey() & !0x8000)
                            | if controls::checked(HWND(l.0 as *mut _)) {
                                0x8000
                            } else {
                                0
                            };
                        let _ = RegistryPreferences.set_int("SwitchKeyStatus", hk as i32);
                        host::reload_settings();
                        settings::schedule_rebuild();
                    }
                    EMOJI => {
                        let _ = RegistryPreferences.set_int(
                            "EmojiPicker",
                            i32::from(controls::checked(HWND(l.0 as *mut _))),
                        );
                        host::reload_settings();
                        crate::emoji::settings_changed();
                        schedule_rebuild();
                        settings::schedule_rebuild();
                    }
                    EMOJIKEY => schedule_rebuild(),
                    _ => (),
                }
                return LRESULT(0);
            }
            WM_REBUILD => {
                state(Guide::build);
                return LRESULT(0);
            }
            WM_CTLCOLOREDIT => return LRESULT(draw::background_brush(draw::GROUP).0 as isize),
            WM_CLOSE => {
                let _ = DestroyWindow(h);
                return LRESULT(0);
            }
            WM_DESTROY => {
                GUIDE.with(|s| {
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
    register("SeviOnboarding", Some(proc), None);
    let window = make_window("SeviOnboarding", "Sevi", WIDTH, HEIGHT, false);
    let step = RegistryPreferences.int("OnboardingStep", 0).clamp(0, 3) as usize;
    GUIDE.with(|s| {
        *s.borrow_mut() = Some(Guide {
            window,
            step,
            entries: [HWND::default(); 3],
            ok: [false; 3],
            positions: [0; 8],
        })
    });
    state(Guide::build);
    center(window);
    bring_front(window);
    let _ = RegistryPreferences.set_int("FirstRunDone", 1);
}

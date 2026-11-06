use super::{
    draw::{self, Font},
    *,
};
use std::cell::Cell;
use windows::Win32::UI::Controls::*;
thread_local! {static ABOUT:Cell<HWND>=Cell::new(HWND::default());static CREDITS:Cell<HWND>=Cell::new(HWND::default());}
struct Link {
    rect: RECT,
    url: String,
}
struct Info {
    window: HWND,
    credits: bool,
    scroll: i32,
    content: i32,
    links: Vec<Link>,
}
impl Info {
    fn link(&mut self, dc: HDC, text: &str, url: &str, x: i32, y: i32, role: Font) -> i32 {
        let width = draw::width(dc, text, role);
        let rect = RECT {
            left: x,
            top: y,
            right: x + width,
            bottom: y + 22,
        };
        draw::text_at(
            dc,
            text,
            role,
            if url.is_empty() {
                draw::TEXT
            } else {
                draw::ACCENT
            },
            rect,
            DT_SINGLELINE,
        );
        if !url.is_empty() {
            self.links.push(Link {
                rect,
                url: url.into(),
            });
        }
        width
    }
    fn paint(&mut self) {
        let p = draw::Paint::begin(self.window, draw::GROUP);
        self.links.clear();
        if self.credits {
            let groups: Vec<serde_json::Value> =
                serde_json::from_str(include_str!("credits.json")).expect("credits");
            let mut y = 24 - self.scroll;
            for group in groups {
                p.text(
                    group["title"].as_str().unwrap(),
                    Font::Title,
                    draw::TEXT,
                    RECT {
                        left: 24,
                        top: y,
                        right: 496,
                        bottom: y + 22,
                    },
                    DT_SINGLELINE,
                );
                y += 28;
                let note = group["note"].as_str().unwrap();
                let height = draw::height(note, Font::Callout, 472);
                p.text(
                    note,
                    Font::Callout,
                    draw::SECONDARY,
                    RECT {
                        left: 24,
                        top: y,
                        right: 496,
                        bottom: y + height,
                    },
                    DT_WORDBREAK,
                );
                y += height + 8;
                for item in group["items"].as_array().unwrap() {
                    let (name, by, license, url) = (
                        item[0].as_str().unwrap(),
                        item[1].as_str().unwrap(),
                        item[2].as_str().unwrap(),
                        item[3].as_str().unwrap(),
                    );
                    let width = draw::width(p.dc, license, Font::Small);
                    self.link(p.dc, name, url, 24, y, Font::Body);
                    p.text(
                        license,
                        Font::Small,
                        draw::SECONDARY,
                        RECT {
                            left: 496 - width,
                            top: y + 2,
                            right: 496,
                            bottom: y + 20,
                        },
                        DT_RIGHT | DT_SINGLELINE,
                    );
                    p.text(
                        by,
                        Font::Small,
                        draw::SECONDARY,
                        RECT {
                            left: 24,
                            top: y + 21,
                            right: 484 - width,
                            bottom: y + 38,
                        },
                        DT_SINGLELINE | DT_END_ELLIPSIS,
                    );
                    y += 40;
                }
                y += 14;
            }
            self.content = y + self.scroll + 14;
            unsafe {
                let info = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                    nMin: 0,
                    nMax: self.content - 1,
                    nPage: p.rect.bottom as u32,
                    nPos: self.scroll,
                    ..Default::default()
                };
                SetScrollInfo(self.window, SB_VERT, &info, true);
            }
        } else {
            let width = p.rect.right;
            draw::icon(p.dc, (width - 96) / 2, 24, 96);
            p.text(
                "Sevi",
                Font::Wordmark,
                draw::TEXT,
                RECT {
                    left: 0,
                    top: 130,
                    right: width,
                    bottom: 160,
                },
                DT_CENTER | DT_SINGLELINE,
            );
            p.text(
                &format!("Phiên bản {}", env!("CARGO_PKG_VERSION")),
                Font::Small,
                draw::SECONDARY,
                RECT {
                    left: 0,
                    top: 162,
                    right: width,
                    bottom: 182,
                },
                DT_CENTER | DT_SINGLELINE,
            );
            p.text(
                "Tony Dinh",
                Font::Small,
                draw::SECONDARY,
                RECT {
                    left: 0,
                    top: 196,
                    right: width,
                    bottom: 216,
                },
                DT_CENTER | DT_SINGLELINE,
            );
            let segments = [
                ("Dựa trên ", ""),
                ("OpenKey", "https://github.com/tuyenvm/OpenKey"),
                (" của Tuyen Mai", ""),
            ];
            let total = segments
                .iter()
                .map(|(s, _)| draw::width(p.dc, s, Font::Small))
                .sum::<i32>();
            let mut x = (width - total) / 2;
            for (text, url) in segments {
                let w = draw::width(p.dc, text, Font::Small);
                if url.is_empty() {
                    p.text(
                        text,
                        Font::Small,
                        draw::SECONDARY,
                        RECT {
                            left: x,
                            top: 213,
                            right: x + w,
                            bottom: 233,
                        },
                        DT_SINGLELINE,
                    )
                } else {
                    self.link(p.dc, text, url, x, 213, Font::Small);
                }
                x += w;
            }
            p.text(
                "Mã nguồn mở theo giấy phép GPL-3.0",
                Font::Small,
                draw::SECONDARY,
                RECT {
                    left: 0,
                    top: 230,
                    right: width,
                    bottom: 250,
                },
                DT_CENTER | DT_SINGLELINE,
            );
            let segments = [("Ghi nhận", "ack:")];
            let total = segments
                .iter()
                .map(|(s, _)| draw::width(p.dc, s, Font::Small))
                .sum::<i32>();
            let mut x = (width - total) / 2;
            for (text, url) in segments {
                let w = draw::width(p.dc, text, Font::Small);
                if url.is_empty() {
                    p.text(
                        text,
                        Font::Small,
                        draw::SECONDARY,
                        RECT {
                            left: x,
                            top: 255,
                            right: x + w,
                            bottom: 275,
                        },
                        DT_SINGLELINE,
                    )
                } else {
                    self.link(p.dc, text, url, x, 255, Font::Small);
                }
                x += w;
            }
        }
    }
    fn hit(&self, x: i32, y: i32) -> Option<&str> {
        let scale = dpi::window(self.window);
        self.links
            .iter()
            .find(|l| scale.contains([l.rect.left, l.rect.top, l.rect.right, l.rect.bottom], x, y))
            .map(|l| l.url.as_str())
    }
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        let ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Info;
        if let Some(s) = ptr.as_mut() {
            match m {
                WM_PAINT => {
                    s.paint();
                    return LRESULT(0);
                }
                WM_ERASEBKGND => return LRESULT(1),
                WM_MOUSEWHEEL | WM_VSCROLL if s.credits => {
                    let rect = dpi::client_rect(h);
                    let max = (s.content - rect.bottom).max(0);
                    let value = if m == WM_MOUSEWHEEL {
                        s.scroll - ((w.0 >> 16) as u16 as i16 as i32) / 120 * 60
                    } else {
                        match w.0 & 65535 {
                            0 => s.scroll - 30,
                            1 => s.scroll + 30,
                            2 => s.scroll - rect.bottom,
                            3 => s.scroll + rect.bottom,
                            4 | 5 => (w.0 >> 16) as i32,
                            6 => 0,
                            7 => max,
                            _ => s.scroll,
                        }
                    };
                    s.scroll = value.clamp(0, max);
                    SetScrollPos(h, SB_VERT, s.scroll, true);
                    invalidate(h);
                    return LRESULT(0);
                }
                WM_SETCURSOR => {
                    let mut point = POINT::default();
                    let _ = GetCursorPos(&mut point);
                    let _ = ScreenToClient(h, &mut point);
                    SetCursor(
                        LoadCursorW(
                            None,
                            if s.hit(point.x, point.y).is_some() {
                                IDC_HAND
                            } else {
                                IDC_ARROW
                            },
                        )
                        .ok(),
                    );
                    return LRESULT(1);
                }
                WM_LBUTTONUP => {
                    if let Some(url) =
                        s.hit(l.0 as u16 as i16 as i32, (l.0 >> 16) as u16 as i16 as i32)
                    {
                        if url == "ack:" {
                            credits()
                        } else {
                            open_uri(url)
                        }
                    }
                    return LRESULT(0);
                }
                WM_CLOSE => {
                    let _ = DestroyWindow(h);
                    return LRESULT(0);
                }
                WM_NCDESTROY => {
                    let credits = s.credits;
                    SetWindowLongPtrW(h, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                    if credits {
                        CREDITS.with(|w| w.set(HWND::default()))
                    } else {
                        ABOUT.with(|w| w.set(HWND::default()))
                    }
                }
                _ => (),
            }
        }
        DefWindowProcW(h, m, w, l)
    })
}
fn show(credits: bool) {
    let existing = if credits {
        CREDITS.with(Cell::get)
    } else {
        ABOUT.with(Cell::get)
    };
    if !existing.is_invalid() {
        bring_front(existing);
        return;
    }
    register(
        if credits { "SeviAck" } else { "SeviAbout" },
        Some(proc),
        None,
    );
    let window = make_window(
        if credits { "SeviAck" } else { "SeviAbout" },
        if credits {
            "Ghi nhận"
        } else {
            "Giới thiệu Sevi"
        },
        if credits { 520 } else { 300 },
        if credits { 600 } else { 304 },
        false,
    );
    if credits {
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
            SetWindowLongPtrW(window, GWL_STYLE, (style | WS_VSCROLL.0) as isize);
        }
    }
    let info = Box::new(Info {
        window,
        credits,
        scroll: 0,
        content: 0,
        links: vec![],
    });
    unsafe {
        SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(info) as isize);
    }
    if credits {
        CREDITS.with(|w| w.set(window))
    } else {
        ABOUT.with(|w| w.set(window))
    }
    center(window);
    bring_front(window);
}
pub fn about() {
    show(false)
}
pub fn credits() {
    show(true)
}

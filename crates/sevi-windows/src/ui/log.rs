use super::{
    controls,
    draw::{self, Font},
    *,
};
use chrono::{Duration, Local};
use std::{cell::RefCell, collections::BTreeMap, fs};
use windows::Win32::UI::Controls::*;
const REFRESH: i32 = 300;
const OPEN: i32 = 301;
const CLEAR: i32 = 302;
const KEYS: i32 = 212;
struct Log {
    window: HWND,
    summary: HWND,
    grid: HWND,
    list: HWND,
}
thread_local! {static LOG:RefCell<Option<Log>>=const{RefCell::new(None)};}
fn state<T: Default>(f: impl FnOnce(&mut Log) -> T) -> T {
    LOG.with(|s| {
        s.try_borrow_mut()
            .ok()
            .and_then(|mut s| s.as_mut().map(f))
            .unwrap_or_default()
    })
}
fn stats(days: i64) -> BTreeMap<String, i32> {
    let from = if days > 0 {
        (Local::now().date_naive() - Duration::days(days - 1))
            .format("%Y-%m-%d")
            .to_string()
    } else {
        "0000".into()
    };
    let mut map = BTreeMap::new();
    for key in RegistryPreferences.keys() {
        if !key.starts_with("TLStat_")
            || key.len() < 19
            || key.get(7..17).is_none_or(|day| day < from.as_str())
        {
            continue;
        }
        let n = RegistryPreferences.int(&key, 0);
        *map.entry(key[18..].into()).or_default() += n;
    }
    map
}
fn column(list: HWND, index: i32, title: &str, width: i32, right: bool) {
    let mut text = wide(title);
    let column = LVCOLUMNW {
        mask: LVCF_TEXT | LVCF_WIDTH | LVCF_FMT,
        pszText: windows::core::PWSTR(text.as_mut_ptr()),
        cx: dpi::column(list, index, width),
        fmt: if right { LVCFMT_RIGHT } else { LVCFMT_LEFT },
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            list,
            LVM_INSERTCOLUMNW,
            Some(WPARAM(index as usize)),
            Some(LPARAM((&column as *const LVCOLUMNW) as isize)),
        );
    }
}
fn row(list: HWND, index: i32, text: &str, param: isize) {
    let mut text = wide(text);
    let item = LVITEMW {
        mask: LVIF_TEXT | LVIF_PARAM,
        iItem: index,
        pszText: windows::core::PWSTR(text.as_mut_ptr()),
        lParam: LPARAM(param),
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            list,
            LVM_INSERTITEMW,
            None,
            Some(LPARAM((&item as *const LVITEMW) as isize)),
        );
    }
}
fn cell(list: HWND, row: i32, column: i32, text: &str) {
    let mut text = wide(text);
    let item = LVITEMW {
        iSubItem: column,
        pszText: windows::core::PWSTR(text.as_mut_ptr()),
        ..Default::default()
    };
    unsafe {
        SendMessageW(
            list,
            LVM_SETITEMTEXTW,
            Some(WPARAM(row as usize)),
            Some(LPARAM((&item as *const LVITEMW) as isize)),
        );
    }
}
impl Log {
    fn refresh(&self) {
        host::flush_log();
        let periods = [stats(1), stats(7), stats(0)];
        let get = |p: &BTreeMap<String, i32>, key: &str| *p.get(key).unwrap_or(&0);
        let autos = |p: &BTreeMap<String, i32>| {
            get(p, "auto_vi_word_to_en") + get(p, "auto_en_word_to_vi") + get(p, "auto_double_key")
        };
        set_title(
            self.summary,
            &format!(
                "Hôm nay: {} lần chuyển tay, {} lần tự chuyển",
                get(&periods[0], "manual"),
                autos(&periods[0])
            ),
        );
        unsafe {
            SendMessageW(self.grid, LVM_DELETEALLITEMS, None, None);
        }
        for (i, (title, key)) in [
            ("Từ đã gõ", "words"),
            ("Chuyển tay", "manual"),
            ("Tự chuyển", "auto"),
            ("    Tiếng Việt → Anh (cả từ)", "auto_vi_word_to_en"),
            ("    Anh → Tiếng Việt (cả từ)", "auto_en_word_to_vi"),
            ("    Gõ dấu hai lần → Anh", "auto_double_key"),
            ("Hoàn tác bằng Delete", "undo"),
            ("Đổi theo ứng dụng / trang web", "context"),
            ("Chuyển tay mỗi 100 từ", "rate"),
        ]
        .iter()
        .enumerate()
        {
            row(self.grid, i as i32, title, 0);
            for (c, p) in periods.iter().enumerate() {
                let words = get(p, "words_vi") + get(p, "words_en");
                let text = match *key {
                    "words" => words.to_string(),
                    "auto" => autos(p).to_string(),
                    "rate" => {
                        if words == 0 {
                            "–".into()
                        } else {
                            format!("{:.1}", 100.0 * get(p, "manual") as f64 / words as f64)
                        }
                    }
                    _ => get(p, key).to_string(),
                };
                cell(self.grid, i as i32, c as i32 + 1, &text);
            }
        }
        unsafe {
            SendMessageW(self.list, LVM_DELETEALLITEMS, None, None);
        }
        let source = crate::prefs::data_dir()
            .ok()
            .and_then(|d| fs::read_to_string(d.join("typing-log.jsonl")).ok())
            .unwrap_or_default();
        for (i, line) in source
            .lines()
            .rev()
            .filter(|l| !l.is_empty())
            .take(300)
            .enumerate()
        {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let get = |key: &str| v[key].as_str().unwrap_or("");
            let event = get("e");
            let to = if get("to") == "vi" {
                "Tiếng Việt"
            } else {
                "Anh"
            };
            let time = get("t");
            let time = time.get(5..19).unwrap_or(time).replace('T', " ");
            let place = if get("site").is_empty() {
                get("app").into()
            } else {
                format!("{} · {}", get("app"), get("site"))
            };
            let (title, detail) = match event {
                "word" => (
                    if get("lang") == "vi" {
                        "Từ (Việt)"
                    } else {
                        "Từ (Anh)"
                    }
                    .into(),
                    format!("{}   [{}]", get("keys"), get("end")),
                ),
                "auto" => (
                    format!("Tự chuyển → {to}"),
                    match get("rule") {
                        "vi_word_to_en" => "từ tiếng Anh gõ khi đang ở Tiếng Việt",
                        "en_word_to_vi" => "từ tiếng Việt gõ khi đang ở Anh",
                        _ => "gõ dấu hai lần",
                    }
                    .into(),
                ),
                "manual" => (
                    format!("Chuyển tay → {to}"),
                    format!(
                        "{}{}",
                        if get("via") == "menu" {
                            "bằng menu"
                        } else {
                            "bằng phím tắt"
                        },
                        if v["after_auto"].as_bool() == Some(true) {
                            ", ngay sau tự chuyển"
                        } else {
                            ""
                        }
                    ),
                ),
                "undo" => (format!("Hoàn tác → {to}"), "bằng Delete".into()),
                "context" => (format!("Theo app/web → {to}"), String::new()),
                _ => (event.into(), String::new()),
            };
            row(
                self.list,
                i as i32,
                &time,
                if event == "manual" || event == "undo" {
                    1
                } else if event == "word" {
                    0
                } else {
                    2
                },
            );
            for (c, text) in [place, title, detail].iter().enumerate() {
                cell(self.list, i as i32, c as i32 + 1, text);
            }
        }
    }
}
unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    callback(|| unsafe {
        match m {
            WM_COMMAND => {
                match (w.0 & 65535) as i32 {
                    REFRESH => state(|s| s.refresh()),
                    OPEN => {
                        host::flush_log();
                        if let Ok(dir) = crate::prefs::data_dir() {
                            let path = dir.join("typing-log.jsonl");
                            if path.is_file() {
                                let args = wide(&format!("/select,\"{}\"", path.display()));
                                ShellExecuteW(
                                    None,
                                    w!("open"),
                                    w!("explorer.exe"),
                                    PCWSTR(args.as_ptr()),
                                    None,
                                    SW_SHOW,
                                );
                            } else {
                                open_uri(&dir.to_string_lossy());
                            }
                        }
                    }
                    CLEAR
                        if MessageBoxW(
                            Some(h),
                            w!("Số liệu thống kê và file log sẽ bị xoá."),
                            w!("Xoá toàn bộ nhật ký gõ?"),
                            MB_OKCANCEL | MB_ICONWARNING | MB_DEFBUTTON2,
                        ) == IDOK =>
                    {
                        host::clear_log();
                        for key in RegistryPreferences.keys() {
                            if key.starts_with("TLStat_") {
                                let _ = RegistryPreferences.remove(&key);
                            }
                        }
                        host::reload_settings();
                        state(|s| s.refresh());
                    }
                    KEYS => {
                        let _ = RegistryPreferences.set_int(
                            "TypingLogKeys",
                            i32::from(controls::checked(HWND(l.0 as *mut _))),
                        );
                        host::reload_settings();
                    }
                    _ => (),
                }
                return LRESULT(0);
            }
            WM_NOTIFY if l.0 != 0 => {
                let header = &*(l.0 as *const NMHDR);
                if state(|s| s.list == header.hwndFrom) && header.code == NM_CUSTOMDRAW {
                    let custom = &mut *(l.0 as *mut NMLVCUSTOMDRAW);
                    if custom.nmcd.dwDrawStage == CDDS_PREPAINT {
                        return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
                    }
                    if custom.nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                        custom.clrText = COLORREF(if custom.nmcd.lItemlParam.0 == 1 {
                            0x000095ff
                        } else if custom.nmcd.lItemlParam.0 == 2 {
                            draw::ACCENT
                        } else {
                            draw::TEXT
                        });
                        return LRESULT(CDRF_NEWFONT as isize);
                    }
                }
                return LRESULT(0);
            }
            WM_CTLCOLORSTATIC => {
                let dc = HDC(w.0 as *mut _);
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(
                    dc,
                    COLORREF(if state(|s| s.summary == HWND(l.0 as *mut _)) {
                        draw::TEXT
                    } else {
                        draw::SECONDARY
                    }),
                );
                return LRESULT(draw::background_brush(draw::GROUP).0 as isize);
            }
            WM_SIZE => {
                state(|s| {
                    let rect = dpi::client_rect(h);
                    dpi::move_control(
                        s.list,
                        20,
                        390,
                        rect.right - 40,
                        (rect.bottom - 446).max(30),
                    );
                    for (id, x, width) in [
                        (REFRESH, 20, 90),
                        (OPEN, 118, 100),
                        (CLEAR, rect.right - 120, 100),
                    ] {
                        if let Ok(button) = GetDlgItem(Some(h), id) {
                            dpi::move_control(button, x, rect.bottom - 44, width, 28);
                        }
                    }
                });
                return LRESULT(0);
            }
            WM_CLOSE => {
                let _ = DestroyWindow(h);
                return LRESULT(0);
            }
            WM_DESTROY => {
                LOG.with(|s| {
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
        state(|s| s.refresh());
        bring_front(existing);
        return;
    }
    register(
        "SeviLog",
        Some(proc),
        Some(draw::background_brush(draw::GROUP)),
    );
    let window = make_window("SeviLog", "Nhật ký gõ", 760, 600, true);
    let summary = child(
        window,
        "STATIC",
        "",
        WINDOW_STYLE(SS_LEFT.0),
        20,
        20,
        700,
        24,
        0,
        Font::Title,
    );
    let grid = child(
        window,
        "SysListView32",
        "",
        WINDOW_STYLE(LVS_REPORT | LVS_NOSORTHEADER | LVS_SINGLESEL),
        20,
        52,
        720,
        252,
        0,
        Font::Body,
    );
    unsafe {
        SendMessageW(
            grid,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            None,
            Some(LPARAM(LVS_EX_FULLROWSELECT as isize)),
        );
    }
    for (i, (title, width)) in [
        ("", 330),
        ("Hôm nay", 120),
        ("7 ngày", 120),
        ("Tất cả", 120),
    ]
    .iter()
    .enumerate()
    {
        column(grid, i as i32, title, *width, i != 0)
    }
    controls::toggle(
        window,
        20,
        314,
        KEYS,
        RegistryPreferences.int("TypingLogKeys", 1) != 0,
    );
    child(
        window,
        "STATIC",
        "Ghi lại phím gõ của từng từ",
        WINDOW_STYLE(SS_LEFT.0),
        68,
        316,
        500,
        20,
        0,
        Font::Body,
    );
    child(
        window,
        "STATIC",
        "Không bao giờ ghi trong ô mật khẩu, trình quản lý mật khẩu và Terminal.",
        WINDOW_STYLE(SS_LEFT.0),
        68,
        338,
        600,
        18,
        1,
        Font::Small,
    );
    child(
        window,
        "STATIC",
        "Sự kiện gần đây",
        WINDOW_STYLE(SS_LEFT.0),
        20,
        366,
        300,
        20,
        0,
        Font::Bold,
    );
    let list = child(
        window,
        "SysListView32",
        "",
        WINDOW_STYLE(LVS_REPORT | LVS_NOSORTHEADER | LVS_SINGLESEL),
        20,
        390,
        720,
        150,
        0,
        Font::Body,
    );
    unsafe {
        SendMessageW(
            list,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            None,
            Some(LPARAM(LVS_EX_FULLROWSELECT as isize)),
        );
    }
    for (i, (title, width)) in [
        ("Thời gian", 110),
        ("Ứng dụng / trang", 160),
        ("Sự kiện", 140),
        ("Chi tiết", 290),
    ]
    .iter()
    .enumerate()
    {
        column(list, i as i32, title, *width, false)
    }
    for (id, title, x, width) in [
        (REFRESH, "Làm mới", 20, 90),
        (OPEN, "Mở file log", 118, 100),
        (CLEAR, "Xoá log…", 640, 100),
    ] {
        child(
            window,
            "BUTTON",
            title,
            WINDOW_STYLE(0),
            x,
            556,
            width,
            28,
            id,
            Font::Body,
        );
    }
    LOG.with(|s| {
        *s.borrow_mut() = Some(Log {
            window,
            summary,
            grid,
            list,
        })
    });
    state(|s| s.refresh());
    center(window);
    bring_front(window);
}

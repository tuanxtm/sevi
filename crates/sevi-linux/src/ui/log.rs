use super::*;
use chrono::{Local, NaiveDate};
fn label(text: &str, class: &str) -> gtk::Label {
    let l = super::label(text, class);
    l.set_wrap(false);
    l
}
fn sum(p: &Prefs, days: i64, name: &str) -> i32 {
    let today = Local::now().date_naive();
    let p = p.borrow();
    p.keys()
        .iter()
        .filter(|k| k.starts_with("TLStat_") && k.len() > 18 && k.get(18..) == Some(name))
        .filter(|k| {
            days == 0
                || k.get(7..17)
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                    .is_some_and(|day| (today - day).num_days() < days)
        })
        .map(|k| p.int(k, 0))
        .sum()
}
pub(super) fn build(app: &adw::Application, p: Prefs) {
    let (win, tv) = window(app, "Nhật ký gõ", 720, 720);
    let b = vbox(12);
    margins(&b, 16);
    let autos = |d| {
        sum(&p, d, "auto_vi_word_to_en")
            + sum(&p, d, "auto_en_word_to_vi")
            + sum(&p, d, "auto_double_key")
    };
    b.append(&label(
        &format!(
            "Hôm nay: {} lần chuyển tay, {} lần tự chuyển",
            sum(&p, 1, "manual"),
            autos(1)
        ),
        "title-3",
    ));
    let grid = gtk::Grid::builder()
        .column_spacing(24)
        .row_spacing(4)
        .build();
    for (c, title) in ["", "Hôm nay", "7 ngày", "Tất cả"].iter().enumerate() {
        grid.attach(&label(title, "heading"), c as i32, 0, 1, 1)
    }
    for (r, (title, key)) in [
        ("Từ đã gõ", "words"),
        ("Chuyển tay", "manual"),
        ("Tự chuyển", "auto"),
        ("    Tiếng Việt → Anh (cả từ)", "auto_vi_word_to_en"),
        ("    Anh → Tiếng Việt (cả từ)", "auto_en_word_to_vi"),
        ("    Gõ dấu hai lần → Anh", "auto_double_key"),
        ("Hoàn tác bằng Delete", "undo"),
        ("Đổi theo ứng dụng", "context"),
        ("Chuyển tay mỗi 100 từ", "rate"),
    ]
    .iter()
    .enumerate()
    {
        let l = label(title, "");
        l.set_hexpand(true);
        grid.attach(&l, 0, r as i32 + 1, 1, 1);
        for (c, d) in [1, 7, 0].into_iter().enumerate() {
            let words = sum(&p, d, "words_vi") + sum(&p, d, "words_en");
            let v = match *key {
                "words" => words.to_string(),
                "auto" => autos(d).to_string(),
                "rate" => {
                    if words == 0 {
                        "–".into()
                    } else {
                        format!("{:.1}", 100.0 * sum(&p, d, "manual") as f64 / words as f64)
                    }
                }
                _ => sum(&p, d, key).to_string(),
            };
            let l = label(&v, "");
            l.set_xalign(1.0);
            grid.attach(&l, c as i32 + 1, r as i32 + 1, 1, 1);
        }
    }
    b.append(&grid);
    let h = hbox(10);
    let s = gtk::Switch::builder()
        .active(p.borrow().int("TypingLogKeys", 1) != 0)
        .build();
    s.connect_active_notify({
        let p = p.clone();
        move |s| {
            let _ = p
                .borrow_mut()
                .set_int("TypingLogKeys", i32::from(s.is_active()));
        }
    });
    h.append(&s);
    let col = vbox(2);
    col.append(&label("Ghi lại phím gõ của từng từ", ""));
    col.append(&label(
        "Không bao giờ ghi trong trình quản lý mật khẩu và Terminal.",
        "dim-label",
    ));
    h.append(&col);
    b.append(&h);
    b.append(&label("Sự kiện gần đây", "heading"));
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    let data = p.borrow().data_dir.clone();
    let source = std::fs::read_to_string(data.join("typing-log.jsonl")).unwrap_or_default();
    let mut count = 0;
    for line in source.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let get = |k: &str| v[k].as_str().unwrap_or("");
        let event = get("e");
        if ["cmd", "key", "reload", "reset"].contains(&event) {
            continue;
        }
        let to = if get("to") == "vi" {
            "Tiếng Việt"
        } else {
            "Anh"
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
                if get("via") == "menu" {
                    "bằng menu"
                } else {
                    "bằng phím tắt"
                }
                .into(),
            ),
            "undo" => (format!("Hoàn tác → {to}"), "bằng Delete".into()),
            "context" => (format!("Theo ứng dụng → {to}"), String::new()),
            _ => (event.into(), String::new()),
        };
        let t = get("t");
        let t = t.get(5..19).unwrap_or(t).replace('T', " ");
        let row = adw::ActionRow::builder()
            .title(format!("{t}  ·  {}  ·  {title}", get("app")))
            .subtitle(detail)
            .use_markup(false)
            .build();
        list.append(&row);
        count += 1;
        if count == 200 {
            break;
        }
    }
    let sc = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .min_content_height(220)
        .child(&list)
        .build();
    b.append(&sc);
    let buttons = hbox(8);
    let open = gtk::Button::builder()
        .label("Mở file log")
        .hexpand(true)
        .halign(gtk::Align::Start)
        .build();
    open.connect_clicked({
        let data = data.clone();
        move |_| {
            let _ = Command::new("xdg-open").arg(&data).spawn();
        }
    });
    let clear = gtk::Button::with_label("Xoá log…");
    clear.add_css_class("destructive-action");
    clear.connect_clicked(move |_| {
        let _ = sevi_runtime::typing_log::archive_logs(&data);
        let _ = p.borrow_mut().clear_stats();
        let _ = p.borrow().send_command("clear-log");
    });
    buttons.append(&open);
    buttons.append(&clear);
    b.append(&buttons);
    tv.set_content(Some(&b));
    win.present();
}

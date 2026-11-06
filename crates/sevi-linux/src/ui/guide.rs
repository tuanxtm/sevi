use super::*;
use std::{cell::Cell, time::Duration};
use unicode_normalization::UnicodeNormalization;
const NAMES: [&str; 5] = ["welcome", "setup", "practice", "hotkey", "emoji"];
const LINES: [(&str, &str); 3] = [
    ("order traf suwax nhes", "order trà sữa nhé"),
    ("meeting lucs ba giowf nhes", "meeting lúc ba giờ nhé"),
    ("thanks, camr own nhieeuf", "thanks, cảm ơn nhiều"),
];
fn normalize(s: &str) -> String {
    s.trim().to_lowercase().nfc().collect()
}
pub(super) fn build(app: &adw::Application, p: Prefs, then_main: bool) {
    let initial = std::env::var("SEVI_GUIDE_STEP")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
        .min(4);
    build_at(app, p, then_main, initial)
}
pub(super) fn build_at(app: &adw::Application, p: Prefs, then_main: bool, initial: usize) {
    let (win, tv) = window(app, "Sevi", 540, 517);
    win.set_resizable(false);
    let root = vbox(0);
    margins(&root, 28);
    root.set_margin_top(8);
    let head = hbox(14);
    head.set_margin_bottom(22);
    let icon = gtk::Image::from_icon_name("sevi");
    icon.set_pixel_size(48);
    head.append(&icon);
    let col = vbox(2);
    col.set_hexpand(true);
    col.set_valign(gtk::Align::Center);
    let brand = label("", "");
    brand.set_markup(
        "<span weight=\"bold\" size=\"18pt\">Se<span foreground=\"#4F46E5\">vi</span></span>",
    );
    col.append(&brand);
    col.append(&label(
        "Gõ tiếng Việt, tự chuyển Anh/Việt cho bạn.",
        "dim-label",
    ));
    head.append(&col);
    let dots = hbox(6);
    dots.add_css_class("dots");
    dots.set_valign(gtk::Align::Center);
    let marks: Vec<_> = (0..5)
        .map(|_| {
            let dot = hbox(0);
            dot.set_valign(gtk::Align::Center);
            dots.append(&dot);
            dot
        })
        .collect();
    head.append(&dots);
    root.append(&head);
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(150)
        .vexpand(true)
        .build();
    let pages: Vec<_> = (0..5)
        .map(|i| {
            let b = vbox(if i == 2 { 12 } else { 14 });
            b.set_valign(gtk::Align::Start);
            stack.add_named(&b, Some(NAMES[i]));
            b
        })
        .collect();
    let b = &pages[0];
    b.append(&label("Sevi dùng kiểu gõ Telex", "title-2"));
    b.append(&label("Sevi chỉ hỗ trợ Telex, kiểu gõ phổ biến nhất. Bạn gõ tiếng Anh bình thường, Sevi tự nhận ra và không bỏ dấu nhầm.","dim-label"));
    let grid = gtk::Grid::builder()
        .column_spacing(18)
        .row_spacing(8)
        .halign(gtk::Align::Start)
        .build();
    grid.add_css_class("telex-card");
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
        let row = hbox(6);
        row.append(&label(keys, "telex-key"));
        row.append(&label("→", "dim-label"));
        row.append(&label(result, ""));
        grid.attach(&row, (i % 4) as i32, (i / 4) as i32, 1, 1);
    }
    b.append(&grid);
    let b = &pages[1];
    b.append(&label("Thêm Sevi vào nguồn nhập", "title-2"));
    b.append(&label("Sevi là một bộ gõ IBus: thêm Sevi vào nguồn nhập để gõ tiếng Việt trong mọi ứng dụng. Mọi thứ xử lý ngay trên máy, không gửi đi đâu.","dim-label"));
    let steps = vbox(8);
    for (i, s) in [
        "Bấm nút bên dưới để mở Cài đặt › Bàn phím",
        "Bấm + trong Nguồn nhập, chọn Vietnamese › Vietnamese (Sevi)",
        "Quay lại đây, Sevi sẽ tự nhận ra",
    ]
    .iter()
    .enumerate()
    {
        let h = hbox(8);
        h.append(&label(&(i + 1).to_string(), "step-num"));
        h.append(&label(s, "dim-label"));
        steps.append(&h)
    }
    b.append(&steps);
    let setup = gtk::Button::builder()
        .label("Mở Cài đặt bàn phím")
        .halign(gtk::Align::Start)
        .build();
    setup.add_css_class("suggested-action");
    setup.add_css_class("pill");
    setup.connect_clicked(|_| {
        if Command::new("gnome-control-center")
            .arg("keyboard")
            .spawn()
            .is_err()
        {
            let _ = Command::new("ibus-setup").spawn();
        }
    });
    b.append(&setup);
    let b = &pages[2];
    b.append(&label("Gõ thử", "title-2"));
    let practice = label("", "dim-label");
    practice.set_lines(2);
    practice.set_height_request(40);
    practice.set_valign(gtk::Align::Start);
    b.append(&practice);
    let entries: Vec<gtk::Entry> = (0..3)
        .map(|_| gtk::Entry::builder().hexpand(true).build())
        .collect();
    for (i, (keys, result)) in LINES.into_iter().enumerate() {
        let item = vbox(4);
        let cap = hbox(6);
        cap.append(&label("Gõ", "dim-label"));
        cap.append(&label(keys, "telex-key"));
        cap.append(&label("→", "dim-label"));
        cap.append(&label(result, ""));
        item.append(&cap);
        let row = hbox(8);
        let mark = gtk::Image::from_icon_name("radio-symbolic");
        mark.set_pixel_size(20);
        mark.add_css_class("dim-label");
        row.append(&entries[i]);
        row.append(&mark);
        item.append(&row);
        b.append(&item);
        let was_ok = Cell::new(false);
        let next = entries.get(i + 1).cloned();
        entries[i].connect_changed(move |entry| {
            let ok = normalize(entry.text().as_str()) == normalize(result);
            mark.set_icon_name(Some(if ok {
                "emblem-ok-symbolic"
            } else {
                "radio-symbolic"
            }));
            if ok {
                mark.add_css_class("ok-mark");
                mark.remove_css_class("dim-label");
            } else {
                mark.remove_css_class("ok-mark");
                mark.add_css_class("dim-label");
            }
            if ok && !was_ok.replace(ok) {
                if let Some(next) = &next {
                    next.grab_focus();
                }
            } else {
                was_ok.set(ok)
            }
        });
        let next = entries[(i + 1).min(2)].clone();
        entries[i].connect_activate(move |_| {
            next.grab_focus();
        });
    }
    let b = &pages[3];
    b.append(&label("Phím chuyển Anh/Việt", "title-2"));
    b.append(&label("Sevi tự chuyển khi bạn gõ lẫn tiếng Anh. Khi muốn tự chuyển, bấm phím tắt này. Bấm vào ô để đổi phím.","dim-label"));
    let h = hbox(12);
    h.append(&settings::recorder(&p, false));
    let typing_now = label("", "dim-label");
    h.append(&typing_now);
    b.append(&h);
    let h = hbox(12);
    let l = label("Âm báo khi chuyển", "");
    l.set_hexpand(true);
    h.append(&l);
    let beep = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .active(hotkey(&p) & 0x8000 != 0)
        .build();
    beep.connect_active_notify({
        let p = p.clone();
        move |s| {
            let k = hotkey(&p) & !0x8000 | if s.is_active() { 0x8000 } else { 0 };
            let _ = p.borrow_mut().set_int("SwitchKeyStatus", k);
        }
    });
    h.append(&beep);
    b.append(&h);
    let b = &pages[4];
    let h = hbox(8);
    h.append(&label("Bảng emoji", "title-2"));
    h.append(&label("không bắt buộc", "dim-label"));
    b.append(&h);
    b.append(&label("Bấm một phím tắt để mở bảng emoji ở giữa màn hình. Tìm bằng tiếng Việt hay tiếng Anh đều được: \"ngựa\" hay \"horse\" đều ra 🐴.","dim-label"));
    let h = hbox(12);
    let l = label("Bật bảng emoji", "");
    l.set_hexpand(true);
    h.append(&l);
    let emoji = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .active(p.borrow().int("EmojiPicker", 0) != 0)
        .build();
    emoji.connect_active_notify({
        let p = p.clone();
        move |s| {
            let _ = p
                .borrow_mut()
                .set_int("EmojiPicker", i32::from(s.is_active()));
        }
    });
    h.append(&emoji);
    b.append(&h);
    let emoji_box = hbox(12);
    emoji_box.append(&label("Phím mở bảng emoji", ""));
    emoji_box.append(&settings::recorder(&p, true));
    b.append(&emoji_box);
    let emoji_hint = label(
        "Bấm một phím kèm Ctrl, Alt hoặc Super, ví dụ Ctrl+Alt+E.",
        "dim-label",
    );
    b.append(&emoji_hint);
    root.append(&stack);
    let footer = hbox(8);
    footer.set_margin_top(20);
    let back = gtk::Button::with_label("Quay lại");
    footer.append(&back);
    let spacer = hbox(0);
    spacer.set_hexpand(true);
    footer.append(&spacer);
    let wait = hbox(6);
    let spinner = gtk::Spinner::new();
    spinner.start();
    wait.append(&spinner);
    wait.append(&label("Đang chờ Sevi trong nguồn nhập…", "dim-label"));
    footer.append(&wait);
    let done = hbox(6);
    let ok = gtk::Image::from_icon_name("emblem-ok-symbolic");
    ok.add_css_class("ok-mark");
    done.append(&ok);
    done.append(&label("Đã thêm Sevi", "dim-label"));
    footer.append(&done);
    let next = gtk::Button::with_label("");
    next.add_css_class("suggested-action");
    footer.append(&next);
    root.append(&footer);
    let step = Rc::new(Cell::new(initial));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let step = step.clone();
        let p = p.clone();
        let stack = stack.clone();
        let next = next.clone();
        let back = back.clone();
        let entries = entries.clone();
        let previous = Cell::new(usize::MAX);
        move || {
            let i = step.get();
            stack.set_visible_child_name(NAMES[i]);
            if i == 2 && previous.get() != 2 {
                let entry = entries[0].clone();
                glib::timeout_add_local_once(Duration::from_millis(200), move || {
                    entry.grab_focus();
                });
            }
            previous.set(i);
            for (n, dot) in marks.iter().enumerate() {
                dot.remove_css_class(if n == i { "off" } else { "on" });
                dot.add_css_class(if n == i { "on" } else { "off" });
            }
            back.set_visible(i != 0);
            let configured = if std::env::var_os("SEVI_PREVIEW_SETUP").is_some() {
                0
            } else {
                sources()
            };
            wait.set_visible(i == 1 && configured == 0);
            done.set_visible(i == 1 && configured == 1);
            next.set_label(if i == 0 {
                "Bắt đầu"
            } else if i == 4 {
                "Xong"
            } else {
                "Tiếp tục"
            });
            let emoji_on = p.borrow().int("EmojiPicker", 0) != 0;
            let emoji_key = p.borrow().int("EmojiHotkey", 0);
            next.set_sensitive(
                !(i == 1 && configured == 0 || i == 4 && emoji_on && emoji_key == 0),
            );
            let vi = p.borrow().int("InputMethod", 1) != 0;
            practice.set_text(&if vi{"Gõ đúng các phím bên dưới, kể cả tiếng Anh. Sevi bỏ dấu cho tiếng Việt và để nguyên tiếng Anh.".into()}else{format!("Bạn đang ở tiếng Anh: bấm {} để sang tiếng Việt, rồi gõ các phím bên dưới.",crate::keys::hotkey_label(hotkey(&p)))});
            typing_now.set_text(if vi {
                "Đang gõ: Tiếng Việt"
            } else {
                "Đang gõ: English"
            });
            beep.set_active(hotkey(&p) & 0x8000 != 0);
            emoji_box.set_visible(emoji_on);
            emoji_hint.set_visible(emoji_on && emoji_key == 0);
        }
    });
    back.connect_clicked({
        let step = step.clone();
        let refresh = refresh.clone();
        move |_| {
            step.set(step.get().saturating_sub(1));
            refresh();
        }
    });
    next.connect_clicked({
        let p = p.clone();
        let app = app.clone();
        let win = win.clone();
        let step = step.clone();
        let refresh = refresh.clone();
        move |_| {
            let i = step.get();
            if i == 4 {
                let _ = p.borrow_mut().set_int("OnboardingDone", 1);
                if then_main {
                    settings::build(&app, p.clone())
                }
                win.destroy();
            } else {
                step.set(
                    if i == 0 && sources() == 1 && std::env::var_os("SEVI_PREVIEW_SETUP").is_none()
                    {
                        2
                    } else {
                        i + 1
                    },
                );
                refresh();
            }
        }
    });
    win.connect_close_request({
        let p = p.clone();
        move |_| {
            let _ = p.borrow_mut().set_int("OnboardingDone", 1);
            glib::Propagation::Proceed
        }
    });
    refresh();
    let weak = win.downgrade();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let _ = p.borrow_mut().reload();
        if step.get() == 1 && sources() == 1 && std::env::var_os("SEVI_PREVIEW_SETUP").is_none() {
            step.set(2)
        }
        refresh();
        glib::ControlFlow::Continue
    });
    tv.set_content(Some(&root));
    win.set_default_widget(Some(&next));
    win.present();
}

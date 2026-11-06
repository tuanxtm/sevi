use super::*;
use crate::keys;
use std::{cell::Cell, collections::BTreeMap, time::Duration};
pub(super) fn row(g: &adw::PreferencesGroup, title: &str, tip: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    let b = hbox(2);
    b.set_valign(gtk::Align::Center);
    let title_label = label(title, "");
    title_label.set_wrap(false);
    b.append(&title_label);
    let q = label("?", "tip-mark");
    q.set_wrap(false);
    let button = gtk::MenuButton::builder()
        .child(&q)
        .has_frame(false)
        .valign(gtk::Align::Center)
        .tooltip_text(tip)
        .build();
    button.add_css_class("tip");
    let pop = gtk::Popover::new();
    let text = label(tip, "");
    text.set_max_width_chars(36);
    margins(&text, 6);
    pop.set_child(Some(&text));
    button.set_popover(Some(&pop));
    b.append(&button);
    row.add_prefix(&b);
    g.add(&row);
    row
}
fn group(page: &adw::PreferencesPage, title: &str) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::builder().title(title).build();
    page.add(&g);
    g
}
pub(super) fn switch_row(
    g: &adw::PreferencesGroup,
    title: &str,
    tip: &str,
    key: &'static str,
    default: i32,
    p: &Prefs,
) -> gtk::Switch {
    let r = row(g, title, tip);
    let s = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .active(p.borrow().int(key, default) != 0)
        .build();
    let p = p.clone();
    s.connect_active_notify(move |s| {
        let _ = p.borrow_mut().set_int(key, i32::from(s.is_active()));
    });
    r.add_suffix(&s);
    r.set_activatable_widget(Some(&s));
    s
}
fn mods(state: gtk::gdk::ModifierType) -> i32 {
    use gtk::gdk::ModifierType as M;
    (i32::from(state.contains(M::CONTROL_MASK)) * 0x100)
        | (i32::from(state.contains(M::ALT_MASK)) * 0x200)
        | (i32::from(state.contains(M::SUPER_MASK)) * 0x400)
        | (i32::from(state.contains(M::SHIFT_MASK)) * 0x800)
}
fn modifier(key: u32) -> i32 {
    match key {
        0xffe1 | 0xffe2 => 0x800,
        0xffe3 | 0xffe4 => 0x100,
        0xffe7..=0xffea => 0x200,
        0xffeb | 0xffec => 0x400,
        _ => 0,
    }
}
pub(super) fn recorder(p: &Prefs, emoji: bool) -> gtk::Button {
    let recording = Rc::new(Cell::new(false));
    let pending = Rc::new(Cell::new(0i32));
    let value = move |p: &Prefs| {
        if emoji {
            p.borrow().int("EmojiHotkey", 0)
        } else {
            hotkey(p)
        }
    };
    let b = gtk::Button::builder()
        .label(keys::hotkey_label(value(p) & !0x8000))
        .width_request(130)
        .valign(gtk::Align::Center)
        .build();
    let p = p.clone();
    let refresh: Rc<dyn Fn()> = Rc::new({
        let b = b.downgrade();
        let p = p.clone();
        let recording = recording.clone();
        move || {
            if let Some(b) = b.upgrade() {
                b.set_label(
                    if recording.get() {
                        "Bấm phím tắt…".into()
                    } else {
                        keys::hotkey_label(value(&p) & !0x8000)
                    }
                    .as_str(),
                );
            }
        }
    });
    let stop: Rc<dyn Fn()> = Rc::new({
        let recording = recording.clone();
        let pending = pending.clone();
        let refresh = refresh.clone();
        move || {
            recording.set(false);
            pending.set(0);
            refresh();
        }
    });
    let save: Rc<dyn Fn(i32)> = Rc::new({
        let p = p.clone();
        move |key| {
            if emoji {
                if key != (hotkey(&p) & !0x8000) {
                    let _ = p.borrow_mut().set_int("EmojiHotkey", key & !0x8000);
                }
            } else {
                let beep = hotkey(&p) & 0x8000;
                let _ = p.borrow_mut().set_int("SwitchKeyStatus", key | beep);
            }
        }
    });
    b.connect_clicked({
        let recording = recording.clone();
        let pending = pending.clone();
        let refresh = refresh.clone();
        move |b| {
            recording.set(!recording.get());
            pending.set(0);
            refresh();
            b.grab_focus();
        }
    });
    let controller = gtk::EventControllerKey::new();
    controller.connect_key_pressed({
        let recording = recording.clone();
        let pending = pending.clone();
        let save = save.clone();
        let stop = stop.clone();
        move |_, key, _, state| {
            if !recording.get() {
                return glib::Propagation::Proceed;
            }
            let key = key.into_glib();
            let m = mods(state);
            if key == 0xff1b && m == 0 {
                stop();
                return glib::Propagation::Stop;
            }
            let bit = modifier(key);
            if bit != 0 {
                pending.set(pending.get() | bit | m)
            } else if m != 0
                && let Some((k, _)) = keys::engine_key(key).filter(|(k, _)| *k <= 50)
            {
                save(k as i32 | m);
                stop();
            }
            glib::Propagation::Stop
        }
    });
    controller.connect_key_released({
        let recording = recording.clone();
        let pending = pending.clone();
        let save = save.clone();
        let stop = stop.clone();
        move |_, key, _, state| {
            if !recording.get() {
                return;
            }
            let bit = modifier(key.into_glib());
            if bit == 0 || mods(state) & !bit != 0 {
                return;
            }
            if !emoji && pending.get().count_ones() >= 2 {
                save(0xfe | pending.get());
                stop();
            } else {
                pending.set(0)
            }
        }
    });
    b.add_controller(controller);
    let focus = gtk::EventControllerFocus::new();
    focus.connect_leave(move |_| stop());
    b.add_controller(focus);
    b.connect_map({
        let recording = recording.clone();
        let refresh = refresh.clone();
        let p = p.clone();
        move |b| {
            if emoji && value(&p) & !0x8000 == 0 {
                recording.set(true);
                refresh();
                b.grab_focus();
            }
        }
    });
    let weak = b.downgrade();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        refresh();
        glib::ControlFlow::Continue
    });
    b
}
use glib::translate::IntoGlib;
pub(super) fn build(app: &adw::Application, p: Prefs) {
    let (win, tv) = window(app, "Sevi", 480, 860);
    if let Some(display) = gtk::gdk::Display::default()
        && let Some(m) = display
            .monitors()
            .item(0)
            .and_downcast::<gtk::gdk::Monitor>()
    {
        win.set_default_height(860.min(m.geometry().height() - 110));
    }
    let page = adw::PreferencesPage::new();
    let g0 = group(&page, "");
    let head = hbox(12);
    head.set_margin_start(12);
    head.set_margin_end(12);
    head.set_margin_bottom(8);
    let icon = gtk::Image::from_icon_name("sevi");
    icon.set_pixel_size(44);
    head.append(&icon);
    let col = vbox(2);
    col.set_hexpand(true);
    col.set_valign(gtk::Align::Center);
    let brand = label("", "");
    let color = if adw::StyleManager::default().is_dark() {
        "#8C87FF"
    } else {
        "#4F46E5"
    };
    brand.set_markup(&format!(
        "<span weight=\"bold\" size=\"16pt\">Se<span foreground=\"{color}\">vi</span></span>"
    ));
    col.append(&brand);
    let hint = label("", "dim-label");
    col.append(&hint);
    let setup = gtk::Button::with_label("Thêm Sevi vào nguồn nhập…");
    setup.add_css_class("link");
    setup.set_halign(gtk::Align::Start);
    setup.connect_clicked(|_| spawn("--guide"));
    col.append(&setup);
    head.append(&col);
    let seg = hbox(0);
    seg.add_css_class("linked");
    seg.set_valign(gtk::Align::Center);
    let vi = gtk::ToggleButton::with_label("Tiếng Việt");
    let en = gtk::ToggleButton::with_label("English");
    en.set_group(Some(&vi));
    vi.set_active(p.borrow().int("InputMethod", 1) != 0);
    en.set_active(p.borrow().int("InputMethod", 1) == 0);
    for (b, is_vi) in [(&vi, true), (&en, false)] {
        let p = p.clone();
        b.connect_toggled(move |b| {
            if b.is_active() && (p.borrow().int("InputMethod", 1) != 0) != is_vi {
                let _ = p.borrow().send_command(if is_vi {
                    "lang vi user"
                } else {
                    "lang en user"
                });
            }
        });
        seg.append(b)
    }
    head.append(&seg);
    g0.add(&head);
    let g = group(&page, "");
    let r = row(
        &g,
        "Phím chuyển Anh/Việt",
        "Bấm tổ hợp phím này để chuyển giữa tiếng Việt và tiếng Anh. Bấm vào ô để đặt phím khác.",
    );
    r.add_suffix(&recorder(&p, false));
    let r = row(
        &g,
        "Âm báo khi chuyển",
        "Phát một âm ngắn mỗi lần bạn bấm phím chuyển Anh/Việt: lên giọng khi sang tiếng Việt, xuống giọng khi sang tiếng Anh.",
    );
    let beep = gtk::Switch::builder()
        .active(hotkey(&p) & 0x8000 != 0)
        .valign(gtk::Align::Center)
        .build();
    beep.connect_active_notify({
        let p = p.clone();
        move |s| {
            let k = hotkey(&p) & !0x8000 | if s.is_active() { 0x8000 } else { 0 };
            let _ = p.borrow_mut().set_int("SwitchKeyStatus", k);
        }
    });
    r.add_suffix(&beep);
    r.set_activatable_widget(Some(&beep));
    let mut switches = BTreeMap::new();
    for (title, rows) in [
        (
            "Tự chuyển",
            vec![
                (
                    "Tự chuyển Anh/Việt",
                    "Gõ một từ tiếng Anh khi đang ở tiếng Việt (hoặc ngược lại), Sevi tự sửa từ đó và chuyển chế độ. Bấm Xoá ngay sau đó để hoàn tác.",
                    "AutoSwitchByTyping",
                    1,
                ),
                (
                    "Tạm dừng 5 giây",
                    "Khi bạn tự bấm chuyển Anh/Việt, Sevi không tự chuyển trong 5 giây, để giữ đúng lựa chọn của bạn.",
                    "PauseAutoAfterManual",
                    1,
                ),
                (
                    "Nhớ theo ứng dụng",
                    "Sevi nhớ bạn dùng tiếng Anh hay tiếng Việt ở mỗi ứng dụng và tự chuyển lại khi bạn quay về ứng dụng đó. Trên Wayland chỉ với ứng dụng X11.",
                    "UseSmartSwitchKey",
                    1,
                ),
            ],
        ),
        (
            "Gõ tiếng Việt",
            vec![
                (
                    "Kiểm tra chính tả",
                    "Chỉ bỏ dấu khi từ đang gõ đúng quy tắc chính tả tiếng Việt.",
                    "Spelling",
                    1,
                ),
                (
                    "Trả lại phím khi sai",
                    "Khi gõ xong một từ sai chính tả tiếng Việt, Sevi trả lại đúng các phím bạn đã gõ.",
                    "RestoreIfInvalidWord",
                    1,
                ),
            ],
        ),
    ] {
        let g = group(&page, title);
        if title == "Gõ tiếng Việt" {
            for (title, tip, key, values, modern) in [
                (
                    "Kiểu gõ",
                    "Telex: [ ra ơ, ] ra ư, w ra ư. Telex đơn giản 1: [ ] là dấu ngoặc, w đứng riêng vẫn là w. Telex đơn giản 2: [ ] là dấu ngoặc, w vẫn ra ư. Cả ba kiểu đều gõ uw, ow, aw ra ư, ơ, ă.",
                    "TelexStyle",
                    vec!["Telex", "Telex đơn giản 1", "Telex đơn giản 2"],
                    false,
                ),
                (
                    "Đặt dấu",
                    "Vị trí dấu thanh với oa, oe, uy: kiểu mới (hoà, thuý) hoặc kiểu cũ (hòa, thúy).",
                    "ModernOrthography",
                    vec!["hoà, thuý", "hòa, thúy"],
                    true,
                ),
            ] {
                let r = row(&g, title, tip);
                let dd = gtk::DropDown::from_strings(&values);
                dd.set_valign(gtk::Align::Center);
                let n = p.borrow().int(key, if modern { 1 } else { 0 });
                dd.set_selected(if modern {
                    u32::from(n == 0)
                } else {
                    n.clamp(0, 2) as u32
                });
                dd.connect_selected_notify({
                    let p = p.clone();
                    move |dd| {
                        let n = if modern {
                            i32::from(dd.selected() == 0)
                        } else {
                            dd.selected() as i32
                        };
                        let _ = p.borrow_mut().set_int(key, n);
                    }
                });
                r.add_suffix(&dd)
            }
        }
        for (title, tip, key, default) in rows {
            switches.insert(key, switch_row(&g, title, tip, key, default, &p));
        }
    }
    let g = group(&page, "");
    let emoji = switch_row(
        &g,
        "Bảng emoji",
        "Bấm phím tắt để mở bảng emoji ở giữa màn hình. Tìm bằng tiếng Việt hoặc tiếng Anh, gõ Telex vẫn tìm được: \"ngựa\", \"horse\" đều ra 🐴. Enter để chèn, Esc để đóng.",
        "EmojiPicker",
        0,
        &p,
    );
    let er = row(
        &g,
        "Phím mở bảng emoji",
        "Một phím kèm Ctrl, Alt hoặc Super, ví dụ Ctrl+Alt+E. Bấm vào ô để đặt phím khác.",
    );
    er.add_suffix(&recorder(&p, true));
    er.set_visible(emoji.is_active());
    emoji.connect_active_notify(move |s| er.set_visible(s.is_active()));
    let g = group(&page, "");
    let login = gtk::Switch::builder()
        .valign(gtk::Align::Center)
        .sensitive(false)
        .build();
    let r = row(
        &g,
        "Mở Sevi khi đăng nhập",
        "IBus tự mở Sevi mỗi khi bạn đăng nhập, khi Sevi có trong Cài đặt › Bàn phím › Nguồn nhập.",
    );
    r.add_suffix(&login);
    switch_row(
        &g,
        "Tự động cập nhật",
        "Tự động cập nhật chỉ hoạt động khi đã cấu hình nguồn cập nhật riêng cho Sevi.",
        "AutoUpdate",
        1,
        &p,
    );
    for (parent, child) in [
        ("AutoSwitchByTyping", "PauseAutoAfterManual"),
        ("Spelling", "RestoreIfInvalidWord"),
    ] {
        let s = &switches[parent];
        let c = switches[child].clone();
        c.set_sensitive(s.is_active());
        s.connect_active_notify(move |s| c.set_sensitive(s.is_active()));
    }
    let refresh = move || {
        let _ = p.borrow_mut().reload();
        let configured = sources() != 0;
        hint.set_visible(configured);
        hint.set_text(&format!("Bấm {} để chuyển", keys::hotkey_label(hotkey(&p))));
        setup.set_visible(!configured);
        login.set_active(configured);
        vi.set_active(p.borrow().int("InputMethod", 1) != 0);
        en.set_active(p.borrow().int("InputMethod", 1) == 0);
    };
    refresh();
    let weak = win.downgrade();
    glib::timeout_add_local(Duration::from_secs(2), move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        refresh();
        glib::ControlFlow::Continue
    });
    tv.set_content(Some(&page));
    win.present();
}

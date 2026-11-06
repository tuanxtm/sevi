use super::*;
use glib::translate::IntoGlib;
use sevi_runtime::emoji::{EmojiIndex, convert_telex, frequent, record_usage};
use std::{cell::Cell, collections::BTreeMap, time::Duration};
const COLS: usize = 9;
const CELL: f64 = 44.0;
const PAD: f64 = 12.0;
const VI: [&str; 10] = [
    "Mặt cười & cảm xúc",
    "Người & cơ thể",
    "",
    "Động vật & thiên nhiên",
    "Đồ ăn & thức uống",
    "Du lịch & địa điểm",
    "Hoạt động",
    "Đồ vật",
    "Biểu tượng",
    "Cờ",
];
const EN: [&str; 10] = [
    "Smileys & Emotion",
    "People & Body",
    "",
    "Animals & Nature",
    "Food & Drink",
    "Travel & Places",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];
const TONES: [&str; 6] = ["✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"];
#[derive(Clone)]
struct Row {
    title: String,
    y: f64,
    height: f64,
    start: usize,
    count: usize,
}
struct Picker {
    index: EmojiIndex,
    all: Vec<usize>,
    recents: Vec<usize>,
    visible: Vec<usize>,
    rows: Vec<Row>,
    raw: String,
    selected: usize,
    tone: usize,
    vietnamese: bool,
    all_selected: bool,
    content: f64,
}
impl Picker {
    fn layout(&mut self) {
        self.rows.clear();
        self.visible.clear();
        let mut y = 0.0;
        let groups = if self.raw.trim().is_empty() {
            let mut groups = vec![(
                if self.vietnamese {
                    "Dùng thường xuyên"
                } else {
                    "Frequently Used"
                }
                .to_owned(),
                self.recents.clone(),
            )];
            let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
            for &i in &self.all {
                grouped.entry(self.index.all[i].group).or_default().push(i);
            }
            for (g, items) in grouped {
                groups.push((
                    (if self.vietnamese { &VI } else { &EN })
                        .get(g)
                        .unwrap_or(&"")
                        .to_string(),
                    items,
                ));
            }
            groups
        } else {
            vec![(
                String::new(),
                self.index.search(&self.raw, 200, Some(&self.all)),
            )]
        };
        for (title, items) in groups {
            if items.is_empty() {
                continue;
            }
            if !title.is_empty() {
                let height = if self.rows.is_empty() { 20.0 } else { 28.0 };
                self.rows.push(Row {
                    title,
                    y,
                    height,
                    start: 0,
                    count: 0,
                });
                y += height
            }
            let start = self.visible.len();
            let n = items.len();
            self.visible.extend(items);
            for i in (0..n).step_by(COLS) {
                self.rows.push(Row {
                    title: String::new(),
                    y,
                    height: CELL,
                    start: start + i,
                    count: COLS.min(n - i),
                });
                y += CELL
            }
        }
        self.content = y;
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
    fn selected_row(&self) -> Option<&Row> {
        self.rows
            .iter()
            .find(|r| r.count > 0 && (r.start..r.start + r.count).contains(&self.selected))
    }
    fn at(&self, x: f64, y: f64) -> Option<usize> {
        let col = ((x - PAD) / CELL).floor() as isize;
        self.rows
            .iter()
            .find(|r| r.count > 0 && y >= r.y && y < r.y + r.height)
            .and_then(|r| {
                if col >= 0 && (col as usize) < r.count {
                    Some(r.start + col as usize)
                } else {
                    None
                }
            })
    }
    fn move_row(&mut self, delta: isize) {
        let rows: Vec<_> = self.rows.iter().filter(|r| r.count > 0).collect();
        if let Some(i) = rows
            .iter()
            .position(|r| (r.start..r.start + r.count).contains(&self.selected))
        {
            let target = i as isize + delta;
            if target >= 0 && (target as usize) < rows.len() {
                let old = rows[i];
                let new = rows[target as usize];
                self.selected = new.start + (self.selected - old.start).min(new.count - 1)
            }
        }
    }
}
fn usage(p: &Prefs) -> BTreeMap<String, f64> {
    p.borrow()
        .string("EmojiRecents")
        .split('|')
        .filter_map(|s| {
            let (k, v) = s.rsplit_once('=')?;
            Some((k.to_owned(), v.parse().ok()?))
        })
        .collect()
}
fn max_version() -> f64 {
    use std::ffi::{c_int, c_uint, c_void};
    #[link(name = "fontconfig")]
    unsafe extern "C" {
        fn FcNameParse(s: *const u8) -> *mut c_void;
        fn FcConfigSubstitute(config: *mut c_void, pattern: *mut c_void, kind: c_int) -> c_int;
        fn FcDefaultSubstitute(p: *mut c_void);
        fn FcFontMatch(c: *mut c_void, p: *mut c_void, result: *mut c_int) -> *mut c_void;
        fn FcPatternGetCharSet(
            p: *mut c_void,
            name: *const std::ffi::c_char,
            id: c_int,
            cs: *mut *mut c_void,
        ) -> c_int;
        fn FcCharSetHasChar(cs: *const c_void, c: c_uint) -> c_int;
        fn FcPatternDestroy(p: *mut c_void);
    }
    unsafe {
        let p = FcNameParse(c"emoji:color".as_ptr().cast());
        if p.is_null() {
            return 14.0;
        }
        FcConfigSubstitute(std::ptr::null_mut(), p, 0);
        FcDefaultSubstitute(p);
        let mut result = 0;
        let m = FcFontMatch(std::ptr::null_mut(), p, &mut result);
        let mut cs = std::ptr::null_mut();
        let mut version = 14.0;
        if !m.is_null() {
            if FcPatternGetCharSet(m, c"charset".as_ptr(), 0, &mut cs) == 0 && !cs.is_null() {
                for (ch, v) in [(0x1faea, 17.0), (0x1fae9, 16.0), (0x1fae8, 15.1)] {
                    if FcCharSetHasChar(cs, ch) != 0 {
                        version = v;
                        break;
                    }
                }
            }
            FcPatternDestroy(m)
        }
        FcPatternDestroy(p);
        version
    }
}
pub(super) fn build(app: &adw::Application, p: Prefs) {
    let index = std::env::var("SEVI_EMOJI_JSON")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|s| EmojiIndex::parse(&s).ok())
        .unwrap_or_default();
    let version = max_version();
    let all: Vec<usize> = index
        .all
        .iter()
        .enumerate()
        .filter(|(_, e)| e.version <= version)
        .map(|(i, _)| i)
        .collect();
    let by_char: BTreeMap<_, _> = all
        .iter()
        .map(|&i| (index.all[i].text.clone(), i))
        .collect();
    let recents = frequent(&usage(&p), 27)
        .iter()
        .filter_map(|s| by_char.get(s).copied())
        .collect();
    let vietnamese = p.borrow().int("InputMethod", 1) != 0;
    let tone = p.borrow().int("EmojiSkinTone", 0).clamp(0, 5) as usize;
    let picker = Rc::new(RefCell::new(Picker {
        index,
        all,
        recents,
        visible: vec![],
        rows: vec![],
        raw: String::new(),
        selected: 0,
        tone,
        vietnamese,
        all_selected: false,
        content: 0.0,
    }));
    picker.borrow_mut().layout();
    let win = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Sevi Emoji")
        .decorated(false)
        .resizable(false)
        .default_width(460)
        .default_height(444)
        .build();
    win.remove_css_class("background");
    win.add_css_class("sevi-emoji");
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("emoji.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        )
    }
    let panel = vbox(0);
    panel.add_css_class("emoji-panel");
    panel.set_size_request(420, 404);
    panel.set_overflow(gtk::Overflow::Hidden);
    let search = hbox(8);
    margins(&search, 14);
    search.set_margin_bottom(8);
    let mag = gtk::Image::from_icon_name("system-search-symbolic");
    mag.add_css_class("dim-label");
    search.append(&mag);
    let text_box = hbox(1);
    text_box.add_css_class("emoji-search");
    text_box.set_hexpand(true);
    let display = label("", "");
    display.set_ellipsize(gtk::pango::EllipsizeMode::Start);
    text_box.append(&display);
    let caret = hbox(0);
    caret.add_css_class("emoji-caret");
    caret.set_valign(gtk::Align::Center);
    text_box.append(&caret);
    let placeholder = label("", "dim-label");
    placeholder.set_margin_start(2);
    text_box.append(&placeholder);
    search.append(&text_box);
    let badge = label("", "emoji-badge");
    badge.set_valign(gtk::Align::Center);
    search.append(&badge);
    panel.append(&search);
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let overlay = gtk::Overlay::builder()
        .vexpand(true)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    let area = gtk::DrawingArea::builder().vexpand(true).build();
    let adj = gtk::Adjustment::new(0.0, 0.0, 1.0, CELL, CELL * 4.0, 1.0);
    let scroll = gtk::Scrollbar::new(gtk::Orientation::Vertical, Some(&adj));
    scroll.set_halign(gtk::Align::End);
    scroll.add_css_class("overlay-indicator");
    overlay.set_child(Some(&area));
    overlay.add_overlay(&scroll);
    let empty = label("", "dim-label");
    empty.set_halign(gtk::Align::Center);
    empty.set_valign(gtk::Align::Start);
    empty.set_margin_top(110);
    overlay.add_overlay(&empty);
    panel.append(&overlay);
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let footer = hbox(10);
    footer.set_height_request(44);
    footer.set_margin_start(14);
    footer.set_margin_end(14);
    let current = label("", "emoji-cur");
    footer.append(&current);
    let name = label("", "dim-label");
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_hexpand(true);
    footer.append(&name);
    footer.append(&label("Sevi", "brand"));
    footer.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let tone_button = gtk::MenuButton::builder()
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    tone_button.add_css_class("emoji-tone");
    let tone_pop = gtk::Popover::new();
    let tones = hbox(2);
    tone_pop.set_child(Some(&tones));
    tone_button.set_popover(Some(&tone_pop));
    footer.append(&tone_button);
    panel.append(&footer);
    win.set_child(Some(&panel));
    let refresh: Rc<dyn Fn()> = Rc::new({
        let picker = picker.clone();
        let adj = adj.clone();
        let area = area.clone();
        let tone_button = tone_button.clone();
        move || {
            let k = picker.borrow();
            let shown = if k.vietnamese {
                convert_telex(&k.raw)
            } else {
                k.raw.clone()
            };
            display.set_text(&shown);
            if k.all_selected && !k.raw.is_empty() {
                display.add_css_class("emoji-allsel")
            } else {
                display.remove_css_class("emoji-allsel")
            }
            placeholder.set_text(if k.vietnamese {
                "Tìm emoji: ngựa, horse…"
            } else {
                "Search emoji: horse, ngựa…"
            });
            placeholder.set_visible(k.raw.is_empty());
            badge.set_text(if k.vietnamese { "VI" } else { "EN" });
            empty.set_text(if k.vietnamese {
                "Không tìm thấy emoji nào"
            } else {
                "No emoji found"
            });
            empty.set_visible(!k.raw.trim().is_empty() && k.visible.is_empty());
            if let Some(e) = k.visible.get(k.selected).map(|&i| &k.index.all[i]) {
                current.set_text(e.with_skin(k.tone));
                name.set_text(if k.vietnamese { &e.name_vi } else { &e.name_en });
            } else {
                current.set_text("");
                name.set_text("");
            }
            tone_button.set_label(TONES[k.tone]);
            tone_button.set_tooltip_text(Some(if k.vietnamese { "Màu da" } else { "Skin tone" }));
            adj.set_upper(k.content.max(1.0));
            scroll.set_visible(k.content > adj.page_size());
            area.queue_draw();
        }
    });
    for (t, text) in TONES.iter().enumerate() {
        let b = gtk::Button::builder().label(*text).has_frame(false).build();
        b.add_css_class("emoji-tone-item");
        b.connect_clicked({
            let picker = picker.clone();
            let p = p.clone();
            let refresh = refresh.clone();
            let tone_pop = tone_pop.clone();
            move |_| {
                picker.borrow_mut().tone = t;
                let _ = p.borrow_mut().set_int("EmojiSkinTone", t as i32);
                tone_pop.popdown();
                refresh();
            }
        });
        tones.append(&b)
    }
    let choose: Rc<dyn Fn()> = Rc::new({
        let picker = picker.clone();
        let p = p.clone();
        let win = win.clone();
        let app = app.clone();
        move || {
            let k = picker.borrow();
            let Some(e) = k.visible.get(k.selected).map(|&i| &k.index.all[i]) else {
                return;
            };
            let mut scores = usage(&p);
            record_usage(&mut scores, &e.text);
            let value = scores
                .iter()
                .map(|(s, n)| format!("{s}={n:.4}"))
                .collect::<Vec<_>>()
                .join("|");
            let _ = p.borrow_mut().set("EmojiRecents", value.into());
            let _ = p
                .borrow()
                .send_command(&format!("emoji {}", e.with_skin(k.tone)));
            win.set_visible(false);
            app.quit();
        }
    });
    area.set_draw_func({
        let picker = picker.clone();
        let adj = adj.clone();
        move |area, cr, width, _| {
            let k = picker.borrow();
            let offset = adj.value();
            let page = adj.page_size();
            cr.translate(0.0, -offset);
            let color = area.color();
            let layout = area.create_pango_layout(None);
            let mut emoji_font = gtk::pango::FontDescription::new();
            emoji_font.set_absolute_size(28.0 * gtk::pango::SCALE as f64);
            let mut header_font = gtk::pango::FontDescription::new();
            header_font.set_absolute_size(11.0 * gtk::pango::SCALE as f64);
            header_font.set_weight(gtk::pango::Weight::Semibold);
            for row in &k.rows {
                if row.y + row.height < offset || row.y > offset + page {
                    continue;
                }
                if row.count == 0 {
                    layout.set_font_description(Some(&header_font));
                    layout.set_text(&row.title);
                    cr.set_source_rgba(
                        color.red() as f64,
                        color.green() as f64,
                        color.blue() as f64,
                        0.55,
                    );
                    cr.move_to(PAD, row.y + row.height - 20.0);
                    pangocairo::functions::show_layout(cr, &layout);
                    continue;
                }
                let cell = (width as f64 - 2.0 * PAD) / COLS as f64;
                for col in 0..row.count {
                    let i = row.start + col;
                    let x = PAD + col as f64 * cell;
                    if i == k.selected {
                        cr.set_source_rgba(0.310, 0.275, 0.898, 0.22);
                        let r = 8.0;
                        let y = row.y;
                        cr.new_sub_path();
                        cr.arc(x + cell - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
                        cr.arc(
                            x + cell - r,
                            y + CELL - r,
                            r,
                            0.0,
                            std::f64::consts::FRAC_PI_2,
                        );
                        cr.arc(
                            x + r,
                            y + CELL - r,
                            r,
                            std::f64::consts::FRAC_PI_2,
                            std::f64::consts::PI,
                        );
                        cr.arc(
                            x + r,
                            y + r,
                            r,
                            std::f64::consts::PI,
                            3.0 * std::f64::consts::FRAC_PI_2,
                        );
                        cr.close_path();
                        let _ = cr.fill();
                    }
                    let e = &k.index.all[k.visible[i]];
                    layout.set_font_description(Some(&emoji_font));
                    layout.set_text(e.with_skin(k.tone));
                    let (w, h) = layout.pixel_size();
                    cr.set_source_rgba(
                        color.red() as f64,
                        color.green() as f64,
                        color.blue() as f64,
                        1.0,
                    );
                    cr.move_to(x + (cell - w as f64) / 2.0, row.y + (CELL - h as f64) / 2.0);
                    pangocairo::functions::show_layout(cr, &layout);
                }
            }
        }
    });
    area.connect_resize({
        let adj = adj.clone();
        let refresh = refresh.clone();
        move |_, _, height| {
            adj.set_page_size(height as f64);
            adj.set_page_increment(height as f64);
            refresh();
        }
    });
    adj.connect_value_changed({
        let area = area.clone();
        move |_| area.queue_draw()
    });
    let scroll_controller = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::KINETIC,
    );
    scroll_controller.connect_scroll({
        let adj = adj.clone();
        move |controller, _, dy| {
            let step = if controller.unit() == gtk::gdk::ScrollUnit::Wheel {
                CELL * 1.5
            } else {
                1.0
            };
            adj.set_value(
                (adj.value() + dy * step).clamp(0.0, (adj.upper() - adj.page_size()).max(0.0)),
            );
            glib::Propagation::Stop
        }
    });
    area.add_controller(scroll_controller);
    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion({
        let picker = picker.clone();
        let adj = adj.clone();
        let refresh = refresh.clone();
        let last = Cell::new(None);
        move |_, x, y| {
            let previous = last.replace(Some((x, y)));
            if previous.is_none() || previous == Some((x, y)) {
                return;
            }
            let i = picker.borrow().at(x, y + adj.value());
            if let Some(i) = i {
                picker.borrow_mut().selected = i;
                refresh();
            }
        }
    });
    area.add_controller(motion);
    let click = gtk::GestureClick::new();
    click.connect_released({
        let picker = picker.clone();
        let adj = adj.clone();
        let choose = choose.clone();
        move |_, _, x, y| {
            let i = picker.borrow().at(x, y + adj.value());
            if let Some(i) = i {
                picker.borrow_mut().selected = i;
                choose();
            }
        }
    });
    area.add_controller(click);
    let set_raw: Rc<dyn Fn(String)> = Rc::new({
        let picker = picker.clone();
        let adj = adj.clone();
        let refresh = refresh.clone();
        move |raw| {
            let mut k = picker.borrow_mut();
            k.raw = raw;
            k.all_selected = false;
            k.selected = 0;
            k.layout();
            drop(k);
            adj.set_value(0.0);
            refresh();
        }
    });
    let type_text: Rc<dyn Fn(&str)> = Rc::new({
        let picker = picker.clone();
        let set_raw = set_raw.clone();
        move |text| {
            let k = picker.borrow();
            let raw = if k.all_selected {
                String::new()
            } else {
                k.raw.clone()
            };
            drop(k);
            set_raw(raw + text);
        }
    });
    let kc = gtk::EventControllerKey::new();
    kc.set_propagation_phase(gtk::PropagationPhase::Capture);
    kc.connect_key_pressed({
        let picker = picker.clone();
        let p = p.clone();
        let app = app.clone();
        let choose = choose.clone();
        let refresh = refresh.clone();
        let set_raw = set_raw.clone();
        let type_text = type_text.clone();
        let adj = adj.clone();
        let clipboard = win.clipboard();
        move |_, key, _, state| {
            use gtk::gdk::ModifierType as M;
            let kv = key.into_glib();
            let control = state.contains(M::CONTROL_MASK);
            let alt = state.contains(M::ALT_MASK);
            let super_ = state.contains(M::SUPER_MASK);
            let mods = (i32::from(control) * 0x100)
                | (i32::from(alt) * 0x200)
                | (i32::from(super_) * 0x400)
                | (i32::from(state.contains(M::SHIFT_MASK)) * 0x800);
            if let Some((key, _)) = crate::keys::engine_key(kv) {
                let hit = |hk: i32| hk != 0 && (hk & 255) == key as i32 && (hk & 0xf00) == mods;
                if hit(p.borrow().int("EmojiHotkey", 0)) {
                    app.quit();
                    return glib::Propagation::Stop;
                }
                if hit(hotkey(&p) & !0x8000) {
                    let mut k = picker.borrow_mut();
                    k.vietnamese = !k.vietnamese;
                    let _ = p.borrow().send_command(if k.vietnamese {
                        "lang vi user"
                    } else {
                        "lang en user"
                    });
                    k.layout();
                    drop(k);
                    refresh();
                    return glib::Propagation::Stop;
                }
            }
            match kv {
                0xff1b => {
                    if picker.borrow().raw.is_empty() {
                        app.quit()
                    } else {
                        set_raw(String::new())
                    }
                }
                0xff0d | 0xff8d => choose(),
                0xff51..=0xff54 => {
                    let mut k = picker.borrow_mut();
                    k.all_selected = false;
                    match kv {
                        0xff51 => k.selected = k.selected.saturating_sub(1),
                        0xff53 => {
                            k.selected = (k.selected + 1).min(k.visible.len().saturating_sub(1))
                        }
                        0xff52 => k.move_row(-1),
                        _ => k.move_row(1),
                    }
                    if let Some(row) = k.selected_row() {
                        let y = row.y;
                        if y < adj.value() {
                            adj.set_value(if row.start == 0 { 0.0 } else { y })
                        } else if y + CELL > adj.value() + adj.page_size() {
                            adj.set_value(y + CELL - adj.page_size())
                        }
                    }
                    drop(k);
                    refresh();
                }
                0xff08 => {
                    let k = picker.borrow();
                    let mut raw = if k.all_selected {
                        String::new()
                    } else {
                        k.raw.clone()
                    };
                    if control || alt {
                        raw = raw.trim_end().to_owned();
                        while raw.ends_with(|c: char| !c.is_whitespace()) {
                            raw.pop();
                        }
                    } else {
                        raw.pop();
                    }
                    drop(k);
                    set_raw(raw);
                }
                _ => {
                    if control {
                        match key.to_lower().to_unicode() {
                            Some('v') => {
                                let type_text = type_text.clone();
                                clipboard.read_text_async(gio::Cancellable::NONE, move |result| {
                                    if let Ok(Some(text)) = result {
                                        type_text(&text.replace(['\n', '\t'], " "));
                                    }
                                });
                            }
                            Some('a') => {
                                picker.borrow_mut().all_selected = true;
                                refresh();
                            }
                            Some('c' | 'x') => {
                                let k = picker.borrow();
                                if k.all_selected && !k.raw.is_empty() {
                                    clipboard.set_text(&if k.vietnamese {
                                        convert_telex(&k.raw)
                                    } else {
                                        k.raw.clone()
                                    });
                                }
                                let cut =
                                    key.to_lower().to_unicode() == Some('x') && k.all_selected;
                                drop(k);
                                if cut {
                                    set_raw(String::new())
                                }
                            }
                            Some('w') => app.quit(),
                            _ => (),
                        }
                    } else if !alt
                        && !super_
                        && let Some(c) = key.to_unicode().filter(|c| *c >= ' ' && *c != '\u{7f}')
                    {
                        type_text(&c.to_string())
                    }
                }
            }
            glib::Propagation::Stop
        }
    });
    win.add_controller(kc);
    win.connect_is_active_notify({
        let was_active = Cell::new(false);
        let tone_pop = tone_pop.clone();
        let app = app.clone();
        move |w| {
            if w.is_active() {
                was_active.set(true)
            } else if was_active.get() && !tone_pop.is_visible() {
                app.quit();
            }
        }
    });
    let weak = win.downgrade();
    glib::timeout_add_local(Duration::from_millis(530), {
        let picker = picker.clone();
        let on = Cell::new(true);
        move || {
            if weak.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }
            on.set(!on.get());
            caret.set_opacity(if on.get() && !picker.borrow().all_selected {
                1.0
            } else {
                0.0
            });
            glib::ControlFlow::Continue
        }
    });
    win.connect_map(|w| {
        let w = w.clone();
        glib::timeout_add_local_once(Duration::from_millis(30), move || {
            crate::x11::place_picker(&w)
        });
    });
    refresh();
    win.present();
}

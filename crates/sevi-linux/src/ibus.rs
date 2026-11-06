//! Native IBus adapter. The stable engine vfunc prefix is accessed after the
//! parent service class size reported by GObject; no private IBus struct is copied.
use crate::{keys, surrounding, x11};
use gio::prelude::*;
use glib::{ControlFlow, gobject_ffi as object};
use sevi_engine::Language;
use sevi_runtime::{DesktopSession, Effect, EventResult, Platform, prefs::JsonPreferences};
use std::{
    cell::RefCell,
    ffi::{CStr, CString, c_char, c_int, c_void},
    fs,
    io::{self, Write},
    path::PathBuf,
    ptr,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};
type Engine = c_void;
type Text = c_void;
type Property = c_void;
type PropList = c_void;
type Bus = c_void;
#[link(name = "ibus-1.0")]
unsafe extern "C" {
    fn g_unix_signal_add(
        signal: c_int,
        callback: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
        data: *mut c_void,
    ) -> u32;
    fn ibus_init();
    fn ibus_main();
    fn ibus_quit();
    fn ibus_engine_get_type() -> usize;
    fn ibus_service_get_type() -> usize;
    fn ibus_text_new_from_string(text: *const c_char) -> *mut Text;
    fn ibus_text_append_attribute(text: *mut Text, type_: u32, value: u32, start: u32, end: u32);
    fn ibus_engine_commit_text(e: *mut Engine, t: *mut Text);
    fn ibus_engine_delete_surrounding_text(e: *mut Engine, offset: i32, count: u32);
    fn ibus_engine_update_preedit_text_with_mode(
        e: *mut Engine,
        t: *mut Text,
        cursor: u32,
        visible: i32,
        mode: i32,
    );
    fn ibus_engine_forward_key_event(e: *mut Engine, keyval: u32, keycode: u32, state: u32);
    fn ibus_engine_get_surrounding_text(
        e: *mut Engine,
        text: *mut *mut Text,
        cursor: *mut u32,
        anchor: *mut u32,
    );
    fn ibus_engine_get_content_type(e: *mut Engine, purpose: *mut u32, hints: *mut u32);
    fn ibus_engine_register_properties(e: *mut Engine, p: *mut PropList);
    fn ibus_engine_update_property(e: *mut Engine, p: *mut Property);
    fn ibus_property_new(
        name: *const c_char,
        type_: i32,
        label: *mut Text,
        icon: *const c_char,
        tooltip: *mut Text,
        sensitive: i32,
        visible: i32,
        state: i32,
        children: *mut PropList,
    ) -> *mut Property;
    fn ibus_property_set_label(p: *mut Property, t: *mut Text);
    fn ibus_property_set_symbol(p: *mut Property, t: *mut Text);
    fn ibus_property_set_state(p: *mut Property, s: i32);
    fn ibus_prop_list_new() -> *mut PropList;
    fn ibus_prop_list_append(list: *mut PropList, p: *mut Property);
    fn ibus_bus_new() -> *mut Bus;
    fn ibus_bus_is_connected(b: *mut Bus) -> i32;
    fn ibus_bus_get_connection(b: *mut Bus) -> *mut gio::ffi::GDBusConnection;
    fn ibus_bus_request_name(b: *mut Bus, name: *const c_char, flags: u32) -> u32;
    fn ibus_factory_new(connection: *mut gio::ffi::GDBusConnection) -> *mut c_void;
    fn ibus_factory_add_engine(factory: *mut c_void, name: *const c_char, type_: usize);
    fn ibus_component_new(
        name: *const c_char,
        description: *const c_char,
        version: *const c_char,
        license: *const c_char,
        author: *const c_char,
        homepage: *const c_char,
        command: *const c_char,
        textdomain: *const c_char,
    ) -> *mut c_void;
    fn ibus_engine_desc_new(
        name: *const c_char,
        longname: *const c_char,
        description: *const c_char,
        language: *const c_char,
        license: *const c_char,
        author: *const c_char,
        icon: *const c_char,
        layout: *const c_char,
    ) -> *mut c_void;
    fn ibus_component_add_engine(component: *mut c_void, description: *mut c_void);
    fn ibus_bus_register_component(bus: *mut Bus, component: *mut c_void) -> i32;
    fn ibus_keyval_to_unicode(value: u32) -> u32;
}
#[link(name = "gstreamer-1.0")]
unsafe extern "C" {
    fn gst_init(argc: *mut c_int, argv: *mut *mut *mut c_char);
    fn gst_element_factory_make(factory: *const c_char, name: *const c_char) -> *mut c_void;
    fn gst_element_set_state(element: *mut c_void, state: i32) -> i32;
}
const PREEDIT: u32 = 1;
const SURROUNDING: u32 = 1 << 5;
const RELEASE: u32 = 1 << 30;
fn c(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap()
}
fn text(s: &str) -> *mut Text {
    unsafe { ibus_text_new_from_string(c(s).as_ptr()) }
}
fn now() -> f64 {
    static CLOCK: OnceLock<Instant> = OnceLock::new();
    CLOCK.get_or_init(Instant::now).elapsed().as_secs_f64()
}
struct EngineState {
    engine: *mut Engine,
    caps: u32,
    props: *mut PropList,
    properties: [*mut Property; 6],
    shown: Option<(String, bool, bool)>,
}
struct App {
    session: DesktopSession,
    active: *mut Engine,
    surrounding: bool,
    reported_cursor: Option<u32>,
    clamp_deletes: bool,
    pre: Vec<char>,
    last_output: f64,
    app_pending: bool,
    pending_emoji: String,
    display: x11::Display,
    config: PathBuf,
    data: PathBuf,
    last_settings: Option<SystemTime>,
    player: *mut c_void,
}
thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};}
fn with_app<T: Default>(f: impl FnOnce(&mut App) -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        APP.with(|a| {
            a.try_borrow_mut()
                .ok()
                .and_then(|mut a| a.as_mut().map(f))
                .unwrap_or_default()
        })
    }))
    .unwrap_or_default()
}
unsafe fn state<'a>(engine: *mut Engine) -> &'a mut EngineState {
    unsafe {
        &mut *(object::g_object_get_data(engine.cast(), c"sevi-state".as_ptr()) as *mut EngineState)
    }
}
impl App {
    fn preedit(&self) -> bool {
        !self.active.is_null()
            && unsafe { state(self.active).caps } & PREEDIT != 0
            && (unsafe { state(self.active).caps } & SURROUNDING == 0 || !self.surrounding)
    }
    fn show_pre(&self) {
        if self.active.is_null() {
            return;
        }
        let s: String = self.pre.iter().collect();
        unsafe {
            let t = text(&s);
            if !s.is_empty() {
                ibus_text_append_attribute(t, 1, 1, 0, self.pre.len() as u32)
            }
            ibus_engine_update_preedit_text_with_mode(
                self.active,
                t,
                self.pre.len() as u32,
                i32::from(!s.is_empty()),
                1,
            );
        }
    }
    fn commit_raw(&self, s: &str) {
        if !self.active.is_null() && !s.is_empty() {
            unsafe {
                ibus_engine_commit_text(self.active, text(s));
            }
        }
    }
    fn flush_pre(&mut self) {
        if self.pre.is_empty() {
            return;
        }
        let s = self.pre.drain(..).collect::<String>();
        // IBus Wayland delivers these as separate updates. Chromium keeps the
        // cleared pre-edit unless its commit arrives first (upstream #35).
        self.commit_raw(&s);
        self.show_pre();
    }
    fn trim_pre(&mut self) {
        let mut separators = 0;
        let mut cut = None;
        for i in (0..self.pre.len()).rev() {
            if [' ', '\n', '\t'].contains(&self.pre[i]) {
                separators += 1;
                if separators == 3 {
                    cut = Some(i + 1);
                    break;
                }
            }
        }
        let cut = match cut {
            Some(c) => c,
            None if self.pre.len() > 160 => self.pre.len() - 80,
            _ => return,
        };
        let head = self.pre.drain(..cut).collect::<String>();
        self.commit_raw(&head);
    }
    fn effects(&mut self, result: EventResult) {
        for effect in result.effects {
            match effect {
                Effect::MacRewrite { .. } => (),
                Effect::Backspaces(mut n) => {
                    if self.active.is_null() || n == 0 {
                        continue;
                    }
                    if self.preedit() {
                        let count = n.min(self.pre.len());
                        self.pre.truncate(self.pre.len() - count);
                        n -= count;
                        self.show_pre();
                        if n == 0 {
                            continue;
                        }
                    }
                    unsafe {
                        if state(self.active).caps & SURROUNDING != 0 {
                            if self.clamp_deletes {
                                n = surrounding::delete_count(n, self.reported_cursor);
                            }
                            if n > 0 {
                                ibus_engine_delete_surrounding_text(
                                    self.active,
                                    -(n as i32),
                                    n as u32,
                                )
                            }
                        } else {
                            for _ in 0..n {
                                ibus_engine_forward_key_event(self.active, 0xff08, 14, 0);
                                ibus_engine_forward_key_event(self.active, 0xff08, 14, RELEASE);
                            }
                        }
                    }
                }
                Effect::Text(s) => {
                    if self.active.is_null() {
                        continue;
                    }
                    if self.preedit() {
                        self.pre.extend(s.chars());
                        self.trim_pre();
                        self.show_pre()
                    } else {
                        self.flush_pre();
                        self.commit_raw(&s)
                    }
                }
                Effect::ControlKey { key, shift } => {
                    if self.active.is_null() {
                        continue;
                    }
                    self.flush_pre();
                    let value = keys::keysym(key);
                    if value != 0 {
                        unsafe {
                            ibus_engine_forward_key_event(self.active, value, 0, u32::from(shift));
                            ibus_engine_forward_key_event(
                                self.active,
                                value,
                                0,
                                u32::from(shift) | RELEASE,
                            )
                        }
                    }
                }
                Effect::SwitchSound(language) => self.sound(language),
                Effect::OpenEmoji => launch("--emoji", None),
                Effect::LanguageChanged(_) => (),
            }
        }
    }
    fn sound(&mut self, language: Language) {
        let file = PathBuf::from(
            std::env::var("SEVI_DATADIR").unwrap_or_else(|_| "/usr/share/sevi".into()),
        )
        .join("sounds")
        .join(if language == Language::Vietnamese {
            "SwitchToVietnamese.wav"
        } else {
            "SwitchToEnglish.wav"
        });
        unsafe {
            if self.player.is_null() {
                self.player = gst_element_factory_make(c"playbin".as_ptr(), c"sevi-sound".as_ptr())
            }
            if !self.player.is_null() && file.is_file() {
                gst_element_set_state(self.player, 1);
                let uri = gio::File::for_path(file).uri();
                object::g_object_set(
                    self.player.cast(),
                    c"uri".as_ptr(),
                    c(uri.as_str()).as_ptr(),
                    ptr::null::<c_char>(),
                );
                if gst_element_set_state(self.player, 4) != 0 {
                    return;
                }
            }
        }
        let _ = std::process::Command::new("canberra-gtk-play")
            .args(["-i", "bell"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    fn update_properties(&mut self) {
        if self.active.is_null() {
            return;
        }
        unsafe {
            let props = state(self.active).properties;
            let vi = self.session.language() == Language::Vietnamese;
            let label = format!("Vietnamese   {}", keys::hotkey_label(self.session.hotkey));
            let auto = self.session.preferences.int("AutoSwitchByTyping", 1) != 0;
            let next = (label.clone(), vi, auto);
            if state(self.active).shown.as_ref() == Some(&next) {
                return;
            }
            let previous = state(self.active).shown.replace(next);
            if previous.as_ref().is_none_or(|p| p.0 != label || p.1 != vi) {
                ibus_property_set_label(props[0], text(&label));
                ibus_property_set_symbol(props[0], text(if vi { "V" } else { "E" }));
                ibus_property_set_state(props[0], i32::from(vi));
                ibus_engine_update_property(self.active, props[0]);
            }
            if previous.as_ref().is_none_or(|p| p.2 != auto) {
                ibus_property_set_state(props[1], i32::from(auto));
                ibus_engine_update_property(self.active, props[1]);
            }
        }
    }
    fn commit_emoji(&mut self) {
        if self.pending_emoji.is_empty() || self.active.is_null() {
            return;
        }
        self.session.reset_typing();
        self.commit_raw(&self.pending_emoji);
        self.pending_emoji.clear();
        self.last_output = now();
    }
    fn read_command(&mut self) {
        let path = self.config.join("command");
        let Ok(source) = fs::read_to_string(&path) else {
            return;
        };
        let line = source.lines().next().unwrap_or("");
        if let Some(emoji) = line.strip_prefix("emoji ") {
            self.pending_emoji = emoji.into();
            self.commit_emoji()
        } else if line.starts_with("lang ") {
            let lang = if line.contains(" vi") {
                Language::Vietnamese
            } else {
                Language::English
            };
            let result = if line.contains("user") {
                self.session.set_language_user(lang, now(), "menu", false)
            } else {
                self.session.set_language_quiet(lang)
            };
            self.effects(result);
        }
        let _ = fs::remove_file(path);
        self.update_properties();
    }
    fn poll(&mut self) {
        let modified = fs::metadata(self.config.join("settings.json"))
            .and_then(|m| m.modified())
            .ok();
        if modified != self.last_settings {
            self.last_settings = modified;
            if self.session.preferences.changed_externally() {
                let result = self.session.reload_settings(now());
                self.effects(result);
                self.update_properties();
            }
        }
        self.session.tick(now());
        self.read_command();
    }
    fn flush_log(&mut self) {
        let _ = self.session.preferences.flush();
        let data = self.session.take_log();
        if data.is_empty() {
            return;
        }
        let _ = sevi_runtime::prefs::private_directory(&self.data);
        let path = self.data.join("typing-log.jsonl");
        if fs::metadata(&path).is_ok_and(|m| m.len() > 20 * 1024 * 1024) {
            let _ = fs::rename(&path, self.data.join("typing-log.1.jsonl"));
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(data.as_bytes());
        }
    }
}
fn launch(arg: &str, second: Option<&str>) {
    let mut command = std::process::Command::new("sevi-settings");
    if !arg.is_empty() {
        command.arg(arg);
    }
    if let Some(second) = second {
        command.arg(second);
    }
    if arg == "--emoji" {
        command
            .env("GTK_IM_MODULE", "gtk-im-context-simple")
            .env("GDK_BACKEND", "x11");
    }
    let _ = command.spawn();
}
unsafe extern "C" fn process(engine: *mut Engine, keyval: u32, _keycode: u32, flags: u32) -> i32 {
    with_app(|a| {
        a.active = engine;
        let release = flags & RELEASE != 0;
        if !a.surrounding && !release {
            unsafe {
                ibus_engine_get_surrounding_text(
                    engine,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            }
        }
        if keys::is_modifier(keyval) {
            if let Some(bit) = keys::modifier(keyval) {
                let result = a.session.modifier(bit, !release, now());
                let consumed = result.consumed;
                a.effects(result);
                if consumed {
                    a.update_properties();
                    return 1;
                }
            }
            return 0;
        }
        if release {
            return 0;
        }
        let (mut purpose, mut hints) = (0, 0);
        unsafe { ibus_engine_get_content_type(engine, &mut purpose, &mut hints) }
        if purpose == 8 || purpose == 9 {
            a.flush_pre();
            return 0;
        }
        if a.app_pending {
            a.app_pending = false;
            let app = a.display.front_app();
            if !app.is_empty() && app != a.session.front_app {
                let result = a.session.focus_changed(&app, now());
                a.effects(result)
            }
        }
        let Some((key, shift)) = keys::engine_key(keyval) else {
            a.flush_pre();
            let printable =
                unsafe { glib::ffi::g_unichar_isprint(ibus_keyval_to_unicode(keyval)) } != 0;
            a.session
                .unmodeled_key(printable || matches!(keyval, 0xff0b | 0xff9d));
            return 0;
        };
        let modifiers = keys::flags(flags);
        let mut result = a
            .session
            .key_down(key, shift || modifiers.shift(), modifiers, now());
        // Let the original physical event reach the widget after committing the
        // restoration. IBus's GTK4 forward-key callback filters synthetic events
        // through the IM context, where Home/End widget bindings do not run.
        let pass_original = result
            .effects
            .iter()
            .any(|e| matches!(e, Effect::ControlKey { .. }));
        if pass_original {
            result
                .effects
                .retain(|e| !matches!(e, Effect::ControlKey { .. }));
        }
        let consumed = result.consumed && !pass_original;
        a.effects(result);
        a.update_properties();
        a.last_output = now();
        if pass_original {
            a.flush_pre();
            return 0;
        }
        if !consumed && a.preedit() {
            if !modifiers.control() {
                if keyval == 0xff08 && !a.pre.is_empty() {
                    a.pre.pop();
                    a.show_pre();
                    return 1;
                }
                let ch = unsafe { ibus_keyval_to_unicode(keyval) };
                if ch >= 32
                    && ch != 127
                    && ![0xff0d, 0xff8d, 0xff09].contains(&keyval)
                    && let Some(ch) = char::from_u32(ch)
                {
                    a.pre.push(ch);
                    a.trim_pre();
                    a.show_pre();
                    return 1;
                }
            }
            a.flush_pre()
        }
        i32::from(consumed)
    })
}
unsafe extern "C" fn focus_in(engine: *mut Engine) {
    with_app(|a| {
        a.active = engine;
        a.surrounding = false;
        a.reported_cursor = None;
        a.pre.clear();
        unsafe {
            ibus_engine_get_surrounding_text(
                engine,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            ibus_engine_register_properties(engine, state(engine).props);
            state(engine).shown = None;
        }
        let result = a.session.focus_changed(&a.display.front_app(), now());
        a.effects(result);
        a.app_pending = true;
        a.update_properties();
        a.commit_emoji();
    });
}
unsafe extern "C" fn focus_out(engine: *mut Engine) {
    with_app(|a| {
        a.pre.clear();
        a.session.click();
        if a.active == engine {
            a.active = ptr::null_mut()
        }
    });
}
unsafe extern "C" fn reset(_: *mut Engine) {
    with_app(|a| {
        if !a.pre.is_empty() {
            a.pre.clear();
            a.session.click();
            return;
        }
        if now() - a.last_output < 0.3 {
            return;
        }
        a.session.click();
    });
}
unsafe extern "C" fn enable(engine: *mut Engine) {
    unsafe {
        ibus_engine_get_surrounding_text(engine, ptr::null_mut(), ptr::null_mut(), ptr::null_mut())
    }
}
unsafe extern "C" fn disable(_: *mut Engine) {
    with_app(App::flush_log);
}
unsafe extern "C" fn capabilities(engine: *mut Engine, caps: u32) {
    unsafe { state(engine).caps = caps }
}
static PARENT_SURROUNDING: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" fn surrounding(engine: *mut Engine, t: *mut Text, cursor: u32, anchor: u32) {
    with_app(|a| {
        if a.active == engine && !a.surrounding {
            a.flush_pre();
            a.surrounding = true
        }
        if a.active == engine {
            a.reported_cursor = Some(cursor);
        }
    });
    let parent = PARENT_SURROUNDING.load(Ordering::Relaxed);
    if parent != 0 {
        let callback: unsafe extern "C" fn(*mut Engine, *mut Text, u32, u32) =
            unsafe { std::mem::transmute(parent) };
        unsafe { callback(engine, t, cursor, anchor) }
    }
}
unsafe extern "C" fn activate(engine: *mut Engine, name: *const c_char, state: u32) {
    if name.is_null() {
        return;
    }
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    with_app(|a| {
        a.active = engine;
        match name.as_ref() {
            "lang" => {
                let r = a.session.set_language_user(
                    if state == 1 {
                        Language::Vietnamese
                    } else {
                        Language::English
                    },
                    now(),
                    "menu",
                    false,
                );
                a.effects(r)
            }
            "auto" => {
                let _ = a
                    .session
                    .preferences
                    .set_int("AutoSwitchByTyping", i32::from(state == 1));
                let r = a.session.reload_settings(now());
                a.effects(r)
            }
            "open" => launch("", None),
            "guide" => launch("--guide", None),
            "update" => launch("--check-update", None),
            "about" => launch("--about", None),
            _ => (),
        }
        a.update_properties();
    });
}
unsafe extern "C" fn destroy_state(data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let s = unsafe { Box::from_raw(data as *mut EngineState) };
    with_app(|a| {
        if a.active == s.engine {
            a.active = ptr::null_mut();
            a.pre.clear()
        }
    });
    unsafe {
        for p in s.properties {
            object::g_object_unref(p.cast())
        }
        object::g_object_unref(s.props.cast())
    }
}
unsafe extern "C" fn instance_init(instance: *mut object::GTypeInstance, _: *mut c_void) {
    let engine = instance.cast::<Engine>();
    let props = unsafe { ibus_prop_list_new() };
    unsafe { object::g_object_ref_sink(props.cast()) };
    let mut properties = [ptr::null_mut(); 6];
    for (i, (name, label)) in [
        ("lang", "Vietnamese"),
        ("auto", "Detect English / Vietnamese"),
        ("open", "Open Sevi…"),
        ("guide", "Help…"),
        ("update", "Check for updates…"),
        ("about", "About Sevi"),
    ]
    .iter()
    .enumerate()
    {
        unsafe {
            let p = ibus_property_new(
                c(name).as_ptr(),
                if i < 2 { 1 } else { 0 },
                text(label),
                ptr::null(),
                ptr::null_mut(),
                1,
                1,
                if i < 2 { 1 } else { 0 },
                ptr::null_mut(),
            );
            object::g_object_ref_sink(p.cast());
            ibus_prop_list_append(props, p);
            properties[i] = p;
        }
    }
    unsafe {
        ibus_property_set_symbol(properties[0], text("V"));
        let s = Box::new(EngineState {
            engine,
            caps: 0,
            props,
            properties,
            shown: None,
        });
        object::g_object_set_data_full(
            engine.cast(),
            c"sevi-state".as_ptr(),
            Box::into_raw(s).cast(),
            Some(destroy_state),
        );
    }
}
unsafe extern "C" fn class_init(class: *mut c_void, _: *mut c_void) {
    unsafe {
        let mut query = std::mem::zeroed::<object::GTypeQuery>();
        object::g_type_query(ibus_service_get_type(), &mut query);
        let base = query.class_size as usize;
        let parent = object::g_type_class_peek_parent(class);
        let slot = |p: *mut c_void, i: usize| {
            (p as *mut u8).add(base + i * size_of::<usize>()) as *mut usize
        };
        PARENT_SURROUNDING.store(*slot(parent, 16), Ordering::Relaxed);
        for (i, callback) in [
            (0, process as *const ()),
            (1, focus_in as *const ()),
            (2, focus_out as *const ()),
            (3, reset as *const ()),
            (4, enable as *const ()),
            (5, disable as *const ()),
            (7, capabilities as *const ()),
            (12, activate as *const ()),
            (16, surrounding as *const ()),
        ] {
            *slot(class, i) = callback as usize;
        }
    }
}
fn register_type() -> usize {
    unsafe {
        let parent = ibus_engine_get_type();
        let mut query = std::mem::zeroed::<object::GTypeQuery>();
        object::g_type_query(parent, &mut query);
        object::g_type_register_static_simple(
            parent,
            c"SeviEngine".as_ptr(),
            query.class_size,
            Some(class_init),
            query.instance_size,
            Some(instance_init),
            0,
        )
    }
}
unsafe extern "C" fn disconnected(_: *mut Bus, _: *mut c_void) {
    with_app(App::flush_log);
    unsafe { ibus_quit() }
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    use gio::prelude::*;
    x11::configure_environment();
    unsafe {
        ibus_init();
        gst_init(ptr::null_mut(), ptr::null_mut())
    }
    let prefs = JsonPreferences::linux()?;
    let (config, data) = (prefs.config_dir.clone(), prefs.data_dir.clone());
    sevi_runtime::prefs::private_directory(&config)?;
    let last_settings = fs::metadata(prefs.settings_path())
        .and_then(|m| m.modified())
        .ok();
    let session = DesktopSession::new(Platform::Linux, Box::new(prefs));
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            session,
            active: ptr::null_mut(),
            surrounding: false,
            reported_cursor: None,
            clamp_deletes: surrounding::clamp_needed(
                std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
                std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
            ),
            pre: vec![],
            last_output: -1.0,
            app_pending: false,
            pending_emoji: String::new(),
            display: x11::Display::default(),
            config: config.clone(),
            data,
            last_settings,
            player: ptr::null_mut(),
        })
    });
    unsafe {
        let bus = ibus_bus_new();
        if ibus_bus_is_connected(bus) == 0 {
            return Err(io::Error::other("cannot connect to the IBus daemon").into());
        }
        let callback: unsafe extern "C" fn() = std::mem::transmute(disconnected as *const ());
        object::g_signal_connect_data(
            bus.cast(),
            c"disconnected".as_ptr(),
            Some(callback),
            ptr::null_mut(),
            None,
            0,
        );
        let factory = ibus_factory_new(ibus_bus_get_connection(bus));
        ibus_factory_add_engine(factory, c"sevi".as_ptr(), register_type());
        if std::env::args().any(|a| a == "--ibus") {
            ibus_bus_request_name(bus, c"org.freedesktop.IBus.Sevi".as_ptr(), 0);
        } else {
            let component = ibus_component_new(
                c"org.freedesktop.IBus.Sevi".as_ptr(),
                c"Sevi".as_ptr(),
                c(env!("CARGO_PKG_VERSION")).as_ptr(),
                c"GPL-3.0".as_ptr(),
                c"Tony Dinh".as_ptr(),
                c"".as_ptr(),
                c"".as_ptr(),
                c"ibus-sevi".as_ptr(),
            );
            let desc = ibus_engine_desc_new(
                c"sevi".as_ptr(),
                c"Sevi".as_ptr(),
                c("Sevi - Smart English/Vietnamese input").as_ptr(),
                c"vi".as_ptr(),
                c"GPL-3.0".as_ptr(),
                c"Tony Dinh".as_ptr(),
                c"sevi".as_ptr(),
                c"us".as_ptr(),
            );
            ibus_component_add_engine(component, desc);
            ibus_bus_register_component(bus, component);
        }
    }
    let monitor = gio::File::for_path(&config)
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)?;
    monitor.connect_changed(|_, _, _, _| with_app(App::poll));
    glib::timeout_add_local(Duration::from_secs(1), || {
        with_app(App::poll);
        ControlFlow::Continue
    });
    glib::timeout_add_local(Duration::from_secs(10), || {
        with_app(App::flush_log);
        ControlFlow::Continue
    });
    let update = || {
        with_app(|a| {
            if a.session.preferences.int("AutoUpdate", 1) == 0 {
                return;
            }
            let time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i32;
            let last = a.session.preferences.int("LastUpdateCheck", 0);
            if time.saturating_sub(last) < 86400 {
                return;
            }
            let _ = a.session.preferences.set_int("LastUpdateCheck", time);
            launch("--check-update", Some("--quiet"));
        })
    };
    glib::timeout_add_local_once(Duration::from_secs(90), move || {
        update();
        glib::timeout_add_local(Duration::from_secs(3600), move || {
            update();
            ControlFlow::Continue
        });
    });
    unsafe {
        g_unix_signal_add(libc::SIGTERM, Some(shutdown), ptr::null_mut());
        g_unix_signal_add(libc::SIGINT, Some(shutdown), ptr::null_mut());
    }
    unsafe { ibus_main() };
    with_app(App::flush_log);
    drop(monitor);
    Ok(())
}
unsafe extern "C" fn shutdown(_: *mut c_void) -> c_int {
    with_app(App::flush_log);
    unsafe { ibus_quit() };
    0
}

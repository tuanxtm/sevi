//! Main-run-loop event tap and asynchronous browser lookup. No browser IPC in the tap.
use super::{
    ffi::*,
    prefs::{self, Defaults},
};
use crate::policy::{self, Rewrite};
use objc2::{
    class, msg_send,
    rc::{Allocated, Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_foundation::NSString;
use sevi_engine::{Language, keys::*};
use sevi_runtime::{DesktopSession, Effect, EventResult, Modifiers, Platform};
use std::{cell::RefCell, ptr, sync::mpsc, time::Instant};
thread_local! { static HOST: RefCell<Option<Host>> = const { RefCell::new(None) }; }
thread_local! { static SOUNDS:RefCell<[Option<Retained<AnyObject>>;2]>=const{RefCell::new([None,None])}; }
pub fn with<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    HOST.with(|h| h.try_borrow_mut().ok().and_then(|mut h| h.as_mut().map(f)))
}
pub struct Host {
    pub session: DesktopSession,
    tap: CF,
    run_source: CF,
    source: CF,
    proxy: CF,
    clock: Instant,
    last_flags: Modifiers,
    event_flags: u64,
    layout_compat: bool,
    last_layout: Instant,
    last_flush: Instant,
    last_site: Instant,
    site_result: Option<mpsc::Receiver<(String, String)>>,
    rewrite_policy: Option<Rewrite>,
    smart_save_scheduled: bool,
    pub language_changed: bool,
}
pub fn initialize() {
    let clock = Instant::now();
    let host = Host {
        session: DesktopSession::new(Platform::Mac, Box::new(Defaults)),
        tap: ptr::null_mut(),
        run_source: ptr::null_mut(),
        source: unsafe { CGEventSourceCreate(-1) },
        proxy: ptr::null_mut(),
        clock,
        last_flags: Modifiers(0),
        event_flags: 0,
        layout_compat: false,
        last_layout: clock,
        last_flush: clock,
        last_site: clock,
        site_result: None,
        rewrite_policy: None,
        smart_save_scheduled: false,
        language_changed: true,
    };
    HOST.with(|h| *h.borrow_mut() = Some(host));
    with(|h| {
        h.update_layout();
        h.focus_changed();
    });
}
pub fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}
pub fn request_accessibility() {
    unsafe {
        let key = &*(kAXTrustedCheckOptionPrompt as *const NSString);
        let number: Retained<AnyObject> = msg_send![class!(NSNumber), numberWithBool: true];
        let dict: Retained<AnyObject> =
            msg_send![class!(NSDictionary), dictionaryWithObject: &*number, forKey: key];
        AXIsProcessTrustedWithOptions(Retained::as_ptr(&dict) as CF);
    }
}
pub fn front_app() -> (String, String) {
    unsafe {
        let workspace: Retained<AnyObject> = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: Option<Retained<AnyObject>> = msg_send![&*workspace, frontmostApplication];
        let Some(app) = app else {
            return (String::new(), String::new());
        };
        let name = prefs::text(msg_send![&*app, localizedName]);
        let id = prefs::text(msg_send![&*app, bundleIdentifier]);
        (if id.is_empty() { name.clone() } else { id }, name)
    }
}
pub fn context_name() -> String {
    let bundle = with(|h| h.session.front_app.clone()).unwrap_or_default();
    unsafe {
        let apps: Retained<AnyObject> = msg_send![class!(NSRunningApplication),runningApplicationsWithBundleIdentifier:&*NSString::from_str(&bundle)];
        let app: Option<Retained<AnyObject>> = msg_send![&*apps, firstObject];
        app.map(|a| prefs::text(msg_send![&*a, localizedName]))
            .filter(|name| !name.is_empty())
            .unwrap_or(bundle)
    }
}
fn spotlight() -> bool {
    unsafe {
        let windows = CGWindowListCopyWindowInfo(1 | 16, 0);
        if windows.is_null() {
            return false;
        }
        let mut found = false;
        let status_level = CGWindowLevelForKey(9);
        for i in 0..CFArrayGetCount(windows) {
            let window = &*(CFArrayGetValueAtIndex(windows, i) as *const AnyObject);
            let name: Option<Retained<NSString>> =
                msg_send![window, objectForKey: &*NSString::from_str("kCGWindowOwnerName")];
            let layer: Option<Retained<AnyObject>> =
                msg_send![window, objectForKey: &*NSString::from_str("kCGWindowLayer")];
            let layer = layer.map(|n| msg_send![&*n, intValue]).unwrap_or(0);
            if policy::spotlight_window(&prefs::text(name), layer, status_level) {
                found = true;
                break;
            }
        }
        CFRelease(windows);
        found
    }
}
pub fn modifiers(flags: u64) -> Modifiers {
    let mut bits = 0;
    for (native, common) in [(17, 1), (18, 2), (19, 4), (20, 8), (16, 16)] {
        if flags & (1 << native) != 0 {
            bits |= common
        }
    }
    Modifiers(bits)
}
impl Host {
    pub fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64()
    }
    pub fn running(&self) -> bool {
        !self.tap.is_null()
    }
    pub fn start(&mut self) -> bool {
        if self.running() {
            return true;
        }
        unsafe {
            self.tap = CGEventTapCreate(
                1,
                0,
                0,
                (1 << 10) | (1 << 11) | (1 << 12) | (1 << 1) | (1 << 3) | (1 << 25),
                callback,
                ptr::null_mut(),
            );
            if self.tap.is_null() {
                return false;
            }
            self.run_source = CFMachPortCreateRunLoopSource(ptr::null_mut(), self.tap, 0);
            if self.run_source.is_null() {
                self.stop();
                return false;
            }
            CFRunLoopAddSource(CFRunLoopGetMain(), self.run_source, kCFRunLoopCommonModes);
            CGEventTapEnable(self.tap, true);
        }
        self.session.reset_typing();
        true
    }
    pub fn stop(&mut self) {
        unsafe {
            if !self.run_source.is_null() {
                CFRunLoopRemoveSource(CFRunLoopGetMain(), self.run_source, kCFRunLoopCommonModes);
                CFRelease(self.run_source);
                self.run_source = ptr::null_mut();
            }
            if !self.tap.is_null() {
                CGEventTapEnable(self.tap, false);
                CFMachPortInvalidate(self.tap);
                CFRelease(self.tap);
                self.tap = ptr::null_mut();
            }
        }
    }
    pub fn reenable(&mut self) {
        if self.running() {
            unsafe {
                CGEventTapEnable(self.tap, true);
            }
            self.session.reset_typing();
        }
    }
    pub fn focus_changed(&mut self) {
        let (app, _) = front_app();
        if app.is_empty() {
            return;
        }
        let context = if app == "com.devuap.sevi" {
            self.session.front_app.clone()
        } else {
            app.clone()
        };
        let r = self.session.focus_changed(&context, self.now());
        // App/site memory retains the previous typing app while our own window is
        // active; the log separately records the app actually receiving keys.
        self.session.log.app = app;
        self.effects(r);
        self.last_site = self.clock;
    }
    pub fn set_language(&mut self, language: Language) {
        let r = self
            .session
            .set_language_user(language, self.now(), "menu", false);
        self.effects(r);
    }
    pub fn reload(&mut self) {
        let r = self.session.reload_settings(self.now());
        self.effects(r);
        self.update_layout();
        self.last_site = self.clock;
    }
    pub fn effects(&mut self, result: EventResult) {
        let mut selection = false;
        for effect in result.effects {
            match effect {
                Effect::MacRewrite {
                    autocomplete,
                    selection: may_select,
                } => {
                    if !autocomplete && !may_select {
                        continue;
                    }
                    let policy = *self.rewrite_policy.get_or_insert_with(|| {
                        let app = front_app().0;
                        let visible = app == "com.apple.Spotlight" || spotlight();
                        policy::rewrite(&app, visible)
                    });
                    selection = may_select && policy == Rewrite::Selection;
                    if autocomplete && let Rewrite::Prefix(c) = policy {
                        self.post_text(&c.to_string());
                        self.post_key(KEY_DELETE, false);
                    }
                }
                Effect::Backspaces(n) => {
                    for _ in 0..n {
                        self.post_key(if selection { KEY_LEFT } else { KEY_DELETE }, selection);
                    }
                }
                Effect::Text(text) => self.post_text(&text),
                Effect::ControlKey { key, .. } => {
                    self.post_key_flags(key, self.event_flags | (1 << 8))
                }
                Effect::LanguageChanged(_) => self.language_changed = true,
                Effect::SwitchSound(language) => play_sound(language),
                Effect::OpenEmoji => {
                    dispatch2::DispatchQueue::main().exec_async(super::app::open_picker)
                }
            }
        }
        if self.session.smart_preferences_dirty() && !self.smart_save_scheduled {
            self.smart_save_scheduled = true;
            dispatch2::DispatchQueue::main().exec_async(|| {
                with(|host| {
                    host.smart_save_scheduled = false;
                    let _ = host.session.flush_smart_preferences();
                });
            });
        }
    }
    fn post(&self, event: CF) {
        if self.proxy.is_null() {
            unsafe {
                CGEventPost(1, event);
            }
        } else {
            unsafe {
                CGEventTapPostEvent(self.proxy, event);
            }
        }
    }
    pub fn post_key(&self, key: u32, shift: bool) {
        self.post_key_flags(key, if shift { 1 << 17 } else { 0 });
    }
    fn post_key_flags(&self, key: u32, flags: u64) {
        unsafe {
            for down in [true, false] {
                let event = CGEventCreateKeyboardEvent(self.source, key as u16, down);
                if event.is_null() {
                    continue;
                }
                CGEventSetIntegerValueField(event, USER_DATA, EVENT_TAG);
                CGEventSetFlags(event, flags);
                self.post(event);
                CFRelease(event);
            }
        }
    }
    pub fn post_text(&self, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        for chunk in units.chunks(16) {
            unsafe {
                for down in [true, false] {
                    let event = CGEventCreateKeyboardEvent(self.source, 0, down);
                    if event.is_null() {
                        continue;
                    }
                    CGEventSetIntegerValueField(event, USER_DATA, EVENT_TAG);
                    CGEventSetFlags(event, 0);
                    CGEventKeyboardSetUnicodeString(event, chunk.len(), chunk.as_ptr());
                    self.post(event);
                    CFRelease(event);
                }
            }
        }
    }
    pub fn insert_text(&mut self, text: &str) {
        self.session.reset_typing();
        self.post_text(text);
    }
    pub fn flush(&mut self) {
        let _ = self.session.flush_smart_preferences();
    }
    pub fn logical_key(&self, event: &AnyObject, fallback: u32) -> u32 {
        if self.layout_compat {
            let text = unsafe { prefs::text(msg_send![event, charactersIgnoringModifiers]) };
            policy::layout_key(&text, fallback)
        } else {
            fallback
        }
    }
    pub fn update_layout(&mut self) {
        unsafe {
            let layout = TISCopyCurrentKeyboardLayoutInputSource();
            self.layout_compat = false;
            if !layout.is_null() {
                let id =
                    TISGetInputSourceProperty(layout, kTISPropertyInputSourceID) as *const NSString;
                if let Some(id) = id.as_ref() {
                    self.layout_compat = !policy::QWERTY.contains(&id.to_string().as_str());
                }
                CFRelease(layout);
            }
            let source = TISCopyCurrentKeyboardInputSource();
            self.session.other_language = false;
            if !source.is_null() {
                let languages = TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages);
                if !languages.is_null() && CFArrayGetCount(languages) > 0 {
                    let language = &*(CFArrayGetValueAtIndex(languages, 0) as *const NSString);
                    self.session.other_language = language.to_string() != "en";
                }
                CFRelease(source);
            }
        }
    }
    pub fn tick(&mut self) {
        self.session.tick(self.now());
        if self.running() {
            unsafe {
                if !CFMachPortIsValid(self.tap) {
                    self.stop();
                    self.start();
                    self.session.reset_typing();
                } else if !CGEventTapIsEnabled(self.tap) {
                    self.reenable();
                }
            }
        }
        if self.last_layout.elapsed().as_secs_f64() >= 1.0 {
            self.update_layout();
            self.last_layout = Instant::now();
        }
        if self.last_flush.elapsed().as_secs() >= 10 || self.session.log.pending.len() > 8192 {
            self.flush();
            self.last_flush = Instant::now();
        }
        if let Some(receiver) = &self.site_result {
            match receiver.try_recv() {
                Ok((bundle, host)) => {
                    self.site_result = None;
                    if front_app().0 == bundle {
                        let r = self.session.site_changed(&host);
                        self.effects(r);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.site_result = None,
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        if self.session.preferences.int("SmartSwitchPerSite", 0) != 0
            && self.last_site.elapsed().as_secs_f64() >= 1.0
            && self.site_result.is_none()
        {
            self.last_site = Instant::now();
            let bundle = front_app().0;
            if let Some(script) = policy::browser_script(&bundle) {
                let (sender, receiver) = mpsc::channel();
                self.site_result = Some(receiver);
                std::thread::spawn(move || {
                    let (host, _) = read_site(&script);
                    let _ = sender.send((bundle, host));
                });
            }
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.flush();
        self.stop();
        if !self.source.is_null() {
            unsafe {
                CFRelease(self.source);
            }
        }
    }
}
fn play_sound(language: Language) {
    SOUNDS.with(|sounds| {
        let mut sounds=sounds.borrow_mut();
        let slot=&mut sounds[usize::from(language==Language::Vietnamese)];
        if let Some(sound)=slot {unsafe{let _:bool=msg_send![&**sound,stop];let _:bool=msg_send![&**sound,play];}return}
    unsafe {
        let bundle: Retained<AnyObject> = msg_send![class!(NSBundle), mainBundle];
        let name = NSString::from_str(if language == Language::Vietnamese {
            "SwitchToVietnamese"
        } else {
            "SwitchToEnglish"
        });
        let url: Option<Retained<AnyObject>> =
            msg_send![&*bundle, URLForResource: &*name, withExtension: &*NSString::from_str("caf")];
        if let Some(url) = url {
            let sound: Allocated<AnyObject> = msg_send![class!(NSSound), alloc];
            let sound: Option<Retained<AnyObject>> =
                msg_send![sound, initWithContentsOfURL: &*url, byReference: true];
            if let Some(sound) = sound {
                let _: bool = msg_send![&*sound, play];
                *slot=Some(sound);
                return;
            }
        }
        objc2_app_kit::NSBeep();
    }
    });
}
/// Returns hostname and AppleScript error. Called only on a worker thread.
pub fn read_site(script: &str) -> (String, i32) {
    autoreleasepool(|_| unsafe {
        let script_object: Allocated<AnyObject> = msg_send![class!(NSAppleScript), alloc];
        let script_object: Option<Retained<AnyObject>> =
            msg_send![script_object, initWithSource: &*NSString::from_str(script)];
        let Some(script_object) = script_object else {
            return (String::new(), 0);
        };
        let mut error: *mut AnyObject = ptr::null_mut();
        let result: Option<Retained<AnyObject>> =
            msg_send![&*script_object, executeAndReturnError: &mut error];
        if !error.is_null() {
            let code: Option<Retained<AnyObject>> =
                msg_send![error, objectForKey: &*NSString::from_str("NSAppleScriptErrorNumber")];
            return (
                String::new(),
                code.map(|n| msg_send![&*n, intValue]).unwrap_or(0),
            );
        }
        let url = result.and_then(|r| {
            let s: Option<Retained<NSString>> = msg_send![&*r, stringValue];
            s
        });
        let Some(url) = url else {
            return (String::new(), 0);
        };
        let url: Option<Retained<AnyObject>> = msg_send![class!(NSURL), URLWithString: &*url];
        let Some(url) = url else {
            return (String::new(), 0);
        };
        let scheme = prefs::text(msg_send![&*url, scheme]);
        let host = prefs::text(msg_send![&*url, host]).to_lowercase();
        if scheme.starts_with("http") {
            (host.strip_prefix("www.").unwrap_or(&host).to_owned(), 0)
        } else {
            (String::new(), 0)
        }
    })
}
unsafe extern "C" fn callback(proxy: CF, kind: u32, event: CF, _: CF) -> CF {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with(|host| unsafe {
            if kind == u32::MAX || kind == u32::MAX - 1 {
                host.reenable();
                return event;
            }
            if event.is_null()
                || CGEventGetIntegerValueField(event, USER_DATA) == EVENT_TAG
                || CGEventGetIntegerValueField(event, SOURCE_STATE)
                    == CGEventSourceGetSourceStateID(host.source) as i64
            {
                return event;
            }
            host.proxy = proxy;
            host.rewrite_policy = None;
            if super::app::own_ui() {
                host.session.unmodeled_key(false);
                host.last_flags = modifiers(CGEventGetFlags(event));
                host.session.synchronize_modifiers(host.last_flags);
                return event;
            }
            let flags = CGEventGetFlags(event);
            host.event_flags = flags;
            let mut key = CGEventGetIntegerValueField(event, KEY_CODE) as u32;
            if kind == 10 && host.layout_compat {
                let ns_event: Option<Retained<AnyObject>> =
                    msg_send![class!(NSEvent), eventWithCGEvent: event];
                if let Some(e) = ns_event {
                    key = host.logical_key(&e, key);
                }
            }
            let mut mods = modifiers(flags);
            let result = match kind {
                12 => {
                    let changed = host.last_flags.0 ^ mods.0;
                    let mut combined = EventResult::default();
                    for bit in [
                        Modifiers::SHIFT,
                        Modifiers::CONTROL,
                        Modifiers::ALT,
                        Modifiers::SUPER,
                        Modifiers::CAPS,
                    ] {
                        if changed & bit.0 != 0 {
                            let r = host.session.modifier(bit, mods.contains(bit), host.now());
                            combined.consumed |= r.consumed;
                            combined.effects.extend(r.effects);
                        }
                    }
                    host.last_flags = mods;
                    combined
                }
                10 => {
                    host.session.log.secure = IsSecureEventInputEnabled();
                    if flags & ((1 << 21) | (1 << 22) | (1 << 23)) != 0
                        && !([KEY_LEFT, KEY_RIGHT].contains(&key) && mods.0 & 15 == 0)
                    {
                        mods.0 |= Modifiers::OTHER.0;
                    }
                    let repeat = CGEventGetIntegerValueField(event, 8) != 0;
                    host.session
                        .key_down_event(key, mods.shift(), mods, repeat, host.now())
                }
                1 | 3 | 25 => {
                    host.session.synchronize_modifiers(mods);
                    host.session.click();
                    host.last_site = host.clock;
                    EventResult::default()
                }
                _ => EventResult::default(),
            };
            let consumed = result.consumed;
            host.effects(result);
            host.proxy = ptr::null_mut();
            if consumed { ptr::null_mut() } else { event }
        })
        .unwrap_or(event)
    }))
    .unwrap_or(event)
}

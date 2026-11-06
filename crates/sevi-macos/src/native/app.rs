//! Status item and platform services. All application windows use sevi-ui.
use super::{
    host,
    prefs::{self, Defaults},
    ui::{self, Obj},
    updater::{self, Updater},
};
use objc2::{
    MainThreadOnly, class, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
    sel,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSSize, NSString};
use serde_json::json;
use sevi_engine::Language;
use sevi_runtime::prefs::Preferences;
use sevi_ui_protocol::{host::UiHost, *};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    process::Command,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
};

static UI_PROCESS: AtomicU32 = AtomicU32::new(0);
pub fn own_ui() -> bool {
    let pid = UI_PROCESS.load(Ordering::Relaxed);
    pid != 0 && front_pid() == pid
}
fn front_pid() -> u32 {
    unsafe {
        let ws: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: Option<Obj> = msg_send![&*ws, frontmostApplication];
        app.map(|a| {
            let pid: i32 = msg_send![&*a, processIdentifier];
            pid as u32
        })
        .unwrap_or(0)
    }
}
pub fn application() -> Obj {
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}
pub(super) fn guarded(f: impl FnOnce()) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
}
define_class!(
    #[unsafe(super=NSObject)] #[thread_kind=MainThreadOnly] pub struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    impl Delegate {
        #[unsafe(method(action:))] fn action(&self,sender:&AnyObject){guarded(||with(|a|a.action(sender)));}
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))] fn reopen(&self,_:&AnyObject,_:bool)->bool{with(|a|a.open(View::Settings));false}
        #[unsafe(method(applicationWillTerminate:))] fn terminate(&self,_:&AnyObject){with(|a|a.client.close());host::with(|h|h.flush());}
        #[unsafe(method(activated:))] fn activated(&self,_:&AnyObject){guarded(||{host::with(|h|h.focus_changed());with(|a|a.menu());});}
        #[unsafe(method(space:))] fn space(&self,_:&AnyObject){guarded(||{host::with(|h|{h.session.engine.handle_event(sevi_engine::Event::Mouse,sevi_engine::EventState::MouseDown,0,0,false);});});}
        #[unsafe(method(sleep:))] fn sleep(&self,_:&AnyObject){guarded(||{host::with(|h|h.stop());});}
        #[unsafe(method(wake:))] fn wake(&self,_:&AnyObject){guarded(||{host::with(|h|{if host::trusted(){h.start();}});});}
        #[unsafe(method(layout:))] fn layout(&self,_:&AnyObject){guarded(||{host::with(|h|h.update_layout());});}
        #[unsafe(method(menuWillOpen:))] fn menu_open(&self,_:&AnyObject){with(|a|a.menu());}
    }
);
thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};}
pub fn with(f: impl FnOnce(&mut App)) {
    guarded(|| {
        APP.with(|s| {
            if let Ok(mut s) = s.try_borrow_mut()
                && let Some(a) = s.as_mut()
            {
                f(a);
            }
        })
    });
}
pub struct App {
    delegate: Retained<Delegate>,
    status: Obj,
    status_menu: Obj,
    updater: Updater,
    client: UiHost,
    target: u32,
    last_snapshot: String,
    permission: bool,
    browser_access: Option<mpsc::Receiver<bool>>,
    _timer: Option<Obj>,
    tick_count: u32,
}
pub fn open_picker() {
    with(|a| a.open(View::Emoji));
}
pub fn run() {
    objc2::rc::autoreleasepool(|_| unsafe {
        let app = application();
        let _: bool = msg_send![&*app,setActivationPolicy:1isize];
        prefs::migrate();
        host::initialize();
        let delegate: Retained<Delegate> = msg_send![
            Delegate::alloc(MainThreadMarker::new().expect("main thread")),
            init
        ];
        let _: () = msg_send![&*app,setDelegate:&*delegate];
        let bar: Obj = msg_send![class!(NSStatusBar), systemStatusBar];
        let status: Obj = msg_send![&*bar,statusItemWithLength:-1.0f64];
        let menu = ui::object("NSMenu");
        let _: () = msg_send![&*menu,setDelegate:&*delegate];
        let _: () = msg_send![&*status,setMenu:&*menu];
        let permission = host::trusted() && host::with(|h| h.start()).unwrap_or(false);
        let mut a = App {
            delegate,
            status,
            status_menu: menu,
            updater: Updater::new(),
            client: UiHost::default(),
            target: 0,
            last_snapshot: String::new(),
            permission,
            browser_access: None,
            _timer: None,
            tick_count: 0,
        };
        a.menu();
        a.app_menu();
        a.observe();
        if !permission || Defaults.int("showMain", 0) != 0 {
            a.open(View::Settings);
        }
        if Defaults.int("showAbout", 0) != 0 {
            a.open(View::About);
        }
        APP.with(|s| *s.borrow_mut() = Some(a));
        let block = block2::RcBlock::new(|_: *mut AnyObject| {
            guarded(|| {
                host::with(|h| h.tick());
                with(|a| a.tick());
            })
        });
        let timer: Obj = msg_send![class!(NSTimer),scheduledTimerWithTimeInterval:0.1f64,repeats:true,block:&*block];
        with(|a| a._timer = Some(timer));
        let _: () = msg_send![&*app, run];
        host::with(|h| {
            h.flush();
            h.stop();
        });
    });
}
impl App {
    fn item(&self, menu: &AnyObject, title: &str, tag: isize, checked: Option<bool>, key: &str) {
        unsafe {
            if title.is_empty() {
                let item: Obj = msg_send![class!(NSMenuItem), separatorItem];
                let _: () = msg_send![menu,addItem:&*item];
                return;
            }
            let a: Allocated<AnyObject> = msg_send![class!(NSMenuItem), alloc];
            let item: Obj = msg_send![a,initWithTitle:&*NSString::from_str(title),action:sel!(action:),keyEquivalent:&*NSString::from_str(key)];
            let _: () = msg_send![&*item,setTag:tag];
            let _: () = msg_send![&*item,setTarget:&*self.delegate];
            if let Some(on) = checked {
                let _: () = msg_send![&*item,setState:isize::from(on)];
            }
            let _: () = msg_send![&*item,setEnabled:tag>=0];
            let _: () = msg_send![menu,addItem:&*item];
        }
    }
    fn menu(&mut self) {
        unsafe {
            let _: () = msg_send![&*self.status_menu, removeAllItems];
        }
        let (language, hotkey, auto, excluded) = host::with(|h| {
            (
                h.session.language(),
                h.session.hotkey,
                h.session.auto_for_app(),
                h.session.excluded,
            )
        })
        .unwrap();
        if !self.permission {
            self.item(&self.status_menu, "Enable Accessibility…", 302, None, "");
        }
        self.item(
            &self.status_menu,
            &format!("Vietnamese   {}", crate::policy::hotkey_label(hotkey)),
            1,
            Some(language == Language::Vietnamese),
            "",
        );
        self.item(
            &self.status_menu,
            "Detect English / Vietnamese",
            2,
            Some(Defaults.int("AutoSwitchByTyping", 1) != 0),
            "",
        );
        let name = host::context_name();
        self.item(
            &self.status_menu,
            &if excluded {
                format!("{name}: excluded")
            } else {
                format!("Auto-switch in {name}")
            },
            if excluded { -1 } else { 3 },
            if excluded { None } else { Some(auto) },
            "",
        );
        for (title, tag, key) in [
            ("", 0, ""),
            ("Open Sevi…", 5, ","),
            ("Help…", 4, ""),
            ("Check for updates…", 7, ""),
            ("About Sevi", 8, ""),
            ("", 0, ""),
            ("Quit Sevi", 9, "q"),
        ] {
            self.item(&self.status_menu, title, tag, None, key);
        }
        unsafe {
            let button: Obj = msg_send![&*self.status, button];
            let image = status_icon(language == Language::Vietnamese);
            let _: () = msg_send![&*button,setImage:&*image];
        }
    }
    fn app_menu(&self) {
        unsafe {
            let main = ui::object("NSMenu");
            let root = ui::object("NSMenuItem");
            let app = ui::object("NSMenu");
            for (title, tag, key) in [
                ("About Sevi", 8, ""),
                ("Credits…", 10, ""),
                ("Check for updates…", 7, ""),
                ("", 0, ""),
                ("Open Sevi…", 5, ","),
                ("", 0, ""),
                ("Quit Sevi", 9, "q"),
            ] {
                self.item(&app, title, tag, None, key);
            }
            let _: () = msg_send![&*root,setSubmenu:&*app];
            let _: () = msg_send![&*main,addItem:&*root];
            let _: () = msg_send![&*application(),setMainMenu:&*main];
        }
    }
    fn observe(&self) {
        unsafe {
            let distributed: Obj =
                msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
            let name =
                &*(super::ffi::kTISNotifySelectedKeyboardInputSourceChanged as *const NSString);
            let _: () = msg_send![&*distributed,addObserver:&*self.delegate,selector:sel!(layout:),name:name,object:std::ptr::null::<AnyObject>()];
            let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
            let center: Obj = msg_send![&*workspace, notificationCenter];
            for (name, selector) in [
                (
                    "NSWorkspaceDidActivateApplicationNotification",
                    sel!(activated:),
                ),
                ("NSWorkspaceActiveSpaceDidChangeNotification", sel!(space:)),
                ("NSWorkspaceWillSleepNotification", sel!(sleep:)),
                ("NSWorkspaceDidWakeNotification", sel!(wake:)),
            ] {
                let _: () = msg_send![&*center,addObserver:&*self.delegate,selector:selector,name:&*NSString::from_str(name),object:std::ptr::null::<AnyObject>()];
            }
        }
    }
    fn action(&mut self, sender: &AnyObject) {
        let tag: isize = unsafe { msg_send![sender, tag] };
        match tag {
            1 => {
                host::with(|h| h.set_language(h.session.language().toggled()));
            }
            2 => {
                let _ = Defaults.set_int(
                    "AutoSwitchByTyping",
                    i32::from(Defaults.int("AutoSwitchByTyping", 1) == 0),
                );
                host::with(|h| h.reload());
            }
            3 => {
                host::with(|h| {
                    h.session
                        .set_auto_for_app(!h.session.auto_for_app(), h.now())
                });
            }
            4 => self.open(View::Help),
            5 => self.open(View::Settings),
            7 => self.updater.check(),
            8 => self.open(View::About),
            10 => self.open(View::Credits),
            302 => host::request_accessibility(),
            9 => unsafe {
                let _: () = msg_send![&*application(),terminate:std::ptr::null::<AnyObject>()];
            },
            _ => (),
        }
    }
    fn snapshot(&self) -> UiSnapshot {
        let mut s = sevi_runtime::ui::snapshot(&Defaults, Platform::MacOS);
        s.permission_granted = self.permission;
        s.login = updater::login_enabled();
        s.capabilities.updates = self.updater.available();
        s.settings
            .insert(Setting::AutoUpdate, i32::from(self.updater.automatic()));
        if let Some((vi, key, auto)) = host::with(|h| {
            (
                h.session.language() == Language::Vietnamese,
                h.session.hotkey,
                h.session.auto_for_app(),
            )
        }) {
            s.language_vi = vi;
            s.settings.insert(Setting::SwitchKeyStatus, key as i32);
            s.app_auto = auto;
        }
        s.current_app = host::context_name();
        s.apps = excluded_apps();
        s
    }
    fn open(&mut self, view: View) {
        if view == View::Emoji {
            self.target = front_pid();
            host::with(|h| h.session.emoji_open = true);
        }
        let command = match std::env::current_exe() {
            Ok(exe) => Command::new(exe),
            Err(e) => {
                eprintln!("Sevi UI: {e}");
                return;
            }
        };
        if let Err(e) = self.client.open(command, view, self.snapshot(), || {}) {
            host::with(|h| h.session.emoji_open = false);
            eprintln!("Sevi UI: {e}");
        }
        UI_PROCESS.store(self.client.pid().unwrap_or(0), Ordering::Relaxed);
    }
    fn request(&mut self, request: UiCommand) -> std::io::Result<()> {
        match request {
            UiCommand::Set { setting, value } => {
                sevi_runtime::ui::set(&mut Defaults, setting, value)?;
                if setting == Setting::AutoUpdate {
                    self.updater.set_automatic(value != 0);
                }
                if setting == Setting::SmartSwitchPerSite && value != 0 {
                    let scripts = running_browser_scripts();
                    let (tx, rx) = mpsc::channel();
                    self.browser_access = Some(rx);
                    std::thread::spawn(move || {
                        let mut denied = false;
                        for script in scripts {
                            denied |= host::read_site(&script).1 == -1743;
                        }
                        let _ = tx.send(denied);
                    });
                }
                host::with(|h| h.reload());
            }
            UiCommand::SetLanguage(vi) => {
                host::with(|h| {
                    h.set_language(if vi {
                        Language::Vietnamese
                    } else {
                        Language::English
                    })
                });
            }
            UiCommand::SetLogin(on) => updater::set_login(on),
            UiCommand::SetAppAuto(on) => {
                host::with(|h| h.session.set_auto_for_app(on, h.now()));
            }
            UiCommand::SetExcluded { id, excluded } if !id.is_empty() && id.len() < 256 => {
                let mut ids: Vec<String> = Defaults
                    .get("ExcludedApps")
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                ids.retain(|v| *v != id);
                if excluded {
                    ids.push(id);
                }
                Defaults.set("ExcludedApps", json!(ids))?;
                host::with(|h| h.reload());
            }
            UiCommand::ResetExcludedApps => {
                Defaults.set("ExcludedApps", json!(crate::policy::EXCLUDED))?;
                host::with(|h| h.reload());
            }
            UiCommand::AddExcludedApp => unsafe {
                let panel: Obj = msg_send![class!(NSOpenPanel), openPanel];
                let _: () = msg_send![&*panel,setCanChooseDirectories:false];
                let _: () = msg_send![&*panel,setCanChooseFiles:true];
                let _: () = msg_send![&*panel,setAllowsMultipleSelection:false];
                let _: () = msg_send![&*panel,setTreatsFilePackagesAsDirectories:false];
                let _: () = msg_send![&*application(),activateIgnoringOtherApps:true];
                let result: isize = msg_send![&*panel, runModal];
                if result == 1 {
                    let url: Option<Obj> = msg_send![&*panel, URL];
                    if let Some(url) = url {
                        let bundle: Option<Obj> = msg_send![class!(NSBundle),bundleWithURL:&*url];
                        if let Some(bundle) = bundle {
                            let id: Option<Retained<NSString>> =
                                msg_send![&*bundle, bundleIdentifier];
                            if let Some(id) = id {
                                self.request(UiCommand::SetExcluded {
                                    id: id.to_string(),
                                    excluded: true,
                                })?;
                            }
                        }
                    }
                }
            },
            UiCommand::CheckUpdates => self.updater.check(),
            UiCommand::GrantPermission => {
                host::request_accessibility();
                ui::open_url(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                );
            }
            UiCommand::OpenLink(url) if url.starts_with("https://") => {
                ui::open_url(&url);
            }
            UiCommand::PickerClosed => {
                host::with(|h| h.session.emoji_open = false);
            }
            UiCommand::InsertEmoji { text, base } => {
                host::with(|h| h.session.emoji_open = false);
                if text.len() <= 256 && !text.contains(['\0', '\r', '\n']) {
                    let target = self.target;
                    let when =
                        dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(30))
                            .expect("bounded delay");
                    let _ = dispatch2::DispatchQueue::main().after(when, move || {
                        if front_pid() == target {
                            host::with(|h| h.insert_text(&text));
                        }
                    });
                    let mut usage: BTreeMap<String, f64> = Defaults
                        .get("EmojiUsage")
                        .and_then(|v| serde_json::from_value(v).ok())
                        .unwrap_or_default();
                    sevi_runtime::emoji::record_usage(&mut usage, &base);
                    Defaults.set("EmojiUsage", json!(usage))?;
                }
            }
            UiCommand::Quit => unsafe {
                let _: () = msg_send![&*application(),terminate:std::ptr::null::<AnyObject>()];
            },
            _ => (),
        }
        Ok(())
    }
    fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if !self.permission && self.tick_count.is_multiple_of(10) && host::trusted() {
            self.permission = host::with(|h| h.start()).unwrap_or(false);
            self.menu();
        }
        if host::with(|h| {
            let changed = h.language_changed;
            h.language_changed = false;
            changed
        })
        .unwrap_or(false)
        {
            self.menu();
            let vi = host::with(|h| h.session.language() == Language::Vietnamese).unwrap_or(true);
            self.client.send(UiEvent::Language(vi));
        }
        if !self.client.connected() {
            return;
        }
        let requests = self.client.requests();
        let changed = !requests.is_empty();
        for request in requests {
            if let Err(e) = self.request(request) {
                self.client.send(UiEvent::Error(e.to_string()));
            }
        }
        UI_PROCESS.store(self.client.pid().unwrap_or(0), Ordering::Relaxed);
        if self.client.picker_pid().is_none() {
            host::with(|h| h.session.emoji_open = false);
        }
        if let Some(rx) = &self.browser_access
            && let Ok(denied) = rx.try_recv()
        {
            self.browser_access = None;
            if denied {
                let _ = Defaults.set_int("SmartSwitchPerSite", 0);
                host::with(|h| h.reload());
                self.client.send(UiEvent::Error("Allow Sevi to read browser addresses in System Settings > Privacy & Security > Automation, then enable website memory again.".into()));
            }
        }
        if !self.client.connected() {
            UI_PROCESS.store(0, Ordering::Relaxed);
            host::with(|h| h.session.emoji_open = false);
            self.last_snapshot.clear();
            return;
        }
        // Preferences, installed-app discovery and serialization run only while
        // a window is open, at most once per second or after a command.
        if changed || self.tick_count.is_multiple_of(10) {
            let snapshot = self.snapshot();
            if let Ok(encoded) = serde_json::to_string(&snapshot)
                && encoded != self.last_snapshot
            {
                self.last_snapshot = encoded;
                self.client.send(UiEvent::Snapshot(Box::new(snapshot)));
            }
        }
    }
}
fn running_browser_scripts() -> Vec<String> {
    unsafe {
        let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let apps: Obj = msg_send![&*workspace, runningApplications];
        let count: usize = msg_send![&*apps, count];
        let mut scripts = vec![];
        for index in 0..count {
            let app: Obj = msg_send![&*apps,objectAtIndex:index];
            let id: Option<Retained<NSString>> = msg_send![&*app, bundleIdentifier];
            if let Some(id) = id
                && let Some(script) = crate::policy::browser_script(&id.to_string())
            {
                scripts.push(script);
            }
        }
        scripts
    }
}
fn excluded_apps() -> Vec<AppEntry> {
    let ids: Vec<String> = Defaults
        .get("ExcludedApps")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let mut entries: BTreeMap<String, AppEntry> = ids
        .iter()
        .map(|id| {
            (
                id.clone(),
                AppEntry {
                    id: id.clone(),
                    name: id.clone(),
                    excluded: true,
                },
            )
        })
        .collect();
    unsafe {
        let ws: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let apps: Obj = msg_send![&*ws, runningApplications];
        let count: usize = msg_send![&*apps, count];
        for index in 0..count {
            let app: Obj = msg_send![&*apps,objectAtIndex:index];
            let id: Option<Retained<NSString>> = msg_send![&*app, bundleIdentifier];
            let name: Option<Retained<NSString>> = msg_send![&*app, localizedName];
            if let Some(id) = id {
                let id = id.to_string();
                entries.insert(
                    id.clone(),
                    AppEntry {
                        name: name.map(|s| s.to_string()).unwrap_or_else(|| id.clone()),
                        excluded: ids.contains(&id),
                        id,
                    },
                );
            }
        }
    }
    entries.into_values().collect()
}
fn status_icon(vietnamese: bool) -> Obj {
    unsafe {
        let a: Allocated<AnyObject> = msg_send![class!(NSImage), alloc];
        let image: Obj = msg_send![a,initWithSize:NSSize::new(18.,16.)];
        let _: () = msg_send![&*image, lockFocus];
        let color: Obj = msg_send![class!(NSColor), blackColor];
        let _: () = msg_send![&*color, setFill];
        let path: Obj = msg_send![class!(NSBezierPath),bezierPathWithRoundedRect:ui::rect(1.,0.5,16.,15.),xRadius:3.5f64,yRadius:3.5f64];
        let _: () = msg_send![&*path, fill];
        let font: Obj = msg_send![class!(NSFont),boldSystemFontOfSize:11.5f64];
        let attrs: Obj = msg_send![class!(NSMutableDictionary), dictionary];
        let _: () = msg_send![&*attrs,setObject:&*font,forKey:&*NSString::from_str("NSFont")];
        let a: Allocated<AnyObject> = msg_send![class!(NSAttributedString), alloc];
        let text: Obj = msg_send![a,initWithString:&*NSString::from_str(if vietnamese{"V"}else{"E"}),attributes:&*attrs];
        let size: NSSize = msg_send![&*text, size];
        let context: Obj = msg_send![class!(NSGraphicsContext), currentContext];
        let _: () = msg_send![&*context,setCompositingOperation:8usize];
        let _: () = msg_send![&*text,drawAtPoint:NSPoint::new(9.-size.width/2.,8.-size.height/2.)];
        let _: () = msg_send![&*image, unlockFocus];
        let _: () = msg_send![&*image,setTemplate:true];
        image
    }
}

//! Existing offscreen screenshot/debug entrypoints. Never installs an event tap.
use super::{app, prefs::Defaults, ui::Obj};
use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSData, NSPoint, NSString};
use sevi_runtime::prefs::Preferences;
use std::path::Path;
pub fn requested() -> bool {
    cfg!(debug_assertions)
        && (!Defaults.string("emojiSelfTest").is_empty()
            || !Defaults.string("renderMainWindow").is_empty())
}
pub fn run() {
    let dir = Defaults.string("renderMainWindow");
    if !dir.is_empty() {
        let dir = Path::new(&dir);
        let _ = std::fs::create_dir_all(dir);
        let suffix = if Defaults.int("renderDark", 0) != 0 {
            "-dark"
        } else {
            ""
        };
        for tab in ["general", "auto", "viet", "emoji", "excluded"] {
            let mut window = None;
            app::with(|a| window = Some(a.offscreen_main(tab)));
            if let Some(w) = window {
                unsafe {
                    let view: Obj = msg_send![&*w, contentView];
                    let frame: Obj = msg_send![&*view, superview];
                    snapshot(&frame, &dir.join(format!("tab-{tab}{suffix}.png")));
                }
            }
        }
        return;
    }
    let dir = Defaults.string("emojiSelfTest");
    if dir.is_empty() {
        return;
    }
    let dir = Path::new(&dir);
    let _ = std::fs::create_dir_all(dir);
    let mut view = None;
    app::with(|a| view = Some(a.picker.offscreen()));
    let Some(view) = view else { return };
    let mut log = String::new();
    let mut n = 0;
    let mut snap = |what: &str| {
        n += 1;
        snapshot(&view, &dir.join(format!("{n:02}.png")));
        let mut line = String::new();
        app::with(|a| line = a.picker.debug_summary());
        log.push_str(&format!("{n:02} {what}: {line}\n"));
    };
    snap("open");
    type_text("laugh");
    snap("type laugh");
    for (code, name) in [(124, "→"), (124, "→"), (125, "↓"), (123, "←"), (126, "↑")] {
        key(code, "", 0);
        snap(name)
    }
    key(0, "a", 1 << 20);
    snap("⌘A");
    type_text("tim");
    snap("type tim (replaces)");
    key(51, "", 1 << 20);
    snap("⌘⌫ clears");
    for (n, name, code) in [
        (5, "↓×5 in browse", 125),
        (12, "↓×12 more", 125),
        (17, "↑×17 back", 126),
        (15, "↓×15 then close", 125),
    ] {
        for _ in 0..n {
            key(code, "", 0)
        }
        snap(name)
    }
    app::with(|a| {
        a.picker.offscreen();
    });
    snap("reopen: top of the list");
    type_text("nguwaj");
    snap("type nguwaj");
    type_text(" dua");
    snap("type ' dua'");
    key(53, "", 0);
    snap("esc clears the search");
    let _ = std::fs::write(dir.join("log.txt"), log);
}
fn type_text(s: &str) {
    for c in s.chars() {
        key(0, &c.to_string(), 0)
    }
}
fn key(code: u16, chars: &str, flags: u64) {
    unsafe {
        let event: Option<Obj> = msg_send![class!(NSEvent),keyEventWithType:10usize,location:NSPoint::new(0.,0.),modifierFlags:flags,timestamp:0.0f64,windowNumber:0isize,context:std::ptr::null::<AnyObject>(),characters:&*NSString::from_str(chars),charactersIgnoringModifiers:&*NSString::from_str(chars),isARepeat:false,keyCode:code];
        if let Some(event) = event {
            app::with(|a| a.picker.key(&event));
        }
    }
}
fn snapshot(view: &AnyObject, path: &Path) {
    unsafe {
        let _: () = msg_send![view, layoutSubtreeIfNeeded];
        let until: Obj = msg_send![class!(NSDate),dateWithTimeIntervalSinceNow:0.35f64];
        let loop_: Obj = msg_send![class!(NSRunLoop), mainRunLoop];
        let _: () = msg_send![&*loop_,runUntilDate:&*until];
        let bounds: objc2_foundation::NSRect = msg_send![view, bounds];
        let rep: Option<Obj> = msg_send![view,bitmapImageRepForCachingDisplayInRect:bounds];
        if let Some(rep) = rep {
            let _: () = msg_send![view,cacheDisplayInRect:bounds,toBitmapImageRep:&*rep];
            let properties: Obj = msg_send![class!(NSDictionary), dictionary];
            let data: Option<Retained<NSData>> =
                msg_send![&*rep,representationUsingType:4usize,properties:&*properties];
            if let Some(data) = data {
                let _ = std::fs::write(path, data.to_vec());
            }
        }
    }
}

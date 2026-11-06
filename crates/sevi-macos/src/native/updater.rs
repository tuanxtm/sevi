//! The existing Sparkle framework still owns signed downloads and installation.
use objc2::{
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::NSString;
pub struct Updater(Option<Retained<AnyObject>>);
impl Updater {
    pub fn available(&self) -> bool {
        self.0.is_some()
    }
    pub fn disabled() -> Self {
        Self(None)
    }
    pub fn new() -> Self {
        unsafe {
            let bundle: Retained<AnyObject> = msg_send![objc2::class!(NSBundle), mainBundle];
            let feed: Option<Retained<NSString>> =
                msg_send![&*bundle, objectForInfoDictionaryKey:&*NSString::from_str("SUFeedURL")];
            if feed.is_none_or(|url| url.to_string().trim().is_empty()) {
                return Self::disabled();
            }
            let path: Option<Retained<NSString>> = msg_send![&*bundle, privateFrameworksPath];
            if let Some(path) = path {
                let path = NSString::from_str(&format!("{path}/Sparkle.framework"));
                let framework: Option<Retained<AnyObject>> =
                    msg_send![objc2::class!(NSBundle),bundleWithPath:&*path];
                if let Some(f) = framework {
                    let _: bool = msg_send![&*f, load];
                }
            }
            let c = AnyClass::get(c"SPUStandardUpdaterController");
            Self(c.and_then(|c| {
                let a: Allocated<AnyObject>=msg_send![c,alloc];
                msg_send![a,initWithStartingUpdater:true,updaterDelegate:std::ptr::null::<AnyObject>(),userDriverDelegate:std::ptr::null::<AnyObject>()]
            }))
        }
    }
    fn updater(&self) -> Option<Retained<AnyObject>> {
        self.0.as_ref().map(|c| unsafe { msg_send![&**c, updater] })
    }
    pub fn automatic(&self) -> bool {
        self.updater().is_some_and(|u| unsafe {
            let check: bool = msg_send![&*u, automaticallyChecksForUpdates];
            let download: bool = msg_send![&*u, automaticallyDownloadsUpdates];
            check && download
        })
    }
    pub fn set_automatic(&self, on: bool) {
        if let Some(u) = self.updater() {
            unsafe {
                let _: () = msg_send![&*u,setAutomaticallyChecksForUpdates:on];
                let _: () = msg_send![&*u,setAutomaticallyDownloadsUpdates:on];
            }
        }
    }
    pub fn check(&self) {
        if let Some(u) = self.updater() {
            unsafe {
                let _: () = msg_send![&*u, checkForUpdates];
            }
        }
    }
}
#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}
pub fn login_enabled() -> bool {
    unsafe {
        let service: Retained<AnyObject> = msg_send![objc2::class!(SMAppService), mainAppService];
        let status: isize = msg_send![&*service, status];
        status == 1
    }
}
pub fn set_login(on: bool) {
    unsafe {
        let service: Retained<AnyObject> = msg_send![objc2::class!(SMAppService), mainAppService];
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let success: bool = if on {
            msg_send![&*service,registerAndReturnError:&mut error]
        } else {
            msg_send![&*service,unregisterAndReturnError:&mut error]
        };
        if !success && !error.is_null() {
            let s: Retained<NSString> = msg_send![error, localizedDescription];
            eprintln!("Sevi: launch at login change failed: {s}");
        }
    }
}

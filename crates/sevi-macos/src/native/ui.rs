//! Small platform services. Application widgets live in sevi-ui.
use objc2::{
    class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
pub type Obj = Retained<AnyObject>;
pub fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}
pub fn object(name: &str) -> Obj {
    let name = std::ffi::CString::new(name).expect("system class name");
    unsafe {
        let class = objc2::runtime::AnyClass::get(&name).expect("system AppKit class");
        let object: Allocated<AnyObject> = msg_send![class, alloc];
        msg_send![object, init]
    }
}
pub fn open_url(url: &str) {
    unsafe {
        let workspace: Obj = msg_send![class!(NSWorkspace), sharedWorkspace];
        let url: Option<Obj> = msg_send![class!(NSURL),URLWithString:&*NSString::from_str(url)];
        if let Some(url) = url {
            let _: bool = msg_send![&*workspace,openURL:&*url];
        }
    }
}

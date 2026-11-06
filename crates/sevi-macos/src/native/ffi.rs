//! Stable CoreGraphics, CoreFoundation and Carbon C interfaces used by the host.
use std::ffi::c_void;
pub type CF = *mut c_void;
pub const EVENT_TAG: i64 = 0x0f341991;
pub const USER_DATA: u32 = 42;
pub const SOURCE_STATE: u32 = 45;
pub const KEY_CODE: u32 = 9;
pub type TapCallback = unsafe extern "C" fn(CF, u32, CF, CF) -> CF;
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn IsSecureEventInputEnabled() -> bool;
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CF) -> bool;
    pub static kAXTrustedCheckOptionPrompt: CF;
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: TapCallback,
        context: CF,
    ) -> CF;
    pub fn CGEventTapEnable(tap: CF, enable: bool);
    pub fn CGEventTapIsEnabled(tap: CF) -> bool;
    pub fn CGEventSourceCreate(state: i32) -> CF;
    pub fn CGEventSourceGetSourceStateID(source: CF) -> i32;
    pub fn CGEventCreateKeyboardEvent(source: CF, key: u16, down: bool) -> CF;
    pub fn CGEventSetIntegerValueField(event: CF, field: u32, value: i64);
    pub fn CGEventGetIntegerValueField(event: CF, field: u32) -> i64;
    pub fn CGEventGetFlags(event: CF) -> u64;
    pub fn CGEventSetFlags(event: CF, flags: u64);
    pub fn CGEventKeyboardSetUnicodeString(event: CF, length: usize, units: *const u16);
    pub fn CGEventTapPostEvent(proxy: CF, event: CF);
    pub fn CGEventPost(location: u32, event: CF);
    pub fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> CF;
    pub fn CGWindowLevelForKey(key: i32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFRelease(value: CF);
    pub fn CFMachPortCreateRunLoopSource(allocator: CF, port: CF, order: isize) -> CF;
    pub fn CFMachPortIsValid(port: CF) -> bool;
    pub fn CFMachPortInvalidate(port: CF);
    pub fn CFRunLoopGetMain() -> CF;
    pub fn CFRunLoopAddSource(run_loop: CF, source: CF, mode: CF);
    pub fn CFRunLoopRemoveSource(run_loop: CF, source: CF, mode: CF);
    pub static kCFRunLoopCommonModes: CF;
    pub fn CFArrayGetCount(array: CF) -> isize;
    pub fn CFArrayGetValueAtIndex(array: CF, index: isize) -> CF;
}
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn TISCopyCurrentKeyboardLayoutInputSource() -> CF;
    pub fn TISCopyCurrentKeyboardInputSource() -> CF;
    pub fn TISGetInputSourceProperty(source: CF, property: CF) -> CF;
    pub static kTISPropertyInputSourceID: CF;
    pub static kTISPropertyInputSourceLanguages: CF;
    pub static kTISNotifySelectedKeyboardInputSourceChanged: CF;
}

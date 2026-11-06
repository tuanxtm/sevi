//! macOS desktop adapter. Portable policies stay testable without a Mac.
#[cfg(target_os = "macos")]
pub mod native;
pub mod policy;

/// Xcode links this entry as `_main`; Cargo's standalone binary uses its normal entry.
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn sevi_macos_main(_: i32, _: *const *const std::ffi::c_char) -> i32 {
    if sevi_ui::is_client() {
        return match sevi_ui::run_client() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Sevi UI: {e}");
                1
            }
        };
    }
    native::run();
    0
}

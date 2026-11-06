#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    if sevi_ui::is_client() {
        return sevi_ui::run_client().map_err(|e| {
            windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
        });
    }
    sevi_windows::app::run()
}
#[cfg(not(windows))]
fn main() {
    eprintln!("The Sevi Windows desktop host requires Windows.");
    std::process::exit(1);
}

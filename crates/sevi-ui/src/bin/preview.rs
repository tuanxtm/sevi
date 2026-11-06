#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() {
    let result = if sevi_ui::is_client() {
        sevi_ui::run_client()
    } else {
        sevi_ui::preview()
    };
    if let Err(error) = result {
        eprintln!("Sevi UI: {error}");
        std::process::exit(1);
    }
}

mod app;
// The former native widget implementations remain as reference files; only the
// shared renderer is used by the desktop application.
pub mod ffi;
pub mod host;
pub mod prefs;
mod ui;
mod updater;
pub fn run() {
    app::run();
}

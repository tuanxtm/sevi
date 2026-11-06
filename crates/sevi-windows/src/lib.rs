//! Native Windows platform adapters; no Win32 calls enter the portable engine.
#[cfg(windows)]
pub mod app;
pub mod browser;
pub mod dpi;
#[cfg(windows)]
pub mod emoji;
pub mod feed;
#[cfg(windows)]
pub mod host;
#[cfg(any(windows, test))]
mod insertion;
pub mod keys;
#[cfg(windows)]
pub mod prefs;
#[cfg(windows)]
pub mod shared_ui;
#[cfg(windows)]
pub mod site;
#[cfg(any(windows, test))]
mod storage;
#[cfg(windows)]
pub mod ui;
#[cfg(windows)]
pub mod updater;

#[cfg(windows)]
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

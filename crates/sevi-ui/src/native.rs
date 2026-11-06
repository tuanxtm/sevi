//! Window focus and colored glyph services. Widgets never live here.
use sevi_ui_protocol::PickerInput;
use std::sync::mpsc;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub(crate) fn configure(
    frame: &eframe::Frame,
    picker: bool,
    ctx: &egui::Context,
    events: mpsc::Sender<PickerInput>,
) -> u64 {
    #[cfg(windows)]
    {
        windows::configure(frame, picker, ctx, events)
    }
    #[cfg(target_os = "macos")]
    {
        macos::configure(frame, picker, ctx, events)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = (frame, picker, ctx, events);
        0
    }
}
pub(crate) fn glyph(text: &str, pixels: u32) -> Option<egui::ColorImage> {
    #[cfg(windows)]
    {
        windows::glyph(text, pixels)
    }
    #[cfg(target_os = "linux")]
    {
        linux::glyph(text, pixels)
    }
    #[cfg(target_os = "macos")]
    {
        macos::glyph(text, pixels)
    }
}
pub(crate) fn filter(index: &mut sevi_runtime::emoji::EmojiIndex) {
    #[cfg(windows)]
    windows::filter(index);
    #[cfg(target_os = "macos")]
    macos::filter(index);
    #[cfg(target_os = "linux")]
    let _ = index;
}
pub(crate) fn clipboard() -> Option<String> {
    #[cfg(windows)]
    {
        windows::clipboard()
    }
    #[cfg(target_os = "macos")]
    {
        macos::clipboard()
    }
    #[cfg(target_os = "linux")]
    {
        None
    } // Linux receives clipboard events through winit.
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn bgra(bytes: &[u8], side: usize) -> egui::ColorImage {
    let mut rgba = Vec::with_capacity(bytes.len());
    for p in bytes.chunks_exact(4) {
        let a = u32::from(p[3]);
        let unpremul = |c: u8| (u32::from(c) * 255).checked_div(a).unwrap_or(0).min(255) as u8;
        rgba.extend_from_slice(&[unpremul(p[2]), unpremul(p[1]), unpremul(p[0]), p[3]]);
    }
    egui::ColorImage::from_rgba_unmultiplied([side, side], &rgba)
}
pub(crate) fn close_picker() {
    #[cfg(target_os = "macos")]
    macos::close_picker();
}
#[cfg(target_os = "macos")]
pub(crate) use macos::paint;

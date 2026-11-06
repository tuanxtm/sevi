use pangocairo::{cairo, pango};
pub(super) fn glyph(text: &str, pixels: u32) -> Option<egui::ColorImage> {
    let side = pixels as i32;
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, side, side).ok()?;
    let ctx = cairo::Context::new(&surface).ok()?;
    let layout = pangocairo::functions::create_layout(&ctx);
    let mut font = pango::FontDescription::from_string("Noto Color Emoji");
    font.set_absolute_size((pixels as f64 * 0.73) * pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    layout.set_text(text);
    let (w, h) = layout.pixel_size();
    ctx.set_source_rgba(0., 0., 0., 1.);
    ctx.move_to(f64::from(side - w) / 2., f64::from(side - h) / 2.);
    pangocairo::functions::show_layout(&ctx, &layout);
    drop(layout);
    drop(ctx);
    surface.flush();
    let bytes = surface.data().ok()?;
    Some(super::bgra(&bytes, pixels as usize))
}

//! Compact text actions and menus shared by every desktop platform.
use crate::{chrome::hud, theme::Palette};
use egui::{Response, Sense, Stroke, pos2, vec2};

fn text(ui: &mut egui::Ui, label: &str, arrow: bool) -> Response {
    let p = Palette::get(ui.ctx());
    let ink = if ui.is_enabled() { p.ink } else { p.faint };
    let galley = hud(ui, label, ink, false);
    let size = vec2(galley.size().x + if arrow { 17. } else { 0. }, 24.);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.);
    let width = galley.size().x;
    ui.painter().galley(at, galley, ink);
    if response.hovered() || response.has_focus() {
        ui.painter()
            .hline(at.x..=at.x + width, at.y + 15., Stroke::new(1., ink));
    }
    if arrow {
        let c = pos2(rect.right() - 4., rect.center().y);
        ui.painter().add(egui::Shape::line(
            vec![c + vec2(-3., -2.), c + vec2(0., 1.), c + vec2(3., -2.)],
            Stroke::new(1., ink),
        ));
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub(crate) fn action(ui: &mut egui::Ui, label: &str) -> Response {
    text(ui, label, false)
}
pub(crate) fn trigger(ui: &mut egui::Ui, label: &str) -> Response {
    text(ui, label, true)
}
pub(crate) fn menu(response: &Response, contents: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::get(&response.ctx);
    egui::Popup::menu(response)
        .width(180.)
        .align(egui::emath::RectAlign::BOTTOM_END)
        .frame(
            egui::Frame::new()
                .fill(p.background)
                .stroke(p.hairline())
                .inner_margin(4),
        )
        .show(contents);
}
pub(crate) fn option(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let p = Palette::get(ui.ctx());
    let galley = hud(ui, label, p.ink, false);
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width().max(galley.size().x + 20.), 28.),
        Sense::click(),
    );
    let (fill, ink) = if selected {
        (p.invert_bg, p.invert_fg)
    } else if response.hovered() || response.has_focus() {
        (p.sunken, p.strong)
    } else {
        (p.background, p.ink)
    };
    ui.painter().rect_filled(rect, 0., fill);
    ui.painter().galley_with_override_text_color(
        pos2(rect.left() + 10., rect.center().y - galley.size().y / 2.),
        galley,
        ink,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    if response.clicked() {
        ui.close();
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub(crate) fn back(ui: &mut egui::Ui) -> Response {
    let p = Palette::get(ui.ctx());
    let galley = hud(ui, "BACK", p.ink, false);
    let (rect, response) = ui.allocate_exact_size(vec2(20. + galley.size().x, 24.), Sense::click());
    let c = pos2(rect.left() + 4., rect.center().y);
    let stroke = Stroke::new(1., p.ink);
    ui.painter()
        .line_segment([c + vec2(-4., 0.), c + vec2(4., 0.)], stroke);
    ui.painter().add(egui::Shape::line(
        vec![c + vec2(-1., -3.), c + vec2(-4., 0.), c + vec2(-1., 3.)],
        stroke,
    ));
    let at = pos2(rect.left() + 20., rect.center().y - galley.size().y / 2.);
    let width = galley.size().x;
    ui.painter().galley(at, galley, p.ink);
    if response.hovered() || response.has_focus() {
        ui.painter().hline(at.x..=at.x + width, at.y + 15., stroke);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Back"));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
/// Retain egui's label layout, keyboard behavior and checked accessibility state,
/// replacing only the painted tick with a solid inset square.
pub(crate) fn checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> Response {
    let response = ui.checkbox(checked, label);
    if ui.is_rect_visible(response.rect) {
        let p = Palette::get(ui.ctx());
        let side = ui.spacing().icon_width;
        let icon = egui::Rect::from_center_size(
            pos2(response.rect.left() + side / 2., response.rect.center().y),
            vec2(side, side),
        );
        ui.painter().rect_filled(icon, 0., p.background);
        let line = if response.hovered() || response.has_focus() {
            p.line_strong
        } else {
            p.line
        };
        ui.painter()
            .rect_stroke(icon, 0., Stroke::new(1., line), egui::StrokeKind::Inside);
        if *checked {
            ui.painter().rect_filled(
                icon.shrink(4.),
                0.,
                if ui.is_enabled() { p.ink } else { p.faint },
            );
        }
    }
    response
}

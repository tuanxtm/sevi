//! Edge-to-edge Craft Launcher chrome adapted for Sevi.
//! Derived from storytold/craft-launcher views.rs/widgets.rs (MIT).
//! See assets/LICENSE-Craft-Launcher-MIT.txt and CRAFT_SOURCE.md.
use crate::theme::{self, Palette};
use egui::{Color32, CornerRadius, FontId, Rect, Sense, Stroke, pos2, vec2};
use sevi_ui_protocol::View;
use std::sync::Arc;

const CAPTION_W: f32 = 46.;
const LANGUAGE_W: f32 = 48.;
const EDGE: f32 = 5.;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}
impl Caption {
    fn label(self) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Maximize => "Maximize",
            Self::Restore => "Restore",
            Self::Close => "Close",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Action {
    Language(bool),
    Caption(Caption),
}
pub(crate) struct HeaderLayout {
    pub subtitle: Rect,
    pub languages: [Rect; 2],
    pub captions: [Rect; 3],
}
impl HeaderLayout {
    pub fn new(rect: Rect) -> Self {
        let x = rect.right() - CAPTION_W * 3. - LANGUAGE_W * 2.;
        let cell = |left: f32, width: f32| {
            Rect::from_min_size(pos2(left, rect.top()), vec2(width, rect.height()))
        };
        Self {
            subtitle: cell(rect.left(), x - rect.left()),
            languages: [cell(x, LANGUAGE_W), cell(x + LANGUAGE_W, LANGUAGE_W)],
            captions: [
                cell(x + 2. * LANGUAGE_W, CAPTION_W),
                cell(x + 2. * LANGUAGE_W + CAPTION_W, CAPTION_W),
                cell(rect.right() - CAPTION_W, CAPTION_W),
            ],
        }
    }
}
pub(crate) fn hud(ui: &egui::Ui, text: &str, color: Color32, bold: bool) -> Arc<egui::Galley> {
    let format = egui::TextFormat {
        font_id: FontId::new(
            theme::HUD_SIZE,
            if bold {
                theme::mono_bold()
            } else {
                theme::mono()
            },
        ),
        color,
        extra_letter_spacing: theme::HUD_SPACING,
        ..Default::default()
    };
    ui.painter()
        .layout_job(egui::text::LayoutJob::single_section(
            text.to_uppercase(),
            format,
        ))
}
pub(crate) fn heading(ui: &mut egui::Ui, text: &str, p: Palette) {
    let galley = hud(ui, text, p.muted, false);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), galley.size().y + 7.),
        Sense::hover(),
    );
    ui.painter().galley(rect.min, galley, p.muted);
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), p.hairline());
}
pub(crate) fn label(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(egui::Label::new(hud(ui, text, color, false)))
}
fn centre(ui: &egui::Ui, rect: Rect, galley: Arc<egui::Galley>, color: Color32) {
    ui.painter()
        .galley_with_override_text_color(rect.center() - galley.size() / 2., galley, color);
}
pub(crate) fn header(ui: &mut egui::Ui, vietnamese: bool) -> Option<Action> {
    let p = Palette::get(ui.ctx());
    let rect = ui.max_rect();
    let layout = HeaderLayout::new(rect);
    let drag = Rect::from_min_max(
        rect.min + vec2(EDGE, EDGE),
        pos2(layout.languages[0].left(), rect.bottom()),
    );
    let response = ui.interact(
        drag,
        ui.scope_id().with("title-drag"),
        Sense::click_and_drag(),
    );
    if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
            !ui.input(|i| i.viewport().maximized.unwrap_or(false)),
        ));
    }
    let subtitle = hud(ui, "smart english/vietnamese input", p.muted, false);
    let label = Rect::from_min_max(
        layout.subtitle.min + vec2(20., 0.),
        layout.subtitle.max - vec2(16., 0.),
    );
    let painter = ui.painter().with_clip_rect(label);
    painter.galley(
        pos2(label.left(), label.center().y - subtitle.size().y / 2.),
        subtitle,
        p.muted,
    );
    let mut action = None;
    for (index, (label, vi)) in [("VI", true), ("EN", false)].into_iter().enumerate() {
        let cell = layout.languages[index];
        let band = if ui.input(|i| i.viewport().maximized.unwrap_or(false)) {
            0.
        } else {
            EDGE
        };
        let response = ui.interact(
            Rect::from_min_max(cell.min + vec2(0., band), cell.max),
            ui.scope_id().with(("language", vi)),
            Sense::click(),
        );
        let selected = vietnamese == vi;
        let (fill, ink) = if selected {
            (p.invert_bg, p.invert_fg)
        } else if response.hovered() {
            (p.sunken, p.strong)
        } else {
            (Color32::TRANSPARENT, p.muted)
        };
        ui.painter().rect_filled(cell, CornerRadius::ZERO, fill);
        centre(ui, cell, hud(ui, label, ink, true), ink);
        ui.painter()
            .vline(cell.left(), cell.y_range(), p.hairline());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::Button,
                true,
                selected,
                if vi { "Vietnamese" } else { "English" },
            )
        });
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            action = Some(Action::Language(vi));
        }
    }
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    for (rect, caption) in layout.captions.into_iter().zip([
        Caption::Minimize,
        if maximized {
            Caption::Restore
        } else {
            Caption::Maximize
        },
        Caption::Close,
    ]) {
        let band = if maximized { 0. } else { EDGE };
        let hit = Rect::from_min_max(
            rect.min + vec2(0., band),
            rect.max - vec2(if caption == Caption::Close { band } else { 0. }, 0.),
        );
        let response = ui.interact(hit, ui.scope_id().with(caption.label()), Sense::click());
        let (fill, ink) = if response.hovered() {
            if caption == Caption::Close {
                (Color32::from_rgb(196, 43, 28), Color32::WHITE)
            } else {
                (p.line, p.strong)
            }
        } else {
            (Color32::TRANSPARENT, p.muted)
        };
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
        ui.painter().extend(caption_shapes(rect, caption, ink));
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, caption.label()));
        if response.clicked() {
            action = Some(Action::Caption(caption));
        }
    }
    ui.painter()
        .vline(layout.captions[0].left(), rect.y_range(), p.hairline());
    ui.painter()
        .hline(rect.x_range(), rect.bottom() - 0.5, p.hairline());
    action
}
fn caption_shapes(rect: Rect, caption: Caption, ink: Color32) -> Vec<egui::Shape> {
    let c = rect.center();
    let s = 5.;
    let stroke = Stroke::new(1., ink);
    match caption {
        Caption::Minimize => vec![egui::Shape::line_segment(
            [c + vec2(-s, 0.), c + vec2(s, 0.)],
            stroke,
        )],
        Caption::Maximize => vec![egui::Shape::rect_stroke(
            Rect::from_center_size(c, vec2(10., 10.)),
            CornerRadius::ZERO,
            stroke,
            egui::StrokeKind::Inside,
        )],
        Caption::Restore => vec![
            egui::Shape::rect_stroke(
                Rect::from_min_size(c + vec2(-s, -s + 2.), vec2(8., 8.)),
                CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            ),
            egui::Shape::line_segment([c + vec2(-s + 2., -s), c + vec2(s, -s)], stroke),
            egui::Shape::line_segment([c + vec2(s, -s), c + vec2(s, s - 2.)], stroke),
        ],
        Caption::Close => vec![
            egui::Shape::line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke),
            egui::Shape::line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke),
        ],
    }
}
pub(crate) fn footer(
    ui: &mut egui::Ui,
    version: &str,
    status: &str,
    current: View,
) -> Option<View> {
    let p = Palette::get(ui.ctx());
    let rect = ui.max_rect();
    let mut x = rect.left() + 20.;
    let mut selected = None;
    for (label, view) in [
        ("HELP", View::Help),
        ("ABOUT", View::About),
        ("CREDITS", View::Credits),
    ] {
        let galley = hud(ui, label, p.muted, false);
        let width = galley.size().x;
        let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(width, rect.height()));
        let response = ui.interact(cell, ui.scope_id().with(label), Sense::click());
        let ink = if response.hovered() || current == view {
            p.strong
        } else {
            p.muted
        };
        centre(ui, cell, galley, ink);
        if response.hovered() || response.has_focus() {
            ui.painter()
                .hline(cell.x_range(), cell.center().y + 8., Stroke::new(1., ink));
        }
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            selected = Some(view);
        }
        x += width + 24.;
    }
    let version = hud(ui, &format!("SEVI V{version}"), p.muted, false);
    let version_left = rect.right() - 8. - version.size().x;
    ui.painter().galley(
        pos2(version_left, rect.center().y - version.size().y / 2.),
        version,
        p.muted,
    );
    if !status.is_empty() {
        let mut job = egui::text::LayoutJob::single_section(
            status.into(),
            egui::TextFormat {
                font_id: FontId::proportional(11.),
                color: p.accent,
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((version_left - x - 16.).max(0.));
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            pos2(x, rect.center().y - galley.size().y / 2.),
            galley,
            p.accent,
        );
    }
    ui.painter()
        .hline(rect.x_range(), rect.top() + 0.5, p.hairline());
    selected
}
pub(crate) fn window_edges(ctx: &egui::Context) {
    use egui::{CursorIcon as C, viewport::ResizeDirection as D};
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    let p = Palette::get(ctx);
    let rect = ctx.content_rect();
    for (edge, band) in [
        rect.with_max_y(rect.top() + EDGE),
        rect.with_min_y(rect.bottom() - EDGE),
        rect.with_max_x(rect.left() + EDGE),
        rect.with_min_x(rect.right() - EDGE),
    ]
    .into_iter()
    .enumerate()
    {
        egui::Area::new(egui::Id::unique(("sevi-window-edge", edge)))
            .order(egui::Order::Tooltip)
            .movable(false)
            .constrain(false)
            .fixed_pos(band.min)
            .show(ctx, |ui| ui.allocate_response(band.size(), Sense::click()));
    }
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("sevi-window-border"),
    ))
    .rect_stroke(
        rect,
        CornerRadius::ZERO,
        p.hairline(),
        egui::StrokeKind::Inside,
    );
    let Some(at) = ctx
        .input(|i| i.pointer.hover_pos())
        .filter(|at| rect.contains(*at))
    else {
        return;
    };
    let (left, right, top, bottom) = (
        at.x - rect.left(),
        rect.right() - at.x,
        at.y - rect.top(),
        rect.bottom() - at.y,
    );
    let near = |d: f32| d < EDGE;
    let corner = |d: f32| d < 3. * EDGE;
    let hit = if near(top) && corner(left) || near(left) && corner(top) {
        Some((D::NorthWest, C::ResizeNorthWest))
    } else if near(top) && corner(right) || near(right) && corner(top) {
        Some((D::NorthEast, C::ResizeNorthEast))
    } else if near(bottom) && corner(left) || near(left) && corner(bottom) {
        Some((D::SouthWest, C::ResizeSouthWest))
    } else if near(bottom) && corner(right) || near(right) && corner(bottom) {
        Some((D::SouthEast, C::ResizeSouthEast))
    } else if near(top) {
        Some((D::North, C::ResizeNorth))
    } else if near(bottom) {
        Some((D::South, C::ResizeSouth))
    } else if near(left) {
        Some((D::West, C::ResizeWest))
    } else if near(right) {
        Some((D::East, C::ResizeEast))
    } else {
        None
    };
    if let Some((direction, cursor)) = hit {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_cells_touch_the_window_edges_and_share_the_same_height() {
        for width in [theme::MAIN_SIZE[0], 900.] {
            let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(width, theme::HEADER_H));
            let h = HeaderLayout::new(rect);
            let cells = [
                h.subtitle,
                h.languages[0],
                h.languages[1],
                h.captions[0],
                h.captions[1],
                h.captions[2],
            ];
            assert_eq!(cells[0].left(), 0.);
            assert_eq!(cells.last().unwrap().right(), width);
            for cell in cells {
                assert_eq!(cell.top(), 0.);
                assert_eq!(cell.height(), theme::HEADER_H);
            }
            for pair in cells.windows(2) {
                assert_eq!(pair[0].right(), pair[1].left());
            }
            for (rect, caption) in
                h.captions
                    .into_iter()
                    .zip([Caption::Minimize, Caption::Restore, Caption::Close])
            {
                for shape in caption_shapes(rect, caption, Color32::WHITE) {
                    assert!(rect.contains_rect(shape.visual_bounding_rect()));
                }
            }
        }
    }
}

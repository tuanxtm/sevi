//! Craft Launcher type and palette, adapted to Sevi's compact settings.
//! Source: storytold/craft-launcher, MIT; see assets/LICENSE-Craft-Launcher-MIT.txt.
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Stroke};
use std::sync::Arc;

pub const MAIN_SIZE: [f32; 2] = [600., 420.];
pub const HEADER_H: f32 = 48.;
pub const FOOTER_H: f32 = 34.;
pub const HUD_SIZE: f32 = 11.;
pub const HUD_SPACING: f32 = HUD_SIZE * 0.04;
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}
pub fn mono() -> FontFamily {
    FontFamily::Name("mono".into())
}
pub fn mono_bold() -> FontFamily {
    FontFamily::Name("mono-bold".into())
}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub background: Color32,
    pub sunken: Color32,
    pub ink: Color32,
    pub strong: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub accent: Color32,
    pub invert_bg: Color32,
    pub invert_fg: Color32,
}
const fn rgb(v: u32) -> Color32 {
    let [_, r, g, b] = v.to_be_bytes();
    Color32::from_rgb(r, g, b)
}
const fn alpha(v: u32, a: u8) -> Color32 {
    let [_, r, g, b] = v.to_be_bytes();
    Color32::from_rgba_unmultiplied_const(r, g, b, a)
}
impl Palette {
    pub fn for_theme(dark: bool) -> Self {
        if dark {
            Self {
                background: rgb(0x121316),
                sunken: rgb(0x060607),
                ink: rgb(0xf2f1ee),
                strong: Color32::WHITE,
                muted: alpha(0xf2f1ee, 153),
                faint: alpha(0xf2f1ee, 97),
                line: alpha(0xf2f1ee, 38),
                line_strong: alpha(0xf2f1ee, 102),
                accent: rgb(0x74aaff),
                invert_bg: Color32::WHITE,
                invert_fg: rgb(0x0b0b0c),
            }
        } else {
            Self {
                background: rgb(0xf2f1ee),
                sunken: rgb(0xe9e8e4),
                ink: rgb(0x101014),
                strong: Color32::BLACK,
                muted: alpha(0x101014, 158),
                faint: alpha(0x101014, 107),
                line: alpha(0x101014, 41),
                line_strong: alpha(0x101014, 102),
                accent: rgb(0x1659c4),
                invert_bg: rgb(0x101014),
                invert_fg: rgb(0xf2f1ee),
            }
        }
    }
    pub fn get(ctx: &egui::Context) -> Self {
        Self::for_theme(ctx.theme() == egui::Theme::Dark)
    }
    pub fn hairline(self) -> Stroke {
        Stroke::new(1., self.line)
    }
}
pub(crate) fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    let faces = [
        (
            "Inter",
            include_bytes!("../assets/Inter.ttf").as_slice(),
            FontFamily::Proportional,
        ),
        (
            "Inter-Medium",
            include_bytes!("../assets/Inter-Medium.ttf").as_slice(),
            medium(),
        ),
        (
            "GeistMono",
            include_bytes!("../assets/GeistMono.ttf").as_slice(),
            FontFamily::Monospace,
        ),
        (
            "GeistMono-Medium",
            include_bytes!("../assets/GeistMono-Medium.ttf").as_slice(),
            mono(),
        ),
        (
            "GeistMono-Bold",
            include_bytes!("../assets/GeistMono-Bold.ttf").as_slice(),
            mono_bold(),
        ),
    ];
    for (name, bytes, family) in faces {
        let tweak = FontTweak {
            subpixel_binning: Some(false),
            ..Default::default()
        };
        fonts.font_data.insert(
            name.into(),
            Arc::new(FontData::from_static(bytes).tweak(tweak)),
        );
        fonts.families.insert(family, vec![name.into()]);
    }
    ctx.set_fonts(fonts);
    for dark in [false, true] {
        let p = Palette::for_theme(dark);
        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = p.background;
        visuals.window_fill = p.background;
        visuals.extreme_bg_color = p.sunken;
        visuals.text_edit_bg_color = Some(p.sunken);
        visuals.faint_bg_color = p.sunken;
        visuals.code_bg_color = p.sunken;
        visuals.override_text_color = Some(p.ink);
        visuals.hyperlink_color = p.accent;
        visuals.window_corner_radius = egui::CornerRadius::ZERO;
        visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        visuals.selection.bg_fill = rgb(0x2d81ff);
        visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
        for (widget, fill, line, ink) in [
            (
                &mut visuals.widgets.noninteractive,
                p.background,
                p.line,
                p.muted,
            ),
            (&mut visuals.widgets.inactive, p.background, p.line, p.ink),
            (
                &mut visuals.widgets.hovered,
                p.sunken,
                p.line_strong,
                p.strong,
            ),
            (
                &mut visuals.widgets.active,
                p.sunken,
                p.line_strong,
                p.strong,
            ),
            (&mut visuals.widgets.open, p.sunken, p.line_strong, p.strong),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::new(1., line);
            widget.fg_stroke = Stroke::new(1., ink);
            widget.corner_radius = egui::CornerRadius::ZERO;
            widget.expansion = 0.;
        }
        let mut style = egui::Style {
            visuals,
            ..Default::default()
        };
        style.spacing.item_spacing = egui::vec2(8., 6.);
        style.spacing.icon_spacing = 10.;
        style.spacing.button_padding = egui::vec2(12., 6.);
        style.spacing.interact_size = egui::vec2(30., 24.);
        style.spacing.icon_width = 17.;
        style.spacing.menu_margin = egui::Margin::same(4);
        style.text_styles = [
            (egui::TextStyle::Heading, FontId::proportional(24.)),
            (egui::TextStyle::Body, FontId::proportional(14.)),
            (egui::TextStyle::Button, FontId::new(13.5, medium())),
            (egui::TextStyle::Small, FontId::proportional(12.)),
            (egui::TextStyle::Monospace, FontId::monospace(13.)),
        ]
        .into();
        ctx.set_style_of(
            if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            },
            style,
        );
    }
}

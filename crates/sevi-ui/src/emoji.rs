use crate::{controls, native, theme::Palette};
use egui::{RichText, TextureHandle};
use sevi_runtime::emoji::{EmojiIndex, frequent};
use sevi_ui_protocol::{PickerInput, Setting, UiCommand, UiSnapshot};
use std::collections::{BTreeMap, VecDeque};
use unicode_segmentation::UnicodeSegmentation;

const COLUMNS: usize = 9;
const GROUPS: [&str; 10] = [
    "Faces",
    "People",
    "Components",
    "Nature",
    "Food",
    "Travel",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];
pub(crate) struct Picker {
    index: EmojiIndex,
    query: String,
    group: Option<usize>,
    selected: usize,
    results: Vec<usize>,
    tone: usize,
    all_selected: bool,
    textures: BTreeMap<(String, u32), TextureHandle>,
    order: VecDeque<(String, u32)>,
    last_query: String,
    last_group: Option<usize>,
    ensure_visible: bool,
    scroll_y: f32,
}
impl Picker {
    pub fn new(snapshot: &UiSnapshot) -> Self {
        let mut index = EmojiIndex::default();
        native::filter(&mut index);
        let results = frequent(&snapshot.emoji_usage, 36)
            .iter()
            .filter_map(|e| index.all.iter().position(|r| r.text == *e))
            .collect();
        Self {
            index,
            query: String::new(),
            group: None,
            selected: 0,
            results,
            tone: snapshot.int(Setting::EmojiSkinTone).clamp(0, 5) as usize,
            all_selected: false,
            textures: BTreeMap::new(),
            order: VecDeque::new(),
            last_query: String::new(),
            last_group: None,
            ensure_visible: true,
            scroll_y: 0.,
        }
    }
    fn search(&mut self, snapshot: &UiSnapshot) {
        if self.query == self.last_query && self.group == self.last_group {
            return;
        }
        self.last_query = self.query.clone();
        self.last_group = self.group;
        self.selected = 0;
        self.ensure_visible = true;
        self.scroll_y = 0.;
        self.results = if !self.query.trim().is_empty() {
            self.index.search(&self.query, 250, None)
        } else if let Some(group) = self.group {
            self.index
                .all
                .iter()
                .enumerate()
                .filter(|(_, e)| e.group == group && group != 2)
                .map(|(i, _)| i)
                .collect()
        } else {
            frequent(&snapshot.emoji_usage, 36)
                .iter()
                .filter_map(|e| self.index.all.iter().position(|r| r.text == *e))
                .collect()
        };
    }
    pub fn input(&mut self, event: PickerInput, snapshot: &UiSnapshot) -> Option<UiCommand> {
        match event {
            PickerInput::Text(text) => {
                if self.all_selected {
                    self.query.clear();
                    self.all_selected = false;
                }
                let remaining = 512usize.saturating_sub(self.query.chars().count());
                self.query
                    .extend(text.chars().filter(|c| !c.is_control()).take(remaining));
            }
            PickerInput::Backspace { word, all } => {
                if all || self.all_selected {
                    self.query.clear();
                    self.all_selected = false;
                } else if word {
                    self.query = self.query.trim_end().to_owned();
                    while !self.query.is_empty() && !self.query.ends_with(' ') {
                        self.query.pop();
                    }
                } else if let Some((at, _)) = self.query.grapheme_indices(true).next_back() {
                    self.query.truncate(at);
                }
            }
            PickerInput::SelectAll => self.all_selected = !self.query.is_empty(),
            PickerInput::Move { dx, dy } => {
                self.ensure_visible = true;
                self.selected = (self.selected as i32 + dx + dy * COLUMNS as i32)
                    .clamp(0, self.results.len().saturating_sub(1) as i32)
                    as usize;
            }
            PickerInput::Escape => {
                self.all_selected = false;
                if self.query.is_empty() {
                    return Some(UiCommand::PickerClosed);
                }
                self.query.clear();
            }
            PickerInput::Enter => return self.choice(self.selected),
            PickerInput::Paste => {
                if let Some(text) = native::clipboard() {
                    if self.all_selected {
                        self.query.clear();
                        self.all_selected = false;
                    }
                    self.query
                        .extend(text.chars().filter(|c| !c.is_control()).take(512));
                }
            }
        }
        self.search(snapshot);
        None
    }
    fn choice(&self, at: usize) -> Option<UiCommand> {
        self.results.get(at).map(|&i| UiCommand::InsertEmoji {
            text: self.index.all[i].with_skin(self.tone).to_owned(),
            base: self.index.all[i].text.clone(),
        })
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &UiSnapshot,
        send: &mut Vec<UiCommand>,
        external_keys: bool,
    ) {
        let palette = Palette::get(ui.ctx());
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("EMOJI")
                    .monospace()
                    .size(11.)
                    .color(palette.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if controls::action(ui, "Close").clicked() {
                    send.push(UiCommand::PickerClosed);
                }
            });
        });
        let response = ui.add_sized(
            [ui.available_width(), 30.],
            egui::TextEdit::singleline(&mut self.query)
                .char_limit(512)
                .hint_text("Search in English, Vietnamese or Telex"),
        );
        if !external_keys {
            response.request_focus();
        }
        self.search(snapshot);
        ui.horizontal(|ui| {
            if controls::action(ui, "Frequent").clicked() {
                self.group = None;
                self.last_query = "\0".into();
            }
            let response = controls::trigger(ui, self.group.map_or("Categories", |g| GROUPS[g]));
            controls::menu(&response, |ui| {
                for (i, name) in GROUPS.iter().enumerate() {
                    if i != 2 && controls::option(ui, name, self.group == Some(i)).clicked() {
                        self.group = Some(i);
                    }
                }
            });
            let old = self.tone;
            let response = controls::trigger(ui, &format!("Tone {}", self.tone));
            controls::menu(&response, |ui| {
                for t in 0..=5 {
                    let label = if t == 0 {
                        "Default".into()
                    } else {
                        format!("Tone {t}")
                    };
                    if controls::option(ui, &label, self.tone == t).clicked() {
                        self.tone = t;
                    }
                }
            });
            if old != self.tone {
                send.push(UiCommand::Set {
                    setting: Setting::EmojiSkinTone,
                    value: self.tone as i32,
                });
            }
        });
        self.search(snapshot);
        ui.separator();
        let mut choice = None;
        let row_count = self.results.len().div_ceil(COLUMNS);
        let height = (ui.available_height() - 28.).max(40.);
        let mut scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(height);
        if self.ensure_visible {
            let top = (self.selected / COLUMNS) as f32 * (37. + ui.spacing().item_spacing.y);
            let offset = if top < self.scroll_y {
                top
            } else if top + 37. > self.scroll_y + height {
                top + 37. - height
            } else {
                self.scroll_y
            };
            scroll = scroll.vertical_scroll_offset(offset);
            self.ensure_visible = false;
        }
        let output = scroll.show_rows(ui, 37., row_count, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    for col in 0..COLUMNS {
                        let at = row * COLUMNS + col;
                        let Some(&index) = self.results.get(at) else {
                            break;
                        };
                        let emoji = &self.index.all[index];
                        let text = emoji.with_skin(self.tone).to_owned();
                        let pixels =
                            (32. * ui.ctx().pixels_per_point()).round().clamp(24., 160.) as u32;
                        let key = (text.clone(), pixels);
                        if !self.textures.contains_key(&key)
                            && let Some(image) = native::glyph(&text, pixels)
                        {
                            let texture = ui.ctx().load_texture(
                                format!("emoji-{text}-{pixels}"),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            self.textures.insert(key.clone(), texture);
                            self.order.push_back(key.clone());
                            if self.order.len() > 256
                                && let Some(old) = self.order.pop_front()
                            {
                                self.textures.remove(&old);
                            }
                        }
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(35., 37.), egui::Sense::click());
                        if at == self.selected {
                            ui.painter().rect_filled(rect, 0., palette.line);
                        }
                        if let Some(texture) = self.textures.get(&key) {
                            ui.painter().image(
                                texture.id(),
                                rect.shrink(1.),
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "?",
                                egui::FontId::proportional(20.),
                                palette.muted,
                            );
                        }
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, &emoji.name_en)
                        });
                        if response.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
                        {
                            self.selected = at;
                        }
                        if response.clicked() {
                            choice = Some(at);
                        }
                    }
                });
            }
        });
        self.scroll_y = output.state.clamped_offset().y;
        if let Some(at) = choice
            && let Some(command) = self.choice(at)
        {
            send.push(command);
        }
        ui.separator();
        ui.label(
            RichText::new(
                self.results
                    .get(self.selected)
                    .map(|&i| self.index.all[i].name_en.as_str())
                    .unwrap_or("No emoji found"),
            )
            .small()
            .color(palette.muted),
        );
        if !external_keys {
            let event = ui.input(|i| {
                use egui::Key;
                if i.key_pressed(Key::Escape) {
                    Some(PickerInput::Escape)
                } else if i.key_pressed(Key::Enter) {
                    Some(PickerInput::Enter)
                } else if i.key_pressed(Key::ArrowDown) {
                    Some(PickerInput::Move { dx: 0, dy: 1 })
                } else if i.key_pressed(Key::ArrowUp) {
                    Some(PickerInput::Move { dx: 0, dy: -1 })
                } else if i.key_pressed(Key::ArrowLeft) {
                    Some(PickerInput::Move { dx: -1, dy: 0 })
                } else if i.key_pressed(Key::ArrowRight) {
                    Some(PickerInput::Move { dx: 1, dy: 0 })
                } else {
                    None
                }
            });
            if let Some(event) = event
                && let Some(command) = self.input(event, snapshot)
            {
                send.push(command);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_search_selection_and_grapheme_delete() {
        let s = UiSnapshot::default();
        let mut p = Picker::new(&s);
        p.input(PickerInput::Text("nguwaj".into()), &s);
        assert!(!p.results.is_empty());
        assert!(matches!(
            p.input(PickerInput::Enter, &s),
            Some(UiCommand::InsertEmoji { .. })
        ));
        p.input(PickerInput::SelectAll, &s);
        p.input(PickerInput::Text("a👨‍👩‍👧‍👦".into()), &s);
        p.input(
            PickerInput::Backspace {
                word: false,
                all: false,
            },
            &s,
        );
        assert_eq!(p.query, "a");
        p.input(PickerInput::Escape, &s);
        assert!(p.query.is_empty());
        assert!(matches!(
            p.input(PickerInput::Escape, &s),
            Some(UiCommand::PickerClosed)
        ));
    }
    #[test]
    fn skin_tone_choice_retains_base_usage_key() {
        let s = UiSnapshot::default();
        let mut p = Picker::new(&s);
        let at = p.index.all.iter().position(|e| e.skins.len() == 5).unwrap();
        p.results = vec![at];
        p.tone = 3;
        let Some(UiCommand::InsertEmoji { text, base }) = p.choice(0) else {
            panic!("missing selection")
        };
        assert_eq!(base, p.index.all[at].text);
        assert_eq!(text, p.index.all[at].skins[2]);
        assert_ne!(text, base);
    }
}

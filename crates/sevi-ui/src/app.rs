use crate::{
    chrome, controls,
    theme::{self, Palette},
};
use egui::{Align, RichText};
use sevi_ui_protocol::*;
use std::{
    io::{self, BufReader},
    sync::mpsc,
    thread,
};

pub fn run_client() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(io::stdin());
    let Some(UiEvent::Snapshot(snapshot)) = read::<UiEvent>(&mut reader)? else {
        return Err("Missing UI host handshake.".into());
    };
    if snapshot.protocol != VERSION {
        return Err("Incompatible UI protocol.".into());
    }
    let view = initial_view();
    let (event_tx, event_rx) = mpsc::channel();
    let (command_tx, command_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        for command in command_rx {
            if write(&mut output, &command).is_err() {
                break;
            }
        }
    });
    let result = run(
        *snapshot,
        view,
        event_rx,
        command_tx,
        Some((reader, event_tx)),
    );
    let _ = writer.join();
    result
}
pub fn preview() -> Result<(), Box<dyn std::error::Error>> {
    let platform = if cfg!(windows) {
        Platform::Windows
    } else if cfg!(target_os = "macos") {
        Platform::MacOS
    } else {
        Platform::Linux
    };
    let snapshot = UiSnapshot {
        platform,
        capabilities: Capabilities {
            per_site: true,
            teencode: platform == Platform::MacOS,
            login: true,
            updates: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let (_events, receiver) = mpsc::channel();
    let (sender, _commands) = mpsc::channel();
    run(snapshot, initial_view(), receiver, sender, None)
}
fn initial_view() -> View {
    match std::env::args()
        .find_map(|a| a.strip_prefix("--sevi-ui-view=").map(str::to_owned))
        .as_deref()
    {
        Some("Emoji") => View::Emoji,
        Some("Help") => View::Help,
        Some("About") => View::About,
        Some("Credits") => View::Credits,
        Some("Excluded") => View::Excluded,
        _ => View::Settings,
    }
}
type Reader = Option<(BufReader<io::Stdin>, mpsc::Sender<UiEvent>)>;
fn run(
    snapshot: UiSnapshot,
    view: View,
    events: mpsc::Receiver<UiEvent>,
    commands: mpsc::Sender<UiCommand>,
    reader: Reader,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sevi")
            // Keep the native resource/bundle icon. None lets eframe apply its
            // egui icon later, racing our Windows WM_SETICON initialization.
            .with_icon(egui::IconData::default())
            .with_app_id(if cfg!(target_os = "linux") {
                "sevi"
            } else {
                "app.sevi.Settings"
            })
            .with_decorations(false)
            .with_inner_size(if view == View::Emoji {
                [430., 430.]
            } else {
                theme::MAIN_SIZE
            })
            .with_min_inner_size(if view == View::Emoji {
                [430., 430.]
            } else {
                theme::MAIN_SIZE
            })
            .with_active(view != View::Emoji || cfg!(target_os = "linux"))
            .with_visible(false),
        #[cfg(target_os = "macos")]
        renderer: eframe::Renderer::Wgpu,
        #[cfg(not(target_os = "macos"))]
        renderer: eframe::Renderer::Glow,
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder
                .with_activation_policy(ActivationPolicy::Accessory)
                .with_activate_ignoring_other_apps(false);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "sevi",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            if reader.is_none() {
                if std::env::args().any(|a| a == "--preview-theme=light") {
                    cc.egui_ctx.set_theme(egui::Theme::Light);
                } else if std::env::args().any(|a| a == "--preview-theme=dark") {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                }
            }
            if let Some((mut reader, sender)) = reader {
                let ctx = cc.egui_ctx.clone();
                thread::spawn(move || {
                    while let Ok(Some(event)) = read::<UiEvent>(&mut reader) {
                        if sender.send(event).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                    let _ = sender.send(UiEvent::Shutdown);
                    ctx.request_repaint();
                });
            }
            let (native_sender, native_events) = mpsc::channel();
            let picker = (view == View::Emoji).then(|| crate::emoji::Picker::new(&snapshot));
            Ok(Box::new(App {
                snapshot,
                view,
                events,
                commands,
                recording: None,
                pending_mods: 0,
                status: String::new(),
                configured: false,
                native_sender,
                native_events,
                picker,
                practice: [String::new(), String::new(), String::new()],
                screenshot_requested: false,
                update_offer: None,
                settings_size: None,
            }))
        }),
    )?;
    Ok(())
}
struct App {
    snapshot: UiSnapshot,
    view: View,
    events: mpsc::Receiver<UiEvent>,
    commands: mpsc::Sender<UiCommand>,
    recording: Option<Setting>,
    pending_mods: u32,
    status: String,
    configured: bool,
    native_sender: mpsc::Sender<PickerInput>,
    native_events: mpsc::Receiver<PickerInput>,
    picker: Option<crate::emoji::Picker>,
    practice: [String; 3],
    screenshot_requested: bool,
    update_offer: Option<(String, String)>,
    settings_size: Option<egui::Vec2>,
}
impl App {
    fn send(&self, command: UiCommand) {
        let _ = self.commands.send(command);
    }
    fn open(&mut self, view: View) {
        if self.view == View::Emoji && view != View::Emoji {
            self.send(UiCommand::PickerClosed);
        }
        if view == View::Emoji {
            self.picker = Some(crate::emoji::Picker::new(&self.snapshot));
        } else {
            self.picker = None;
        }
        self.view = view;
        self.recording = None;
        self.configured = false;
        self.send(UiCommand::Capture(false));
        self.send(UiCommand::Refresh(view));
    }
    fn picker_action(&mut self, command: UiCommand, ctx: &egui::Context) {
        if matches!(
            command,
            UiCommand::InsertEmoji { .. } | UiCommand::PickerClosed
        ) {
            crate::native::close_picker();
            self.send(command);
            self.send(UiCommand::PickerClosed);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.send(command);
        }
    }
    fn heading(ui: &mut egui::Ui, text: &str, p: Palette) {
        chrome::heading(ui, text, p);
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        use chrome::{Action, Caption};
        if let Some(action) = chrome::header(ui, self.snapshot.language_vi) {
            match action {
                Action::Language(vi) => {
                    self.snapshot.language_vi = vi;
                    self.send(UiCommand::SetLanguage(vi));
                }
                Action::Caption(Caption::Close) => {
                    self.send(UiCommand::Close);
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::Caption(Caption::Minimize) => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                Action::Caption(Caption::Maximize) => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(true)),
                Action::Caption(Caption::Restore) => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(false)),
            }
        }
    }
    fn boolean(&mut self, ui: &mut egui::Ui, key: Setting, text: &str) {
        let mut value = self.snapshot.enabled(key);
        if controls::checkbox(ui, &mut value, text).changed() {
            self.snapshot.settings.insert(key, i32::from(value));
            self.send(UiCommand::Set {
                setting: key,
                value: i32::from(value),
            });
        }
    }
    fn combo(&mut self, ui: &mut egui::Ui, key: Setting, label: &str, items: &[&str]) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let old = self.snapshot.int(key).clamp(0, items.len() as i32 - 1);
                let mut selected = old;
                let response = controls::trigger(ui, items[selected as usize]);
                controls::menu(&response, |ui| {
                    for (i, title) in items.iter().enumerate() {
                        if controls::option(ui, title, selected == i as i32).clicked() {
                            selected = i as i32;
                        }
                    }
                });
                if old != selected {
                    self.snapshot.settings.insert(key, selected);
                    self.send(UiCommand::Set {
                        setting: key,
                        value: selected,
                    });
                }
            });
        });
    }
    fn shortcut(&mut self, ui: &mut egui::Ui, key: Setting, label: &str) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let text = if self.recording == Some(key) {
                    "Press shortcut…".into()
                } else {
                    hotkey_label(self.snapshot.int(key) as u32, self.snapshot.platform)
                };
                let response = controls::trigger(ui, &text);
                controls::menu(&response, |ui| {
                    controls::option(ui, &text, true);
                    let capturing = self.recording == Some(key);
                    if controls::option(
                        ui,
                        if capturing {
                            "Cancel"
                        } else {
                            "Change shortcut"
                        },
                        false,
                    )
                    .clicked()
                    {
                        self.recording = if capturing { None } else { Some(key) };
                        self.pending_mods = 0;
                        self.send(UiCommand::Capture(self.recording.is_some()));
                    }
                });
            });
        });
    }
    fn settings(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.columns(2, |columns| {
            let left = &mut columns[0];
            Self::heading(left, "INPUT", p);
            self.shortcut(left, Setting::SwitchKeyStatus, "Switch language");
            let mut beep = self.snapshot.int(Setting::SwitchKeyStatus) as u32 & 0x8000 != 0;
            if controls::checkbox(left, &mut beep, "Play switch sound").changed() {
                let raw = self.snapshot.int(Setting::SwitchKeyStatus) ^ 0x8000;
                self.snapshot.settings.insert(Setting::SwitchKeyStatus, raw);
                self.send(UiCommand::Set {
                    setting: Setting::SwitchKeyStatus,
                    value: raw,
                });
            }
            left.add_space(5.);
            self.combo(
                left,
                Setting::TelexStyle,
                "Typing style",
                &["Telex", "Simple Telex 1", "Simple Telex 2"],
            );
            self.combo(
                left,
                Setting::ModernOrthography,
                "Tone placement",
                &["hòa, thúy", "hoà, thuý"],
            );
            self.boolean(left, Setting::Spelling, "Check Vietnamese spelling");
            left.add_enabled_ui(self.snapshot.enabled(Setting::Spelling), |ui| {
                self.boolean(ui, Setting::RestoreIfInvalidWord, "Restore invalid words")
            });
            if self.snapshot.capabilities.teencode {
                self.boolean(left, Setting::Teencode, "Recognize teencode");
            }

            let right = &mut columns[1];
            Self::heading(right, "AUTOMATIC SWITCHING", p);
            self.boolean(
                right,
                Setting::AutoSwitchByTyping,
                "Detect English / Vietnamese",
            );
            right.add_enabled_ui(self.snapshot.enabled(Setting::AutoSwitchByTyping), |ui| {
                self.boolean(
                    ui,
                    Setting::PauseAutoAfterManual,
                    "Pause 5 seconds after manual switch",
                )
            });
            self.boolean(
                right,
                Setting::UseSmartSwitchKey,
                "Remember language per application",
            );
            if self.snapshot.capabilities.per_site {
                self.boolean(
                    right,
                    Setting::SmartSwitchPerSite,
                    "Remember language per website",
                );
            }
            if self.snapshot.capabilities.per_app_auto && !self.snapshot.current_app.is_empty() {
                let mut on = self.snapshot.app_auto;
                if controls::checkbox(
                    right,
                    &mut on,
                    &format!("Auto-switch in {}", self.snapshot.current_app),
                )
                .changed()
                {
                    self.send(UiCommand::SetAppAuto(on));
                }
            }
        });
        ui.add_space(10.);
        ui.columns(2, |columns| {
            let left = &mut columns[0];
            Self::heading(left, "EMOJI", p);
            self.boolean(left, Setting::EmojiPicker, "Enable emoji picker");
            left.add_enabled_ui(self.snapshot.enabled(Setting::EmojiPicker), |ui| {
                self.shortcut(ui, Setting::EmojiHotkey, "Emoji shortcut")
            });

            let right = &mut columns[1];
            Self::heading(right, "SYSTEM", p);
            if self.snapshot.capabilities.permission
                && !self.snapshot.permission_granted
                && controls::action(right, "Enable Accessibility").clicked()
            {
                self.send(UiCommand::GrantPermission);
            }
            if self.snapshot.capabilities.login {
                let mut on = self.snapshot.login;
                if controls::checkbox(right, &mut on, "Start Sevi at login").changed() {
                    self.snapshot.login = on;
                    self.send(UiCommand::SetLogin(on));
                }
            }
            if self.snapshot.capabilities.updates {
                self.boolean(right, Setting::AutoUpdate, "Automatic updates");
            } else {
                let mut off = false;
                right.add_enabled_ui(false, |ui| {
                    controls::checkbox(ui, &mut off, "Automatic updates")
                });
            }
            if self.snapshot.capabilities.excluded_apps
                && controls::action(right, "Excluded applications…").clicked()
            {
                self.open(View::Excluded);
            }
        });
    }
    fn secondary(&mut self, ui: &mut egui::Ui, p: Palette) {
        if controls::back(ui).clicked() {
            self.open(View::Settings);
        }
        ui.add_space(10.);
        match self.view {
            View::About => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.;
                    Self::heading(ui, &format!("SEVI VERSION {}", self.snapshot.version), p);
                    Self::about_row(ui, "Smart English/Vietnamese input.");
                    ui.add_space(8.);
                    ui.columns(2, |columns| {
                        let left = &mut columns[0];
                        Self::heading(left, "OVERVIEW", p);
                        for line in [
                            "Detect English and Vietnamese.",
                            "Compose Vietnamese with Telex.",
                            "Small, native desktop input.",
                        ] {
                            Self::about_row(left, line);
                        }
                        let right = &mut columns[1];
                        Self::heading(right, "IMPLEMENTATION", p);
                        egui::Grid::new("about-details")
                            .spacing(egui::vec2(12., 8.))
                            .min_row_height(24.)
                            .show(right, |ui| {
                                for (key, value) in [
                                    ("CORE", "Rust"),
                                    ("UI", "egui / eframe"),
                                    ("DESKTOP", "Windows, Linux, macOS"),
                                ] {
                                    chrome::label(ui, key, p.muted);
                                    ui.label(value);
                                    ui.end_row();
                                }
                            });
                    });
                    if self.snapshot.capabilities.updates {
                        ui.add_space(8.);
                        if controls::action(ui, "Check for updates").clicked() {
                            self.send(UiCommand::CheckUpdates);
                        }
                    }
                    if let Some((version, notes)) = self.update_offer.clone() {
                        ui.separator();
                        ui.label(format!("Sevi {version} is available."));
                        egui::ScrollArea::vertical()
                            .id_salt("update-notes")
                            .max_height(60.)
                            .show(ui, |ui| {
                                ui.label(notes);
                            });
                        if controls::action(ui, "Download update").clicked() {
                            self.send(UiCommand::AcceptUpdate);
                            self.update_offer = None;
                            self.status = "Opening update…".into();
                        }
                    }
                });
            }
            View::Credits => {
                let credits: serde_json::Value =
                    serde_json::from_str(include_str!("../assets/credits.json"))
                        .expect("checked-in credits");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for group in credits.as_array().expect("credit groups") {
                        Self::heading(ui, group["title"].as_str().unwrap_or(""), p);
                        ui.label(
                            RichText::new(group["note"].as_str().unwrap_or(""))
                                .small()
                                .color(p.muted),
                        );
                        for row in group["items"].as_array().expect("credit rows") {
                            ui.horizontal_wrapped(|ui| {
                                let title = row[0].as_str().unwrap_or("");
                                let url = row[3].as_str().unwrap_or("");
                                if url.is_empty() {
                                    ui.label(title);
                                } else if ui.link(title).clicked() {
                                    self.send(UiCommand::OpenLink(url.into()));
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        row[1].as_str().unwrap_or(""),
                                        row[2].as_str().unwrap_or("")
                                    ))
                                    .small()
                                    .color(p.muted),
                                );
                            });
                        }
                        ui.add_space(12.);
                    }
                });
            }
            View::Help => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    Self::heading(ui, "Type Vietnamese with Telex", p);
                    for (keys, result) in [
                        ("aa · ee · oo", "â · ê · ô"),
                        ("aw · ow · uw · dd", "ă · ơ · ư · đ"),
                        ("s · f · r · x · j", "acute · grave · hook · tilde · dot"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.monospace(keys);
                            ui.label(result);
                        });
                    }
                    ui.separator();
                    ui.label("Type English normally. Sevi detects the language as you type.");
                    ui.label(
                        "Press Backspace immediately after an automatic correction to undo it.",
                    );
                    ui.label(
                        "A manual language switch can pause automatic switching for five seconds.",
                    );
                    ui.label("Emoji search accepts English, Vietnamese and Telex.");
                    ui.add_space(12.);
                    Self::heading(ui, "PRACTICE", p);
                    for (i, (keys, expected)) in [
                        ("order traf suwax nhes", "order trà sữa nhé"),
                        ("meeting lucs ba giowf nhes", "meeting lúc ba giờ nhé"),
                        ("thanks, camr own nhieeuf", "thanks, cảm ơn nhiều"),
                    ]
                    .iter()
                    .enumerate()
                    {
                        ui.label(RichText::new(*keys).monospace().size(12.));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.practice[i])
                                .hint_text("Type the Telex keys above…")
                                .desired_width(f32::INFINITY),
                        );
                        let result = practice_result(&self.practice[i]);
                        ui.label(
                            RichText::new(if result.trim() == *expected {
                                format!("Correct: {result}")
                            } else {
                                format!("Result: {result}")
                            })
                            .small()
                            .color(p.accent),
                        );
                    }
                });
            }
            View::Excluded => {
                ui.label("Sevi leaves these applications untouched. The language shortcut remains available.");
                ui.horizontal(|ui| {
                    if controls::action(ui, "Add application…").clicked() {
                        self.send(UiCommand::AddExcludedApp);
                    }
                    if controls::action(ui, "Restore defaults").clicked() {
                        self.send(UiCommand::ResetExcludedApps);
                    }
                });
                let apps = self.snapshot.apps.clone();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for app in apps {
                        let mut on = app.excluded;
                        if controls::checkbox(ui, &mut on, &app.name).changed() {
                            self.send(UiCommand::SetExcluded {
                                id: app.id,
                                excluded: on,
                            });
                        }
                    }
                });
            }
            _ => (),
        }
    }
    fn about_row(ui: &mut egui::Ui, text: &str) {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 24.),
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_height(24.);
                ui.label(text);
            },
        );
    }
}
impl App {
    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = Palette::get(&ctx);
        ui.painter().rect_filled(ui.max_rect(), 0., p.background);
        if self.view == View::Emoji {
            let mut commands = vec![];
            egui::Frame::new()
                .inner_margin(12)
                .stroke(egui::Stroke::new(1., p.line))
                .show(ui, |ui| {
                    if let Some(picker) = &mut self.picker {
                        picker.draw(
                            ui,
                            &self.snapshot,
                            &mut commands,
                            cfg!(any(windows, target_os = "macos")),
                        );
                    }
                });
            for command in commands {
                self.picker_action(command, &ctx);
            }
            return;
        }
        egui::Panel::top("sevi-header")
            .frame(egui::Frame::new().fill(p.background))
            .show_separator_line(false)
            .exact_size(theme::HEADER_H)
            .resizable(false)
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("sevi-footer")
            .frame(egui::Frame::new().fill(p.background))
            .show_separator_line(false)
            .exact_size(theme::FOOTER_H)
            .resizable(false)
            .show(ui, |ui| {
                if let Some(view) =
                    chrome::footer(ui, &self.snapshot.version, &self.status, self.view)
                {
                    self.open(view);
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.background)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 8,
                        top: 18,
                        bottom: 14,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 16.;
                if self.view == View::Settings {
                    self.settings(ui, p);
                    let height = (ui.min_rect().bottom() + 14. + theme::FOOTER_H).ceil();
                    if self
                        .settings_size
                        .is_none_or(|size| (size.y - height).abs() > 1.)
                    {
                        let size = egui::vec2(theme::MAIN_SIZE[0], height);
                        self.settings_size = Some(size);
                        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size));
                        if !ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                                ctx.content_rect().width().max(size.x),
                                size.y,
                            )));
                        }
                    }
                } else {
                    self.secondary(ui, p);
                }
            });
        chrome::window_edges(&ctx);
    }
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !cfg!(target_os = "macos") || self.view != View::Emoji {
            self.draw(ui);
        }
        // The search and local Telex exercise consume raw keyboard text. Avoid
        // attaching the system IME to our own editor or redirecting emoji there.
        ui.ctx().output_mut(|output| output.ime = None);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.configured {
            let picker = self.view == View::Emoji;
            let window = crate::native::configure(frame, picker, ctx, self.native_sender.clone());
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(if picker {
                egui::vec2(430., 430.)
            } else {
                self.settings_size
                    .unwrap_or_else(|| egui::Vec2::from(theme::MAIN_SIZE))
            }));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(if picker {
                egui::vec2(430., 430.)
            } else {
                self.settings_size
                    .unwrap_or_else(|| egui::Vec2::from(theme::MAIN_SIZE))
            }));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(
                !(cfg!(target_os = "macos") && picker),
            ));
            if picker {
                self.send(UiCommand::PickerReady(window));
            }
            self.configured = true;
        }
        while let Ok(input) = self.native_events.try_recv() {
            if let Some(picker) = &mut self.picker
                && let Some(command) = picker.input(input, &self.snapshot)
            {
                self.picker_action(command, ctx);
            }
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                UiEvent::Snapshot(snapshot) => {
                    if !snapshot.status.is_empty() {
                        self.status = snapshot.status.clone();
                    }
                    self.snapshot = *snapshot;
                }
                UiEvent::Language(vi) => self.snapshot.language_vi = vi,
                UiEvent::Open(view) => {
                    if view == View::Emoji && self.view == View::Emoji {
                        self.picker_action(UiCommand::PickerClosed, ctx);
                    } else {
                        self.open(view);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        if view != View::Emoji {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                    }
                }
                UiEvent::Error(error) => self.status = error,
                UiEvent::UpdateOffer { version, notes } => {
                    self.update_offer = Some((version, notes));
                    self.open(View::About);
                }
                UiEvent::Shutdown => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                UiEvent::Input(input) => {
                    if let Some(picker) = &mut self.picker
                        && let Some(command) = picker.input(input, &self.snapshot)
                    {
                        self.picker_action(command, ctx);
                    }
                }
            }
        }
        #[cfg(target_os = "macos")]
        if self.view == View::Emoji {
            let mut commands = vec![];
            crate::native::paint(frame, ctx, |ui| {
                let p = Palette::get(ui.ctx());
                ui.painter().rect_filled(ui.max_rect(), 0., p.background);
                egui::Frame::new().inner_margin(12).show(ui, |ui| {
                    if let Some(picker) = &mut self.picker {
                        picker.draw(ui, &self.snapshot, &mut commands, true);
                    }
                });
            });
            for command in commands {
                self.picker_action(command, ctx);
            }
        }
        screenshot(ctx, &mut self.screenshot_requested);
        if !ctx.input(|i| i.focused) && self.recording.is_some() {
            self.recording = None;
            self.send(UiCommand::Capture(false));
        }
        if let Some(setting) = self.recording {
            let mut selected = None;
            let mut cancel = false;
            ctx.input(|input| {
                let held = modifier_bits(input.modifiers);
                self.pending_mods |= held;
                if held == 0 && self.pending_mods != 0 && setting == Setting::SwitchKeyStatus {
                    selected = Some(254 | self.pending_mods);
                }
                for event in &input.events {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } = event
                    {
                        if *key == egui::Key::Escape {
                            cancel = true;
                            break;
                        }
                        let mods = modifier_bits(*modifiers);
                        if (setting == Setting::EmojiHotkey && mods & 0x700 == 0) || mods == 0 {
                            continue;
                        }
                        if self.snapshot.platform != Platform::MacOS
                            && matches!(
                                key,
                                egui::Key::F1
                                    | egui::Key::F2
                                    | egui::Key::F3
                                    | egui::Key::F4
                                    | egui::Key::F5
                                    | egui::Key::F6
                                    | egui::Key::F7
                                    | egui::Key::F8
                                    | egui::Key::F9
                                    | egui::Key::F10
                                    | egui::Key::F11
                                    | egui::Key::F12
                            )
                        {
                            continue;
                        }
                        if let Some(code) = key_code(*key) {
                            selected = Some(code | mods);
                        }
                    }
                }
            });
            if let Some(raw) = selected {
                let beep = if setting == Setting::SwitchKeyStatus {
                    self.snapshot.int(setting) as u32 & 0x8000
                } else {
                    0
                };
                self.send(UiCommand::Set {
                    setting,
                    value: (raw | beep) as i32,
                });
                cancel = true;
            }
            if cancel {
                self.recording = None;
                self.send(UiCommand::Capture(false));
            }
        }
    }
}
fn modifier_bits(mods: egui::Modifiers) -> u32 {
    (u32::from(mods.ctrl) * 0x100)
        | (u32::from(mods.alt) * 0x200)
        | (u32::from(mods.mac_cmd || (mods.command && !mods.ctrl)) * 0x400)
        | (u32::from(mods.shift) * 0x800)
}
fn key_code(key: egui::Key) -> Option<u32> {
    let name = key.name();
    if name.len() == 1 {
        let c = name.as_bytes()[0].to_ascii_lowercase();
        if c.is_ascii_alphabetic() {
            return Some(sevi_engine::keys::LETTER_KEYS[(c - b'a') as usize]);
        }
    }
    use sevi_engine::keys::*;
    if name.len() == 1 {
        let c = name.chars().next()?;
        for key in 0..=50 {
            if sevi_engine::data::key_character(key) == c as u32 {
                return Some(key);
            }
        }
    }
    Some(match key {
        egui::Key::Space => KEY_SPACE,
        egui::Key::Enter => KEY_RETURN,
        egui::Key::Tab => KEY_TAB,
        egui::Key::Backspace => KEY_DELETE,
        egui::Key::F1 => 122,
        egui::Key::F2 => 120,
        egui::Key::F3 => 99,
        egui::Key::F4 => 118,
        egui::Key::F5 => 96,
        egui::Key::F6 => 97,
        egui::Key::F7 => 98,
        egui::Key::F8 => 100,
        egui::Key::F9 => 101,
        egui::Key::F10 => 109,
        egui::Key::F11 => 103,
        egui::Key::F12 => 111,
        _ => return None,
    })
}
fn hotkey_label(raw: u32, platform: Platform) -> String {
    if raw == 0 {
        return "Set shortcut…".into();
    }
    let mut parts = vec![];
    for (bit, win, mac) in [
        (0x100, "Ctrl", "Control"),
        (0x200, "Alt", "Option"),
        (0x400, "Super", "Command"),
        (0x800, "Shift", "Shift"),
    ] {
        if raw & bit != 0 {
            parts.push(
                if platform == Platform::MacOS {
                    mac
                } else {
                    win
                }
                .to_owned(),
            );
        }
    }
    let key = raw & 255;
    if key != 254 {
        let c = sevi_engine::data::key_character(key);
        parts.push(match key {
            49 => "Space".into(),
            36 => "Enter".into(),
            48 => "Tab".into(),
            51 => "Backspace".into(),
            122 => "F1".into(),
            120 => "F2".into(),
            99 => "F3".into(),
            118 => "F4".into(),
            96 => "F5".into(),
            97 => "F6".into(),
            98 => "F7".into(),
            100 => "F8".into(),
            101 => "F9".into(),
            109 => "F10".into(),
            103 => "F11".into(),
            111 => "F12".into(),
            _ => char::from_u32(c)
                .filter(|c| *c != '\0')
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| format!("#{key}")),
        });
    }
    parts.join("+")
}
fn practice_result(raw: &str) -> String {
    use sevi_engine::{Driver, keys::*};
    let mut driver = Driver::default();
    let mut text = Vec::<u16>::new();
    for c in raw.chars() {
        let (key, shift) = if c.is_ascii_alphabetic() {
            (
                LETTER_KEYS[(c.to_ascii_lowercase() as u8 - b'a') as usize],
                c.is_ascii_uppercase(),
            )
        } else {
            match c {
                ' ' => (KEY_SPACE, false),
                ',' => (KEY_COMMA, false),
                '.' => (KEY_DOT, false),
                _ => {
                    text.extend(c.to_string().encode_utf16());
                    continue;
                }
            }
        };
        let edit = driver.press(key, shift, 0.);
        text.truncate(text.len().saturating_sub(edit.delete_count));
        text.extend(edit.text);
    }
    String::from_utf16_lossy(&text)
}
fn screenshot(ctx: &egui::Context, requested: &mut bool) {
    let Some(path) = std::env::var_os("SEVI_UI_SCREENSHOT") else {
        return;
    };
    if *requested || ctx.cumulative_frame_nr() < 1 {
        return;
    }
    *requested = true;
    ctx.request_screenshot(move |image| {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let file = std::io::BufWriter::new(std::fs::File::create(path)?);
            let mut encoder = png::Encoder::new(file, image.size[0] as u32, image.size[1] as u32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            let pixels: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            writer.write_image_data(&pixels)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Sevi screenshot: {e}");
        }
    });
}
impl Drop for App {
    fn drop(&mut self) {
        crate::native::close_picker();
        self.send(UiCommand::Capture(false));
        if self.view == View::Emoji {
            self.send(UiCommand::PickerClosed);
        }
        self.send(UiCommand::Close);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn app(platform: Platform) -> App {
        let (_, events) = mpsc::channel();
        let (commands, _) = mpsc::channel();
        let (native_sender, native_events) = mpsc::channel();
        App {
            snapshot: UiSnapshot {
                platform,
                current_app: "Microsoft Visual Studio Code".into(),
                permission_granted: false,
                capabilities: Capabilities {
                    per_site: true,
                    teencode: platform == Platform::MacOS,
                    login: true,
                    per_app_auto: true,
                    excluded_apps: true,
                    permission: true,
                    updates: true,
                },
                ..Default::default()
            },
            view: View::Settings,
            events,
            commands,
            recording: None,
            pending_mods: 0,
            status: String::new(),
            configured: false,
            native_sender,
            native_events,
            picker: None,
            practice: Default::default(),
            screenshot_requested: false,
            update_offer: None,
            settings_size: None,
        }
    }
    #[test]
    fn settings_fit_one_page_at_all_display_scales() {
        for platform in [Platform::Windows, Platform::MacOS, Platform::Linux] {
            for scale in [1., 1.25, 1.5, 2., 3.] {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                let mut app = app(platform);
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::from(theme::MAIN_SIZE),
                    )),
                    ..Default::default()
                };
                input
                    .viewports
                    .entry(egui::ViewportId::ROOT)
                    .or_default()
                    .native_pixels_per_point = Some(scale);
                for pass in 0..3 {
                    let size = app
                        .settings_size
                        .unwrap_or_else(|| egui::Vec2::from(theme::MAIN_SIZE));
                    input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
                    let mut output = ctx.run_ui(input.clone(), |ui| {
                        app.draw(ui);
                        if pass > 0 {
                            assert!(
                                ui.min_rect().right() <= size.x + 0.5,
                                "{platform:?} at {scale}: {:?}",
                                ui.min_rect()
                            );
                            assert!(
                                ui.min_rect().bottom() <= size.y + 0.5,
                                "{platform:?} at {scale}: {:?}",
                                ui.min_rect()
                            );
                        }
                    });
                    if pass > 0 {
                        let at = |label: &str| {
                            output
                                .shapes
                                .iter()
                                .find_map(|shape| match &shape.shape {
                                    egui::Shape::Text(text) if text.galley.job.text == label => {
                                        Some(text.pos)
                                    }
                                    _ => None,
                                })
                                .unwrap_or_else(|| panic!("missing {label}"))
                        };
                        assert_eq!(
                            at("EMOJI").y,
                            at("SYSTEM").y,
                            "section alignment at {scale}"
                        );
                        assert!(
                            size.y <= 550.,
                            "content should fit a compact desktop window: {size:?}"
                        );
                        assert!(at("Emoji shortcut").y < size.y - theme::FOOTER_H);
                    }
                    output.textures_delta.clear();
                }
            }
        }
    }
    #[test]
    fn about_rows_are_left_aligned_with_uniform_spacing() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.set_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            let mut app = app(Platform::Windows);
            app.view = View::About;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::from(theme::MAIN_SIZE),
                )),
                ..Default::default()
            };
            for pass in 0..2 {
                let mut output = ctx.run_ui(input.clone(), |ui| app.draw(ui));
                if pass > 0 {
                    let at = |label: &str| {
                        output
                            .shapes
                            .iter()
                            .find_map(|shape| match &shape.shape {
                                egui::Shape::Text(text) if text.galley.job.text == label => {
                                    Some(text.pos)
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| panic!("missing {label}"))
                    };
                    let rows = [
                        at("Detect English and Vietnamese."),
                        at("Compose Vietnamese with Telex."),
                        at("Small, native desktop input."),
                    ];
                    assert_eq!(at("Smart English/Vietnamese input.").x, 20.);
                    for row in rows {
                        assert_eq!(row.x, 20.);
                    }
                    assert_eq!(rows[1].y - rows[0].y, rows[2].y - rows[1].y);
                    assert_eq!(rows[0].y, at("Rust").y);
                    assert_eq!(rows[1].y, at("egui / eframe").y);
                }
                output.textures_delta.clear();
            }
        }
    }
    #[test]
    fn shortcut_labels_and_local_practice() {
        assert_eq!(hotkey_label(0x7a000206, Platform::Windows), "Alt+Z");
        assert_eq!(hotkey_label(0x9fe, Platform::MacOS), "Control+Shift");
        for (raw, result) in [
            ("order traf suwax nhes", "order trà sữa nhé"),
            ("meeting lucs ba giowf nhes", "meeting lúc ba giờ nhé"),
            ("thanks, camr own nhieeuf", "thanks, cảm ơn nhiều"),
        ] {
            assert_eq!(practice_result(raw), result);
        }
    }
}

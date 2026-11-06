//! GApplication owns one disposable UI helper. IBus keeps its existing command channel.
use gio::prelude::*;
use sevi_runtime::prefs::{JsonPreferences, Preferences};
use sevi_ui_protocol::{host::UiHost, *};
use std::{
    cell::RefCell,
    process::Command,
    rc::Rc,
    time::{Duration, Instant},
};
#[path = "updates.rs"]
mod updates;

struct State {
    prefs: JsonPreferences,
    client: UiHost,
    pending_language: Option<(bool, Instant)>,
    status: String,
    last_snapshot: String,
    held: Option<gio::ApplicationHoldGuard>,
    update_result: Option<std::sync::mpsc::Receiver<Result<Option<String>, String>>>,
}
impl State {
    fn snapshot(&mut self) -> UiSnapshot {
        let _ = self.prefs.reload();
        let mut s = sevi_runtime::ui::snapshot(&self.prefs, Platform::Linux);
        s.capabilities.updates = updates::configured();
        if let Some((vi, _)) = self.pending_language {
            s.language_vi = vi;
        }
        s.status = self.status.clone();
        s
    }
    fn open(&mut self, app: &gio::Application, view: View) {
        let snapshot = self.snapshot();
        let command = match std::env::current_exe() {
            Ok(p) => Command::new(p),
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        match self.client.open(command, view, snapshot, || {}) {
            Ok(()) => {
                if self.held.is_none() {
                    self.held = Some(app.hold());
                }
            }
            Err(e) => {
                eprintln!("Sevi UI: {e}");
                self.status = e.to_string();
            }
        }
    }
    fn request(&mut self, request: UiCommand) -> std::io::Result<()> {
        match request {
            UiCommand::Set { setting, value } => {
                sevi_runtime::ui::set(&mut self.prefs, setting, value)?
            }
            UiCommand::SetLanguage(vi) => {
                self.prefs
                    .send_command(if vi { "lang vi user" } else { "lang en user" })?;
                self.pending_language = Some((vi, Instant::now()));
            }
            UiCommand::OpenLink(url) if url.starts_with("https://") => {
                let _ = gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>);
            }
            UiCommand::CheckUpdates if self.update_result.is_none() => {
                self.status = "Checking for updates…".into();
                let (tx, rx) = std::sync::mpsc::channel();
                self.update_result = Some(rx);
                std::thread::spawn(move || {
                    let _ = tx.send(updates::check());
                });
            }
            UiCommand::AcceptUpdate if updates::configured() => {
                let url = std::env::var("SEVI_DOWNLOAD_URL").unwrap_or_default();
                gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>)
                    .map_err(std::io::Error::other)?;
            }
            UiCommand::InsertEmoji { text, base }
                if !text.contains(['\n', '\r', '\0']) && text.len() <= 256 =>
            {
                self.prefs.send_command(&format!("emoji {text}"))?;
                let mut usage = self
                    .prefs
                    .get("EmojiUsage")
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                sevi_runtime::emoji::record_usage(&mut usage, &base);
                self.prefs.set(
                    "EmojiUsage",
                    serde_json::to_value(usage).map_err(std::io::Error::other)?,
                )?;
            }
            UiCommand::Quit => self.client.close(),
            _ => (),
        }
        Ok(())
    }
    fn tick(&mut self, app: &gio::Application) {
        for request in self.client.requests() {
            if let Err(e) = self.request(request) {
                self.client.send(UiEvent::Error(e.to_string()));
            }
        }
        if let Some(rx) = &self.update_result
            && let Ok(result) = rx.try_recv()
        {
            self.update_result = None;
            match result {
                Ok(Some(version)) => {
                    self.status.clear();
                    self.client.send(UiEvent::UpdateOffer{version,notes:"Download the latest Sevi package from its configured distribution page.".into()});
                }
                Ok(None) => self.status = "Sevi is up to date.".into(),
                Err(e) => self.status = e,
            }
        }
        if let Some((vi, start)) = self.pending_language
            && (!self.prefs.command_path().exists() || start.elapsed() > Duration::from_secs(2))
        {
            // An inactive IBus service cannot acknowledge a command. Keep the
            // requested language in its existing settings for the next start.
            if self.prefs.command_path().exists() {
                let _ = self.prefs.set_int("InputMethod", i32::from(vi));
            }
            self.pending_language = None;
        }
        if self.client.connected() {
            let snapshot = self.snapshot();
            if let Ok(encoded) = serde_json::to_string(&snapshot)
                && encoded != self.last_snapshot
            {
                self.last_snapshot = encoded;
                self.client.send(UiEvent::Snapshot(Box::new(snapshot)));
            }
        } else {
            self.held = None;
            app.quit();
        }
    }
}
fn view(args: &[String]) -> View {
    if args.iter().any(|s| s == "--emoji") {
        View::Emoji
    } else if args.iter().any(|s| s == "--about") {
        View::About
    } else if args.iter().any(|s| s == "--acknowledgements") {
        View::Credits
    } else if args.iter().any(|s| s == "--guide") {
        View::Help
    } else {
        View::Settings
    }
}
pub fn run() -> glib::ExitCode {
    if sevi_ui::is_client() {
        return match sevi_ui::run_client() {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("Sevi UI: {e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--check-update") && args.iter().any(|a| a == "--quiet") {
        updates::quiet();
        return glib::ExitCode::SUCCESS;
    }
    let prefs = match JsonPreferences::linux() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Sevi: {e}");
            return glib::ExitCode::FAILURE;
        }
    };
    let state = Rc::new(RefCell::new(State {
        prefs,
        client: UiHost::default(),
        pending_language: None,
        status: String::new(),
        last_snapshot: String::new(),
        held: None,
        update_result: None,
    }));
    let app = gio::Application::builder()
        .application_id("app.sevi.Settings")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    for option in [
        "emoji",
        "about",
        "guide",
        "acknowledgements",
        "check-update",
        "quiet",
    ] {
        app.add_main_option(
            option,
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            option,
            None,
        );
    }
    app.connect_command_line({
        let state = state.clone();
        move |app, line| {
            let mut args: Vec<String> = line
                .arguments()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            // GApplication removes registered options from arguments before
            // forwarding the command. Preserve them from its options dictionary.
            for option in [
                "emoji",
                "about",
                "guide",
                "acknowledgements",
                "check-update",
                "quiet",
            ] {
                if line.options_dict().contains(option) {
                    args.push(format!("--{option}"));
                }
            }
            let mut state = state.borrow_mut();
            state.open(
                app,
                if args.iter().any(|a| a == "--check-update") {
                    View::About
                } else {
                    view(&args)
                },
            );
            if args.iter().any(|a| a == "--check-update") {
                let _ = state.request(UiCommand::CheckUpdates);
            }
            glib::ExitCode::SUCCESS
        }
    });
    glib::timeout_add_local(Duration::from_millis(200), {
        let state = state.clone();
        let app = app.clone();
        move || {
            state.borrow_mut().tick(&app);
            glib::ControlFlow::Continue
        }
    });
    app.run()
}

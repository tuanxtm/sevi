//! GTK and libadwaita desktop views, using the same settings and command files as IBus.
use adw::prelude::*;
use sevi_runtime::prefs::{JsonPreferences, Preferences};
use std::{
    cell::RefCell,
    process::{Command, Stdio},
    rc::Rc,
};
mod emoji;
mod guide;
mod live;
mod log;
mod settings;
type Prefs = Rc<RefCell<JsonPreferences>>;
fn preferences() -> Prefs {
    Rc::new(RefCell::new(
        JsonPreferences::linux().expect("read Sevi preferences"),
    ))
}
fn open_uri(uri: &str) {
    let _ = Command::new("xdg-open")
        .arg(uri)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
fn spawn(arg: &str) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).arg(arg).spawn();
    }
}
fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .build();
    for c in class.split_whitespace() {
        l.add_css_class(c)
    }
    l
}
fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}
fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}
fn margins(w: &impl IsA<gtk::Widget>, n: i32) {
    w.set_margin_start(n);
    w.set_margin_end(n);
    w.set_margin_top(n);
    w.set_margin_bottom(n);
}
fn window(
    app: &adw::Application,
    title: &str,
    width: i32,
    height: i32,
) -> (adw::ApplicationWindow, adw::ToolbarView) {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title(title)
        .default_width(width)
        .default_height(height)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    win.set_content(Some(&toolbar));
    (win, toolbar)
}
fn sources() -> i32 {
    let Some(source) = gio::SettingsSchemaSource::default() else {
        return -1;
    };
    let Some(schema) = source.lookup("org.gnome.desktop.input-sources", true) else {
        return -1;
    };
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    let rows = settings
        .value("sources")
        .get::<Vec<(String, String)>>()
        .unwrap_or_default();
    i32::from(rows.iter().any(|(kind, id)| kind == "ibus" && id == "sevi"))
}
fn hotkey(p: &Prefs) -> i32 {
    let n = p.borrow().int("SwitchKeyStatus", 0);
    if n == 0 { 0x7a000206 } else { n }
}
fn present(app: &adw::Application, dialog: &impl IsA<adw::Dialog>) {
    let guard = app.hold();
    let guard = RefCell::new(Some(guard));
    dialog.connect_closed(move |_| {
        guard.borrow_mut().take();
    });
    dialog.present(gtk::Widget::NONE);
}
fn css() {
    let p = gtk::CssProvider::new();
    p.load_from_string(include_str!("ui/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &p,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        )
    }
}
fn about(app: &adw::Application) {
    let d = adw::AboutDialog::builder()
        .application_name("Sevi")
        .application_icon("sevi")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Tony Dinh")
        .license_type(gtk::License::Gpl30)
        .comments(
            "Gõ tiếng Việt, tự chuyển Anh/Việt cho bạn.\nDựa trên OpenKey của Tuyen Mai (GPL-3.0).",
        )
        .build();
    d.add_link("Ghi nhận", "sevi:acknowledgements");
    d.connect_activate_link(|d, uri| {
        if uri == "sevi:acknowledgements" {
            credits_dialog().present(Some(d));
            true
        } else {
            false
        }
    });
    present(app, &d);
}
fn credits_dialog() -> adw::Dialog {
    let d = adw::Dialog::builder()
        .title("Ghi nhận")
        .content_width(520)
        .content_height(600)
        .build();
    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&adw::HeaderBar::new());
    let page = adw::PreferencesPage::new();
    let data: serde_json::Value =
        serde_json::from_str(include_str!("ui/credits.json")).expect("credits");
    for group in data.as_array().unwrap() {
        let g = adw::PreferencesGroup::builder()
            .title(group["title"].as_str().unwrap())
            .description(group["note"].as_str().unwrap())
            .build();
        for credit in group["items"].as_array().unwrap() {
            let url = credit[3].as_str().unwrap().to_owned();
            let row = adw::ActionRow::builder()
                .title(credit[0].as_str().unwrap())
                .subtitle(credit[1].as_str().unwrap())
                .use_markup(false)
                .activatable(!url.is_empty())
                .build();
            row.add_suffix(&label(credit[2].as_str().unwrap(), "dim-label caption"));
            if !url.is_empty() {
                row.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
                row.set_tooltip_text(Some(&url));
                row.connect_activated(move |_| open_uri(&url));
            }
            g.add(&row)
        }
        page.add(&g)
    }
    tv.set_content(Some(&page));
    d.set_child(Some(&tv));
    d
}
fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .map(|v| {
                v.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>()
    };
    let (mut a, mut b) = (parse(a), parse(b));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}
fn update(app: &adw::Application, quiet: bool) {
    let url = std::env::var("SEVI_RELEASE_URL").unwrap_or_default();
    let download_url = std::env::var("SEVI_DOWNLOAD_URL").unwrap_or_default();
    if url.trim().is_empty() || download_url.trim().is_empty() {
        if !quiet {
            let dialog = adw::AlertDialog::builder()
                .heading("Chưa cấu hình cập nhật cho Sevi")
                .body("Cài bản mới từ nơi bạn đã nhận Sevi.")
                .build();
            dialog.add_response("ok", "OK");
            present(app, &dialog);
        }
        return;
    }
    let guard = app.hold();
    let app = app.clone();
    glib::MainContext::default().spawn_local(async move {
        let result = gio::spawn_blocking(move || {
            Command::new("curl")
                .args(["-fsSL", "--max-time", "15", &url])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                .and_then(|v| v["linux"]["version"].as_str().map(str::to_owned))
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default();
        let is_new = newer(&result, env!("CARGO_PKG_VERSION"));
        if quiet {
            if is_new {
                let title = format!("Sevi {result} đã có");
                let body = format!("Bạn đang dùng bản {}.", env!("CARGO_PKG_VERSION"));
                let _ = gio::spawn_blocking(move || {
                    if Command::new("notify-send")
                        .args([
                            "-a",
                            "Sevi",
                            "-i",
                            "sevi",
                            "-A",
                            "open=Tải về",
                            &title,
                            &body,
                        ])
                        .output()
                        .is_ok_and(|o| o.stdout.starts_with(b"open"))
                    {
                        open_uri(&download_url)
                    }
                })
                .await;
            }
        } else {
            let heading = if result.is_empty() {
                "Không kiểm tra được cập nhật".into()
            } else if is_new {
                format!("Sevi {result} đã có")
            } else {
                "Đã là bản mới nhất".into()
            };
            let body = if result.is_empty() {
                "Hãy kiểm tra kết nối mạng rồi thử lại.".into()
            } else if is_new {
                format!("Bạn đang dùng bản {}.", env!("CARGO_PKG_VERSION"))
            } else {
                format!("Sevi {}.", env!("CARGO_PKG_VERSION"))
            };
            let d = adw::AlertDialog::builder()
                .heading(heading)
                .body(body)
                .build();
            if is_new {
                d.add_responses(&[("later", "Để sau"), ("download", "Tải về")]);
                d.set_response_appearance("download", adw::ResponseAppearance::Suggested);
                d.set_default_response(Some("download"));
                d.connect_response(Some("download"), move |_, _| open_uri(&download_url));
            } else {
                d.add_response("ok", "OK")
            }
            present(&app, &d)
        }
        drop(guard);
    });
}
pub fn run() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mode = args
        .iter()
        .rfind(|a| a.starts_with("--") && *a != "--quiet")
        .cloned()
        .unwrap_or_default();
    let quiet = args.iter().any(|a| a == "--quiet");
    if mode == "--emoji" {
        crate::x11::configure_environment();
    }
    let app = adw::Application::builder()
        .application_id(if mode == "--emoji" {
            "app.sevi.Emoji"
        } else {
            "app.sevi.Settings"
        })
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        css();
        let p = preferences();
        match mode.as_str() {
            "--about" => about(app),
            "--acknowledgements" => present(app, &credits_dialog()),
            "--check-update" => update(app, quiet),
            "--log" => log::build(app, p),
            "--guide" => guide::build(app, p, false),
            "--emoji" => emoji::build(app, p),
            _ => settings::build(app, p),
        }
    });
    app.run_with_args::<&str>(&[])
}
pub fn live_test() -> glib::ExitCode {
    live::run()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle() {
        let ctx = glib::MainContext::default();
        for _ in 0..30 {
            while ctx.pending() {
                ctx.iteration(false);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    fn capture(app: &adw::Application, name: &str) {
        settle();
        let win = app
            .active_window()
            .or_else(|| app.windows().first().cloned())
            .expect("native window");
        assert!(
            win.width() > 100 && win.height() > 100,
            "allocated native view"
        );
        let paintable = gtk::WidgetPaintable::new(Some(&win));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, win.width() as f64, win.height() as f64);
        let node = snapshot.to_node().expect("rendered native view");
        let renderer = gtk::gsk::CairoRenderer::new();
        renderer.realize(win.surface().as_ref()).unwrap();
        let texture = renderer.render_texture(&node, None);
        let dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/native-ui");
        std::fs::create_dir_all(&dir).unwrap();
        texture
            .save_to_png(dir.join(format!("{name}.png")))
            .unwrap();
        renderer.unrealize();
        win.destroy();
        settle();
    }
    #[test]
    #[ignore = "requires a disposable graphical session and XDG directories"]
    fn native_views() {
        adw::init().unwrap();
        let app = adw::Application::builder()
            .application_id("app.sevi.NativeViewTests")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(gio::Cancellable::NONE).unwrap();
        css();
        let p = preferences();
        p.borrow_mut().set_int("OnboardingDone", 1).unwrap();
        settings::build(&app, p.clone());
        capture(&app, "settings");
        for step in 0..5 {
            guide::build_at(&app, p.clone(), false, step);
            capture(&app, &format!("guide-{step}"));
        }
        log::build(&app, p.clone());
        capture(&app, "log");
        emoji::build(&app, p);
        capture(&app, "emoji");
    }
}

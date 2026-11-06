use super::*;
use std::{cell::Cell, path::PathBuf, time::Duration};
async fn delay(ms: u64) {
    glib::timeout_future(Duration::from_millis(ms)).await;
}
async fn xdo(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    gio::spawn_blocking(move || {
        let status = Command::new("xdotool")
            .args(args)
            .status()
            .expect("run xdotool");
        assert!(status.success(), "xdotool failed");
    })
    .await
    .expect("xdotool worker");
}
async fn language(p: &Prefs, vi: bool) -> bool {
    if let Err(e) = p
        .borrow()
        .send_command(if vi { "lang vi quiet" } else { "lang en quiet" })
    {
        eprintln!("send language: {e}");
        return false;
    }
    for _ in 0..40 {
        delay(50).await;
        if !p.borrow().command_path().exists() {
            delay(150).await;
            return true;
        }
    }
    eprintln!("IBus engine did not consume the language command");
    false
}
pub(super) fn run() -> glib::ExitCode {
    let cases: Vec<_> = std::env::args().skip(1).map(PathBuf::from).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = if cases.is_empty() {
        vec![
            root.join("Windows/Tests/cases.tsv"),
            root.join("Linux/Tests/cases-linux.tsv"),
        ]
    } else {
        cases
    };
    let result = Rc::new(Cell::new(1));
    let app = adw::Application::builder()
        .application_id("app.sevi.LiveTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate({
        let result = result.clone();
        move |app| {
            let entry = gtk::Entry::new();
            let next_field = gtk::Entry::new();
            let fields = vbox(8);
            fields.append(&entry);
            fields.append(&next_field);
            let win = gtk::ApplicationWindow::builder()
                .application(app)
                .title("SeviLiveTest")
                .default_width(600)
                .default_height(120)
                .child(&fields)
                .build();
            win.present();
            let app = app.clone();
            let cases = cases.clone();
            let result = result.clone();
            glib::MainContext::default().spawn_local(async move {
                let p = preferences();
                gio::spawn_blocking(|| {
                    let _ = Command::new("ibus").args(["engine", "sevi"]).status();
                })
                .await
                .expect("select IBus engine");
                delay(800).await;
                xdo(&["search", "--sync", "--name", "SeviLiveTest", "windowraise"]).await;
                delay(300).await;
                xdo(&[
                    "search",
                    "--sync",
                    "--name",
                    "SeviLiveTest",
                    "mousemove",
                    "--window",
                    "%1",
                    "300",
                    "60",
                    "click",
                    "1",
                ])
                .await;
                delay(500).await;
                xdo(&[
                    "search",
                    "--onlyvisible",
                    "--name",
                    "SeviLiveTest",
                    "windowactivate",
                    "--sync",
                ])
                .await;
                entry.grab_focus();
                delay(500).await;
                delay(1500).await;
                println!("live-test window active: {}", win.is_active());
                let (mut total, mut failures) = (0, 0);
                for path in cases {
                    let source = match std::fs::read_to_string(&path) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("{}: {e}", path.display());
                            failures += 1;
                            continue;
                        }
                    };
                    for line in source
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    {
                        let fields: Vec<_> = line.split('\t').collect();
                        if fields.len() < 4 {
                            eprintln!("invalid live test: {line}");
                            failures += 1;
                            continue;
                        }
                        let (name, start, keys, expected) = (
                            fields[0],
                            fields[1],
                            fields[2].replace("tô thí", "too this"),
                            fields[3],
                        );
                        total += 1;
                        entry.grab_focus();
                        xdo(&["key", "--clearmodifiers", "Escape"]).await;
                        delay(450).await;
                        entry.set_text("");
                        if !language(&p, start == "vi").await {
                            failures += 1;
                            break;
                        }
                        let mut chars = keys.chars().peekable();
                        while let Some(c) = chars.next() {
                            if c == '\\' {
                                if let Some(next) = chars.next() {
                                    let name = match next {
                                        'b' => "BackSpace".into(),
                                        'n' => "Return".into(),
                                        't' => "Tab".into(),
                                        _ => next.to_string(),
                                    };
                                    xdo(&["key", &name]).await;
                                }
                                continue;
                            }
                            if c == '{' {
                                let command: String =
                                    chars.by_ref().take_while(|c| *c != '}').collect();
                                if let Some(command) = command.strip_prefix("cmd:") {
                                    delay(700).await;
                                    let _ = p.borrow().send_command(command);
                                    delay(600).await;
                                } else if command == "wait" {
                                    delay(1500).await;
                                } else {
                                    let mapped = match command.as_str() {
                                        "num1" => "KP_1",
                                        "add" => "KP_Add",
                                        "home" => "Home",
                                        "end" => "End",
                                        "caps" => "Caps_Lock",
                                        _ => &command,
                                    };
                                    let keypad_digit = mapped.starts_with("KP_")
                                        && mapped.as_bytes().last().is_some_and(u8::is_ascii_digit);
                                    let locked = gtk::gdk::Display::default()
                                        .and_then(|d| d.default_seat())
                                        .and_then(|s| s.keyboard())
                                        .is_some_and(|d| d.is_num_locked());
                                    if keypad_digit && !locked {
                                        xdo(&["key", "Num_Lock"]).await;
                                    }
                                    if mapped == "Caps_Lock" {
                                        xdo(&["key", mapped]).await;
                                    } else {
                                        xdo(&["key", "--clearmodifiers", mapped]).await;
                                    }
                                    if keypad_digit && !locked {
                                        xdo(&["key", "Num_Lock"]).await;
                                    }
                                    delay(300).await;
                                }
                                continue;
                            }
                            if c == ' ' {
                                xdo(&["key", "space"]).await
                            } else {
                                xdo(&["type", "--delay", "25", &c.to_string()]).await
                            }
                        }
                        // GtkEntry's buffer excludes pre-edit. A real focus change
                        // commits PREEDIT_COMMIT text, including on the first field.
                        // Do not reset the IM context or type another character just
                        // to force a commit: those change the input under test.
                        next_field.grab_focus();
                        for _ in 0..100 {
                            delay(50).await;
                            if entry.text() == expected {
                                break;
                            }
                        }
                        delay(300).await;
                        let got = entry.text();
                        let ok = got == expected;
                        if !ok {
                            failures += 1
                        }
                        println!(
                            "{} {name}: got {got:?}, expected {expected:?}",
                            if ok { "ok  " } else { "FAIL" }
                        );
                        xdo(&["key", "Escape"]).await;
                    }
                }
                println!("{}/{} passed", total - failures.min(total), total);
                result.set(i32::from(failures != 0));
                app.quit();
            });
        }
    });
    app.run_with_args::<&str>(&[]);
    glib::ExitCode::from(result.get() as u8)
}

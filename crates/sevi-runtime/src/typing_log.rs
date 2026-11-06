use crate::{
    prefs::Preferences,
    session::{Modifiers, Platform},
};
use chrono::Local;
use serde_json::{Value, json};
use sevi_engine::{
    Language,
    data::key_character,
    engine::{is_number, letter},
    keys::*,
};

#[derive(Default)]
pub struct TypingLog {
    pub enabled: bool,
    pub keys_enabled: bool,
    pub secure: bool,
    pub app: String,
    pub site: String,
    pub pending: Vec<Value>,
    word: String,
    word_len: usize,
    word_lang: Language,
    truncated: bool,
    pub since_auto: bool,
    mac_stats: Option<Value>,
    mac_stats_dirty: bool,
}
impl TypingLog {
    fn count(&mut self, name: &str, prefs: &mut dyn Preferences, platform: Platform) {
        if !self.enabled {
            return;
        }
        let day = Local::now().format("%Y-%m-%d").to_string();
        if platform == Platform::Mac {
            let stats = self.mac_stats.get_or_insert_with(|| {
                prefs
                    .get("TypingLogStats")
                    .filter(Value::is_object)
                    .unwrap_or(json!({}))
            });
            let value = stats[&day][name].as_i64().unwrap_or(0) + 1;
            if !stats[&day].is_object() {
                stats[&day] = json!({})
            }
            stats[&day][name] = json!(value);
            self.mac_stats_dirty = true;
        } else {
            let key = format!("TLStat_{day}_{name}");
            let value = prefs.int(&key, 0).wrapping_add(1);
            let _ = prefs.set_int(&key, value);
        }
    }
    pub fn write(&mut self, mut value: Value) {
        if !self.enabled {
            return;
        }
        value["t"] = Value::from(Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string());
        value["app"] = Value::from(self.app.clone());
        if !self.site.is_empty() {
            value["site"] = Value::from(self.site.clone())
        }
        self.pending.push(value);
    }
    fn allowed(&self, platform: Platform) -> bool {
        if !self.keys_enabled || self.secure {
            return false;
        }
        let app = self.app.to_lowercase();
        let excluded: &[&str] = match platform {
            Platform::Linux => &[
                "1password",
                "bitwarden",
                "keepassxc",
                "keepass",
                "enpass",
                "nordpass",
                "gnome-terminal",
                "gnome-terminal-server",
                "konsole",
                "xterm",
                "alacritty",
                "kitty",
                "wezterm",
                "tilix",
                "terminator",
                "ptyxis",
                "org.gnome.ptyxis",
                "foot",
            ],
            Platform::Windows => &[
                "1password.exe",
                "bitwarden.exe",
                "keepassxc.exe",
                "keepass.exe",
                "lastpass.exe",
                "dashlane.exe",
                "enpass.exe",
                "nordpass.exe",
                "windowsterminal.exe",
                "cmd.exe",
                "powershell.exe",
                "pwsh.exe",
                "putty.exe",
                "wezterm-gui.exe",
                "alacritty.exe",
                "conhost.exe",
                "openconsole.exe",
                "termius.exe",
            ],
            Platform::Mac => &[
                "com.1password.1password",
                "com.agilebits.onepassword7",
                "com.bitwarden.desktop",
                "com.apple.passwords",
                "com.apple.keychainaccess",
                "com.lastpass.lastpass",
                "com.dashlane.dashlanephonefinal",
                "in.sinew.enpass-desktop",
                "com.keepassxc.keepassxc",
                "com.sinew.enpass-desktop",
                "com.nordpass.macos.nordpass",
                "com.apple.terminal",
                "com.googlecode.iterm2",
                "dev.warp.warp-stable",
                "com.mitchellh.ghostty",
                "io.alacritty",
                "net.kovidgoyal.kitty",
                "com.github.wez.wezterm",
                "co.zeit.hyper",
                "com.termius-dmg.mac",
            ],
        };
        !excluded.contains(&app.as_str())
    }
    pub fn end_word(&mut self, reason: &str, prefs: &mut dyn Preferences, platform: Platform) {
        if self.word_len == 0 {
            return;
        }
        let has = self.word.chars().any(|c| c != '⌫');
        if has {
            self.count(
                if self.word_lang == Language::Vietnamese {
                    "words_vi"
                } else {
                    "words_en"
                },
                prefs,
                platform,
            )
        }
        if has && self.allowed(platform) {
            let mut value =
                json!({"e":"word","lang":lang(self.word_lang),"keys":self.word,"end":reason});
            if self.truncated {
                value["truncated"] = Value::Bool(true)
            }
            self.write(value)
        }
        self.word.clear();
        self.word_len = 0;
        self.truncated = false;
    }
    fn append(&mut self, c: char, language: Language) {
        if self.word_len == 0 {
            self.word_lang = language
        }
        if self.word_len < 64 {
            self.word.push(c);
            self.word_len += 1
        } else {
            self.truncated = true
        }
    }
    pub fn key(
        &mut self,
        key: u32,
        flags: Modifiers,
        language: Language,
        prefs: &mut dyn Preferences,
        platform: Platform,
    ) {
        if !self.enabled {
            return;
        }
        self.since_auto = false;
        let shift = flags.contains(Modifiers::SHIFT);
        if flags.control() {
            let mut combo = String::new();
            if flags.contains(Modifiers::CONTROL) {
                combo.push_str(if platform == Platform::Mac {
                    "⌃"
                } else {
                    "Ctrl+"
                })
            }
            if flags.contains(Modifiers::ALT) {
                combo.push_str(if platform == Platform::Mac {
                    "⌥"
                } else {
                    "Alt+"
                })
            }
            if shift {
                combo.push_str(if platform == Platform::Mac {
                    "⇧"
                } else {
                    "Shift+"
                })
            }
            if flags.contains(Modifiers::SUPER) {
                combo.push_str(match platform {
                    Platform::Mac => "⌘",
                    Platform::Windows => "Win+",
                    Platform::Linux => "Super+",
                })
            }
            combo.push_str(
                &if key == KEY_DELETE {
                    "Backspace".into()
                } else {
                    key_name(key, false)
                }
                .to_uppercase(),
            );
            self.end_word(&combo, prefs, platform);
            return;
        }
        if key == KEY_DELETE {
            self.append('⌫', language);
            return;
        }
        if letter(key).is_some() || (is_number(key) && !shift) {
            let ch = key_character(
                key | if shift || flags.contains(Modifiers::CAPS) {
                    CAPS_MASK
                } else {
                    0
                },
            );
            if let Some(c) = char::from_u32(ch).filter(|_| ch != 0) {
                self.append(c, language)
            }
            return;
        }
        self.end_word(&key_name(key, shift), prefs, platform);
    }
    pub fn auto_switch(
        &mut self,
        rule: &str,
        to: Language,
        prefs: &mut dyn Preferences,
        platform: Platform,
    ) {
        self.count(&format!("auto_{rule}"), prefs, platform);
        self.write(json!({"e":"auto","rule":rule,"to":lang(to)}));
        self.since_auto = true;
    }
    pub fn manual(
        &mut self,
        via: &str,
        to: Language,
        prefs: &mut dyn Preferences,
        platform: Platform,
    ) {
        self.end_word("switch", prefs, platform);
        let after = self.since_auto;
        self.since_auto = false;
        self.count("manual", prefs, platform);
        let mut event = json!({"e":"manual","via":via,"to":lang(to)});
        if after {
            self.count("manual_after_auto", prefs, platform);
            event["after_auto"] = Value::Bool(true)
        }
        self.write(event);
    }
    pub fn undo(&mut self, to: Language, prefs: &mut dyn Preferences, platform: Platform) {
        self.end_word("undo", prefs, platform);
        self.since_auto = false;
        self.count("undo", prefs, platform);
        self.write(json!({"e":"undo","via":"backspace","to":lang(to)}));
    }
    pub fn context(&mut self, to: Language, prefs: &mut dyn Preferences, platform: Platform) {
        self.end_word("context", prefs, platform);
        self.count("context", prefs, platform);
        self.write(json!({"e":"context","to":lang(to)}));
    }
    pub fn take(&mut self, prefs: &mut dyn Preferences) -> String {
        if self.mac_stats_dirty
            && let Some(stats) = &self.mac_stats
            && prefs.set("TypingLogStats", stats.clone()).is_ok()
        {
            self.mac_stats_dirty = false;
        }
        let mut out = String::new();
        for event in self.pending.drain(..) {
            if let Ok(line) = serde_json::to_string(&event) {
                out.push_str(&line);
                out.push('\n')
            }
        }
        out
    }
    pub fn stats_snapshot(&self, prefs: &dyn Preferences) -> Value {
        self.mac_stats
            .clone()
            .or_else(|| prefs.get("TypingLogStats"))
            .unwrap_or(json!({}))
    }
    pub fn clear(&mut self, prefs: &mut dyn Preferences, platform: Platform) {
        self.pending.clear();
        self.word.clear();
        self.word_len = 0;
        self.truncated = false;
        self.since_auto = false;
        if platform == Platform::Mac {
            self.mac_stats = Some(json!({}));
            self.mac_stats_dirty = prefs.remove("TypingLogStats").is_err();
        } else {
            let _ = prefs.clear_stats();
        }
    }
}
pub fn lang(language: Language) -> &'static str {
    if language == Language::Vietnamese {
        "vi"
    } else {
        "en"
    }
}
/// Clear the visible log files while retaining a recoverable copy.
pub fn archive_logs(directory: &std::path::Path) -> std::io::Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for name in ["typing-log.jsonl", "typing-log.1.jsonl"] {
        let path = directory.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => std::fs::rename(
                &path,
                directory.join(format!("{name}.cleared-{}-{stamp}", std::process::id())),
            )?,
            Ok(_) => return Err(std::io::Error::other("log path is not a regular file")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
/// Read a bounded tail, retaining a complete record exactly at the byte boundary.
pub fn read_log_tail(
    path: &std::path::Path,
    max_bytes: usize,
    max_events: usize,
) -> std::io::Result<Vec<Value>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(max_bytes as u64);
    let probe = start.saturating_sub(1);
    file.seek(SeekFrom::Start(probe))?;
    let mut bytes = Vec::new();
    file.take(size - probe).read_to_end(&mut bytes)?;
    let data = if start == 0 {
        &bytes[..]
    } else if let Some(newline) = bytes.iter().position(|b| *b == b'\n') {
        &bytes[newline + 1..]
    } else {
        return Ok(Vec::new());
    };
    Ok(data
        .split(|b| *b == b'\n')
        .rev()
        .filter_map(|line| serde_json::from_slice(line).ok())
        .take(max_events)
        .collect())
}
pub fn key_name(key: u32, shift: bool) -> String {
    match key {
        KEY_SPACE => "space".into(),
        KEY_RETURN | KEY_ENTER => "enter".into(),
        KEY_TAB => "tab".into(),
        KEY_ESC => "esc".into(),
        KEY_LEFT | KEY_RIGHT | KEY_UP | KEY_DOWN => "arrow".into(),
        _ => char::from_u32(key_character(key | if shift { CAPS_MASK } else { 0 }))
            .filter(|c| *c != '\0')
            .map(|c| c.to_string())
            .unwrap_or_else(|| "other".into()),
    }
}

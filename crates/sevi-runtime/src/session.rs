use crate::{prefs::Preferences, typing_log::TypingLog};
use serde_json::json;
use sevi_engine::{
    data::key_character,
    engine::{is_number, letter},
    keys::*,
    smart_switch::SmartSwitch,
    *,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(pub u32);
impl Modifiers {
    pub const SHIFT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const SUPER: Self = Self(8);
    pub const CAPS: Self = Self(16);
    /// macOS Fn, numeric-pad and help flags on keys other than a plain arrow.
    pub const OTHER: Self = Self(32);
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    pub fn control(self) -> bool {
        self.0 & (14 | Self::OTHER.0) != 0
    }
    pub fn shift(self) -> bool {
        self.contains(Self::SHIFT)
    }
    pub fn caps(self) -> bool {
        self.0 & 17 != 0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// macOS output policy; the host supplies Spotlight visibility and app workarounds.
    MacRewrite {
        autocomplete: bool,
        selection: bool,
    },
    Backspaces(usize),
    Text(String),
    ControlKey {
        key: u32,
        shift: bool,
    },
    LanguageChanged(Language),
    SwitchSound(Language),
    OpenEmoji,
}
#[derive(Default, Clone, Debug, PartialEq)]
pub struct EventResult {
    pub consumed: bool,
    pub effects: Vec<Effect>,
}
#[derive(Clone)]
struct Undo {
    remove: usize,
    text: String,
    language: Language,
    keys: Vec<u32>,
}
#[derive(Clone)]
struct Navigation {
    keys: Vec<u32>,
    valid: bool,
    after_double: bool,
    previous: Vec<Vec<u32>>,
}
pub struct DesktopSession {
    pub engine: Engine,
    pub preferences: Box<dyn Preferences>,
    pub platform: Platform,
    pub log: TypingLog,
    pub front_app: String,
    pub site: String,
    pub excluded: bool,
    pub other_language: bool,
    /// Windows host delivery failed; bypass rewrites until the foreground changes.
    pub insertion_blocked: bool,
    pub emoji_open: bool,
    pub hotkey: u32,
    pub emoji_hotkey: u32,
    smart: SmartSwitch,
    smart_dirty: bool,
    smart_enabled: bool,
    auto_pause: Option<f64>,
    flags: Modifiers,
    last_flags: Modifiers,
    modifier_chord_used: bool,
    key: u32,
    en: Vec<u32>,
    valid: bool,
    after_shortcut: bool,
    after_double: bool,
    previous: Vec<Vec<u32>>,
    previous_restored: bool,
    undone: Vec<u32>,
    undo: Option<Undo>,
    nav: Option<Navigation>,
    nav_offset: i32,
}
fn whitespace(k: u32) -> bool {
    [KEY_SPACE, KEY_RETURN, KEY_ENTER, KEY_TAB].contains(&k)
}
fn word_start(k: u32, shift: bool) -> bool {
    whitespace(k) || (k == KEY_9 && shift) || k == KEY_LEFT_BRACKET || k == KEY_QUOTE
}
fn text_of(keys: &[u32]) -> String {
    keys.iter()
        .filter_map(|k| char::from_u32(rendered_char(*k)))
        .collect()
}
/// Ignore the beep bit and legacy high bits when comparing keyboard chords.
pub fn same_hotkey_keys(a: u32, b: u32) -> bool {
    a & 0xfff == b & 0xfff
}
pub fn valid_mac_emoji_hotkey(emoji: u32, switch: u32) -> bool {
    emoji & 0x700 != 0 && !same_hotkey_keys(emoji, switch)
}
impl DesktopSession {
    pub fn new(platform: Platform, mut preferences: Box<dyn Preferences>) -> Self {
        if platform == Platform::Mac {
            let emoji = preferences.int("EmojiHotkey", 0) as u32;
            let switch = preferences.int("SwitchKeyStatus", 0) as u32;
            let switch = if switch == 0 { 0x7a000206 } else { switch };
            if emoji != 0 && !valid_mac_emoji_hotkey(emoji, switch) {
                let _ = preferences.set_int("EmojiHotkey", 0);
            }
        }
        let smart = SmartSwitch::load(&preferences.binary("smartSwitchKey"));
        let mut s = Self {
            engine: Engine::default(),
            preferences,
            platform,
            log: TypingLog::default(),
            front_app: "UnknownApp".into(),
            site: String::new(),
            excluded: false,
            other_language: false,
            insertion_blocked: false,
            emoji_open: false,
            hotkey: 0x7a000206,
            emoji_hotkey: 0,
            smart,
            smart_dirty: false,
            smart_enabled: true,
            auto_pause: None,
            flags: Modifiers(0),
            last_flags: Modifiers(0),
            modifier_chord_used: false,
            key: 0,
            en: vec![],
            valid: true,
            after_shortcut: false,
            after_double: false,
            previous: vec![],
            previous_restored: false,
            undone: vec![],
            undo: None,
            nav: None,
            nav_offset: 0,
        };
        s.reload_settings(0.0);
        s.log.app = s.front_app.clone();
        s
    }
    pub fn language(&self) -> Language {
        self.engine.config.language
    }
    pub fn auto_for_app(&self) -> bool {
        let global = self.preferences.int("AutoSwitchByTyping", 1) != 0;
        if self.platform == Platform::Mac {
            self.preferences
                .get("AutoSwitchPerApp")
                .and_then(|v| v.get(&self.front_app).cloned())
                .and_then(|v| v.as_i64().map(|n| n != 0).or_else(|| v.as_bool()))
                .unwrap_or(global)
        } else {
            global
        }
    }
    pub fn set_auto_for_app(&mut self, on: bool, now: f64) {
        if self.platform != Platform::Mac {
            return;
        }
        let global = self.preferences.int("AutoSwitchByTyping", 1) != 0;
        let mut map = self
            .preferences
            .get("AutoSwitchPerApp")
            .filter(|v| v.is_object())
            .unwrap_or(json!({}));
        if on == global {
            map.as_object_mut().unwrap().remove(&self.front_app);
        } else {
            map[&self.front_app] = json!(i32::from(on));
        }
        let _ = self.preferences.set("AutoSwitchPerApp", map);
        if !self.paused(now) {
            self.engine.config.auto_switch = self.auto_for_app()
        }
        self.start_english(false);
    }
    pub fn paused(&self, now: f64) -> bool {
        self.auto_pause.is_some_and(|until| now < until)
    }
    pub fn tick(&mut self, now: f64) {
        if self.auto_pause.is_some() && !self.paused(now) {
            self.auto_pause = None;
            self.engine.config.auto_switch = self.auto_for_app();
            self.start_english(false)
        }
    }
    pub fn reload_settings(&mut self, now: f64) -> EventResult {
        let _ = self.preferences.reload();
        self.apply_cached_settings(now)
    }
    /// Apply an acknowledged UI change already present in the host's memory cache.
    pub fn apply_cached_settings(&mut self, now: f64) -> EventResult {
        let language = if self.preferences.int("InputMethod", 1) != 0 {
            Language::Vietnamese
        } else {
            Language::English
        };
        if self.platform == Platform::Linux && language != self.language() {
            self.undo = None;
            self.reset_english(true);
            self.engine.new_session()
        }
        self.engine.config.language = language;
        self.engine.config.spelling = self.preferences.int("Spelling", 1) != 0;
        self.engine.set_check_spelling();
        self.engine.config.restore_if_invalid =
            self.preferences.int("RestoreIfInvalidWord", 1) != 0;
        self.engine.config.modern_orthography = self.preferences.int("ModernOrthography", 1) != 0;
        self.engine.config.telex_style = match self.preferences.int("TelexStyle", 0) {
            0 => TelexStyle::Telex,
            1 => TelexStyle::Simple1,
            _ => TelexStyle::Simple2,
        };
        if self.platform == Platform::Mac {
            self.engine.config.teencode = self.preferences.int("Teencode", 1) != 0
        }
        self.smart_enabled = self.preferences.int("UseSmartSwitchKey", 1) != 0;
        self.engine.config.auto_switch = !self.paused(now) && self.auto_for_app();
        // History was removed; legacy preferences must never restart recording.
        self.log.enabled = false;
        let hk = self.preferences.int("SwitchKeyStatus", 0) as u32;
        self.hotkey = if hk == 0 { 0x7a000206 } else { hk };
        self.emoji_hotkey = if self.preferences.int("EmojiPicker", 0) != 0 {
            self.preferences.int("EmojiHotkey", 0) as u32
        } else {
            0
        };
        self.excluded = self.platform == Platform::Mac
            && self
                .preferences
                .get("ExcludedApps")
                .and_then(|v| v.as_array().cloned())
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&self.front_app)));
        EventResult {
            consumed: false,
            effects: vec![Effect::LanguageChanged(language)],
        }
    }
    fn start_english(&mut self, valid: bool) {
        self.en.clear();
        self.valid = valid;
        self.after_shortcut = false
    }
    fn reset_english(&mut self, valid: bool) {
        self.after_double = false;
        self.undone.clear();
        self.start_english(valid);
        self.previous.clear();
        self.nav = None;
        self.nav_offset = 0;
        self.engine.forget()
    }
    pub fn reset_typing(&mut self) {
        self.reset_english(true);
        self.undo = None;
        self.engine.new_session();
    }
    /// The document no longer matches our predictions. Only the Windows host calls this.
    pub fn discard_failed_output(&mut self) {
        if self.platform != Platform::Windows {
            return;
        }
        // Recreate transient engine state, including navigation and pending conversions.
        // Desktop Windows uses the immutable built-in dictionaries and existing config.
        self.engine = Engine::new(self.engine.config.clone());
        self.reset_typing();
        self.nav = None;
        self.nav_offset = 0;
        self.previous_restored = false;
        self.insertion_blocked = true;
    }
    fn context_key(&self) -> String {
        if self.site.is_empty() {
            self.front_app.clone()
        } else {
            format!("{}|{}", self.front_app, self.site)
        }
    }
    fn save_smart(&mut self) {
        self.smart_dirty = true;
        if self.platform != Platform::Mac {
            let _ = self.flush_smart_preferences();
        }
    }
    pub fn smart_preferences_dirty(&self) -> bool {
        self.smart_dirty
    }
    /// macOS schedules this after the event tap returns; other hosts save immediately.
    pub fn flush_smart_preferences(&mut self) -> std::io::Result<()> {
        if self.smart_dirty {
            self.preferences
                .set_binary("smartSwitchKey", &self.smart.save())?;
            self.smart_dirty = false;
        }
        Ok(())
    }
    fn changed(&mut self, remember: bool, out: &mut EventResult) {
        if remember && self.smart_enabled && !self.excluded {
            self.smart.set(&self.context_key(), self.language());
            self.save_smart()
        }
        let _ = self
            .preferences
            .set_int("InputMethod", self.language() as i32);
        out.effects.push(Effect::LanguageChanged(self.language()));
    }
    fn apply_smart(&mut self, out: &mut EventResult) {
        if !self.smart_enabled || self.excluded {
            return;
        }
        if let Some(language) = self.smart.lookup(&self.context_key(), self.language()) {
            if language != self.language() {
                self.engine.config.language = language;
                self.changed(false, out);
                self.log
                    .context(language, &mut *self.preferences, self.platform);
                self.engine.new_session()
            }
        } else {
            self.save_smart()
        }
    }
    pub fn focus_changed(&mut self, app: &str, now: f64) -> EventResult {
        if !app.is_empty() {
            self.front_app = app.into()
        }
        self.site.clear();
        self.undo = None;
        self.reset_english(true);
        self.log
            .end_word("app", &mut *self.preferences, self.platform);
        self.log.app = self.front_app.clone();
        self.log.site.clear();
        self.excluded = self.platform == Platform::Mac
            && self
                .preferences
                .get("ExcludedApps")
                .and_then(|v| v.as_array().cloned())
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&self.front_app)));
        if !self.paused(now) {
            self.engine.config.auto_switch = self.auto_for_app()
        }
        let mut out = EventResult::default();
        self.apply_smart(&mut out);
        self.engine.new_session();
        out
    }
    pub fn site_changed(&mut self, host: &str) -> EventResult {
        if self.platform == Platform::Linux
            || self.preferences.int(
                "SmartSwitchPerSite",
                i32::from(self.platform == Platform::Windows),
            ) == 0
            || self.site == host
        {
            return EventResult::default();
        }
        self.site = host.into();
        self.log.site = self.site.clone();
        self.undo = None;
        self.reset_english(true);
        let mut out = EventResult::default();
        self.apply_smart(&mut out);
        self.engine.new_session();
        out
    }
    fn pause(&mut self, now: f64) {
        if self.preferences.int("PauseAutoAfterManual", 1) != 0 {
            self.auto_pause = Some(now + 5.0);
            self.engine.config.auto_switch = false
        }
    }
    pub fn set_language_user(
        &mut self,
        language: Language,
        now: f64,
        via: &str,
        sound: bool,
    ) -> EventResult {
        if self.language() == language {
            return EventResult::default();
        }
        self.pause(now);
        self.undo = None;
        self.reset_english(true);
        self.engine.config.language = language;
        self.log
            .manual(via, language, &mut *self.preferences, self.platform);
        let mut out = EventResult::default();
        if sound && self.hotkey & 0x8000 != 0 {
            out.effects.push(Effect::SwitchSound(language))
        }
        self.changed(true, &mut out);
        self.engine.new_session();
        out
    }
    pub fn set_language_quiet(&mut self, language: Language) -> EventResult {
        self.undo = None;
        self.reset_english(true);
        self.auto_pause = None;
        self.engine.config.auto_switch = self.auto_for_app();
        self.engine.config.language = language;
        let mut out = EventResult::default();
        self.changed(false, &mut out);
        self.engine.new_session();
        out
    }
    fn hotkey_matches(&self, hotkey: u32, flags: Modifiers, key: bool) -> bool {
        if hotkey & !0x8000 == 0xfe0000fe {
            return false;
        }
        let required = ((hotkey & 0x100) >> 7)
            | ((hotkey & 0x200) >> 7)
            | ((hotkey & 0x400) >> 7)
            | ((hotkey & 0x800) >> 11);
        flags.0 & 15 == required && (!key || hotkey & 0xff == self.key)
    }
    pub fn modifier(&mut self, bit: Modifiers, down: bool, now: f64) -> EventResult {
        self.tick(now);
        if down {
            self.flags.0 |= bit.0
        } else {
            self.flags.0 &= !bit.0
        }
        if bit == Modifiers::CAPS {
            return EventResult::default();
        }
        if self.platform == Platform::Mac && self.modifier_chord_used {
            self.last_flags = Modifiers(0);
            if self.flags.0 & 15 == 0 {
                self.modifier_chord_used = false;
            }
            return EventResult::default();
        }
        if self.last_flags.0 == 0 || self.last_flags.0 < self.flags.0 {
            self.last_flags = self.flags
        } else if self.last_flags.0 > self.flags.0 {
            let hit = self.hotkey & 0xff == 0xfe
                && self.hotkey_matches(self.hotkey, self.last_flags, false);
            self.last_flags = Modifiers(0);
            if hit {
                let mut result =
                    self.set_language_user(self.language().toggled(), now, "hotkey", true);
                result.consumed = true;
                return result;
            }
        }
        EventResult::default()
    }
    /// Refresh the host's current modifier state (including changes while locked).
    pub fn synchronize_modifiers(&mut self, flags: Modifiers) {
        self.flags = flags;
    }
    fn restored_character(&self) -> u32 {
        // Caps Lock changes letters, not punctuation or Space.
        let shifted = self.flags.shift()
            || (self.flags.contains(Modifiers::CAPS) && letter(self.key).is_some());
        key_character(self.key | if shifted { CAPS_MASK } else { 0 })
    }
    fn send_output(&mut self, out: &mut EventResult) {
        let h = &self.engine.hook;
        if self.platform == Platform::Mac {
            out.effects.push(Effect::MacRewrite {
                autocomplete: h.ext_code != 4 && h.backspaces < MAX_BUFF - 1,
                selection: true,
            });
        }
        if self.platform == Platform::Windows && h.ext_code != 4 && h.backspaces < MAX_BUFF - 1 {
            out.effects.push(Effect::Text("\u{202f}".into()));
            out.effects.push(Effect::Backspaces(1));
        }
        if h.backspaces > 0 && h.backspaces < MAX_BUFF {
            out.effects.push(Effect::Backspaces(h.backspaces))
        }
        let mut text = text_of(
            &h.chars[..h.new_chars]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>(),
        );
        let mut control = false;
        if matches!(h.code, HookCode::Restore | HookCode::RestoreAndReset) {
            let c = self.restored_character();
            if let Some(c) = char::from_u32(c).filter(|c| *c != '\0') {
                text.push(c)
            } else {
                control = true
            }
        }
        if h.code == HookCode::RestoreAndReset {
            self.engine.new_session()
        }
        if !text.is_empty() {
            out.effects.push(Effect::Text(text))
        }
        if control {
            out.effects.push(Effect::ControlKey {
                key: self.key,
                shift: self.flags.shift(),
            })
        }
    }
    fn remember_fix(&mut self, corrected: usize) {
        self.undo = None;
        if self.engine.undo_data.is_empty() || corrected == 0 {
            return;
        }
        if self
            .engine
            .undo_data
            .iter()
            .any(|c| c & CHAR_MASK == KEY_SPACE && c & CHAR_CODE_MASK == 0)
        {
            return;
        }
        let brk = self.restored_character();
        if brk == 0 {
            return;
        }
        let text = text_of(&self.engine.undo_data);
        if text.contains('\0') {
            return;
        }
        let keys = if self
            .engine
            .undo_data
            .iter()
            .any(|c| c & CHAR_CODE_MASK != 0)
        {
            vec![]
        } else {
            self.engine.undo_data.clone()
        };
        self.undo = Some(Undo {
            remove: corrected + 1,
            text,
            language: self.language(),
            keys,
        });
    }
    fn auto_switch(&mut self, to: Language, rule: &str, out: &mut EventResult) {
        if self.language() == to {
            return;
        }
        self.log
            .auto_switch(rule, to, &mut *self.preferences, self.platform);
        self.engine.config.language = to;
        self.changed(true, out);
        self.engine.new_session();
        self.engine.forget();
    }
    fn undo_fix(&mut self, undo: Undo) -> EventResult {
        let mut out = EventResult {
            consumed: true,
            effects: vec![Effect::Backspaces(undo.remove), Effect::Text(undo.text)],
        };
        if self.platform == Platform::Mac {
            out.effects.insert(
                0,
                Effect::MacRewrite {
                    autocomplete: true,
                    selection: false,
                },
            );
        }
        self.log
            .undo(undo.language, &mut *self.preferences, self.platform);
        if self.language() != undo.language {
            self.engine.config.language = undo.language;
            self.changed(true, &mut out)
        }
        self.engine.new_session();
        self.reset_english(false);
        if self.language() == Language::English && !undo.keys.is_empty() {
            self.en = undo.keys.clone();
            self.valid = true;
            self.undone = undo.keys
        }
        out
    }
    fn english_key(&mut self, out: &mut EventResult) -> bool {
        if self.flags.control() {
            self.reset_english(true);
            self.after_shortcut = self.flags.0 & (Modifiers::CONTROL.0 | Modifiers::SUPER.0) != 0;
            return false;
        }
        let (k, shift, caps) = (self.key, self.flags.shift(), self.flags.caps());
        if (k == KEY_LEFT || k == KEY_RIGHT) && !shift {
            if k == KEY_LEFT
                && self.nav_offset == 0
                && (!self.en.is_empty() || !self.previous.is_empty())
            {
                self.nav = Some(Navigation {
                    keys: self.en.clone(),
                    valid: self.valid,
                    after_double: self.after_double,
                    previous: self.previous.clone(),
                });
                self.nav_offset = -1;
                self.start_english(false);
                self.after_double = false;
                self.previous.clear();
                return false;
            }
            if self.nav_offset < 0 {
                if k == KEY_LEFT {
                    self.nav_offset -= 1
                } else {
                    self.nav_offset += 1;
                    if self.nav_offset == 0
                        && let Some(nav) = self.nav.take()
                    {
                        self.en = nav.keys;
                        self.valid = nav.valid;
                        self.after_double = nav.after_double;
                        self.previous = nav.previous;
                    }
                }
                return false;
            }
        }
        self.nav_offset = 0;
        if letter(k).is_some() {
            if self.en.len() < MAX_BUFF - 2 {
                self.en.push(k | if caps { CAPS_MASK } else { 0 })
            } else {
                self.valid = false
            }
            self.after_shortcut = false;
            return false;
        }
        if k == KEY_DELETE {
            if self.en.is_empty() {
                self.previous.clear();
                self.after_double = false
            }
            if self.en.pop().is_none() && !self.after_shortcut {
                self.valid = false
            }
            return false;
        }
        if is_number(k) && !shift {
            self.valid = self.en.is_empty() && self.valid;
            self.en.clear();
            self.previous.clear();
            self.after_double = false;
            return false;
        }
        let word_end = [
            KEY_SPACE,
            KEY_COMMA,
            KEY_DOT,
            KEY_SEMICOLON,
            KEY_RETURN,
            KEY_ENTER,
        ]
        .contains(&k)
            || (shift && [KEY_SLASH, KEY_1, KEY_QUOTE, KEY_0].contains(&k));
        let en = std::mem::take(&mut self.en);
        let valid = self.valid;
        let double = self.after_double;
        self.after_double = false;
        if double && word_end && self.engine.fix_doubled_key(&en) {
            self.start_english(word_start(k, shift));
            self.previous.clear();
            let corrected = self.engine.hook.new_chars;
            self.send_output(out);
            self.remember_fix(corrected);
            return true;
        }
        self.start_english(word_start(k, shift));
        let mut checked = word_end && valid && en.len() >= 2 && !self.other_language;
        if !self.undone.is_empty() {
            if en == self.undone {
                checked = false
            }
            if word_end {
                self.undone.clear()
            }
        }
        self.engine.previous_restored = self.previous_restored && !self.previous.is_empty();
        let converted = checked && self.engine.check_english_word(&en, &self.previous);
        if !converted && k == KEY_SPACE && valid && !en.is_empty() {
            self.previous_restored = false;
            self.previous.insert(0, en);
            self.previous.truncate(2)
        } else {
            self.previous.clear()
        }
        if !converted {
            return false;
        }
        let corrected = self.engine.hook.new_chars;
        self.send_output(out);
        self.remember_fix(corrected);
        self.auto_switch(Language::Vietnamese, "en_word_to_vi", out);
        true
    }
    pub fn key_down(&mut self, key: u32, shift: bool, flags: Modifiers, now: f64) -> EventResult {
        self.key_down_event(key, shift, flags, false, now)
    }
    pub fn key_down_event(
        &mut self,
        key: u32,
        shift: bool,
        flags: Modifiers,
        repeat: bool,
        now: f64,
    ) -> EventResult {
        self.tick(now);
        self.flags = Modifiers((flags.0 & !1) | u32::from(shift));
        self.key = key;
        if self.emoji_hotkey != 0
            && !self.excluded
            && self.hotkey_matches(self.emoji_hotkey, self.flags, true)
        {
            self.cancel_modifier_chord();
            return EventResult {
                consumed: true,
                effects: if self.platform == Platform::Mac && repeat {
                    vec![]
                } else {
                    vec![Effect::OpenEmoji]
                },
            };
        }
        if self.hotkey & 0xff != key {
            self.cancel_modifier_chord();
        } else if self.hotkey_matches(
            self.hotkey,
            if self.platform == Platform::Mac {
                self.flags
            } else {
                self.last_flags
            },
            true,
        ) {
            if self.platform == Platform::Mac && repeat {
                return EventResult {
                    consumed: true,
                    effects: vec![],
                };
            }
            let mut out = self.set_language_user(self.language().toggled(), now, "hotkey", true);
            self.last_flags = Modifiers(0);
            out.consumed = true;
            return out;
        }
        if self.excluded
            || self.emoji_open
            || (self.other_language && self.platform == Platform::Linux)
        {
            return EventResult::default();
        }
        // Windows' UI Automation password/unknown-focus guard is checked after
        // hotkeys, before undo, logging, and any text transformation.
        if self.platform == Platform::Windows && (self.log.secure || self.insertion_blocked) {
            self.reset_typing();
            return EventResult::default();
        }
        if key == KEY_DELETE
            && !self.flags.control()
            && let Some(undo) = self.undo.take()
        {
            return self.undo_fix(undo);
        }
        self.undo = None;
        self.log.key(
            key,
            self.flags,
            self.language(),
            &mut *self.preferences,
            self.platform,
        );
        let mut out = EventResult::default();
        if self.language() == Language::English {
            out.consumed = self.engine.config.auto_switch && self.english_key(&mut out);
            return out;
        }
        if self.other_language {
            return out;
        }
        self.engine.switch_request = None;
        self.engine.handle_event(
            Event::Keyboard,
            EventState::KeyDown,
            key,
            if shift {
                1
            } else if self.flags.contains(Modifiers::CAPS) {
                2
            } else {
                0
            },
            self.flags.control(),
        );
        if !matches!(
            self.engine.hook.code,
            HookCode::Process | HookCode::Restore | HookCode::RestoreAndReset
        ) {
            return out;
        }
        let at_break =
            self.engine.switch_request == Some(Language::English) && letter(key).is_none();
        let corrected = self.engine.hook.new_chars;
        self.send_output(&mut out);
        if self.engine.switch_request == Some(Language::English) {
            self.engine.switch_request = None;
            if at_break {
                self.remember_fix(corrected)
            } else {
                self.undo = None
            }
            self.auto_switch(
                Language::English,
                if at_break {
                    "vi_word_to_en"
                } else {
                    "double_key"
                },
                &mut out,
            );
            self.reset_english(at_break && whitespace(key));
            if at_break && key == KEY_SPACE && !self.engine.restored_keys.is_empty() {
                self.previous.push(self.engine.restored_keys.clone());
                self.previous_restored = true
            }
            self.after_double = !at_break;
        }
        out.consumed = true;
        out
    }
    pub fn click(&mut self) -> EventResult {
        if self.platform == Platform::Mac {
            self.cancel_modifier_chord();
        }
        self.undo = None;
        self.log.since_auto = false;
        self.log
            .end_word("click", &mut *self.preferences, self.platform);
        if self.platform == Platform::Linux {
            self.log.write(json!({"e":"reset"}))
        }
        if self.language() == Language::English {
            self.reset_english(true)
        } else {
            self.engine
                .handle_event(Event::Mouse, EventState::MouseDown, 0, 0, false);
        }
        EventResult::default()
    }
    pub fn insert_text(&mut self, text: &str) -> EventResult {
        self.reset_typing();
        EventResult {
            consumed: false,
            effects: if text.is_empty() {
                vec![]
            } else {
                vec![Effect::Text(text.into())]
            },
        }
    }
    pub fn take_log(&mut self) -> String {
        self.log.take(&mut *self.preferences)
    }
    fn cancel_modifier_chord(&mut self) {
        self.last_flags = Modifiers(0);
        if self.platform == Platform::Mac && self.flags.0 & 15 != 0 {
            self.modifier_chord_used = true;
        }
    }
    /// An unmodeled physical key bypasses transformation, but may end the word.
    pub fn unmodeled_key(&mut self, ends_word: bool) {
        self.cancel_modifier_chord();
        if ends_word {
            self.log
                .end_word("other", &mut *self.preferences, self.platform);
            self.reset_typing();
        }
    }
}

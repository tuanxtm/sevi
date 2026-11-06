use sevi_engine::{
    data::key_character,
    engine::{is_number, letter},
    keys::*,
    *,
};
pub mod check;
pub mod prep;
pub mod rng;
pub mod rtsim;
pub mod sim;
pub mod touch_eval;

pub fn key_for(c: char) -> Option<(u32, bool)> {
    if c.is_ascii_alphabetic() {
        return Some((
            LETTER_KEYS[(c.to_ascii_lowercase() as u8 - b'a') as usize],
            c.is_ascii_uppercase(),
        ));
    }
    Some(match c {
        ' ' => (KEY_SPACE, false),
        '\n' | '|' => (KEY_RETURN, false),
        '\t' => (KEY_TAB, false),
        '<' => (KEY_DELETE, false),
        ',' => (KEY_COMMA, false),
        '.' => (KEY_DOT, false),
        ';' => (KEY_SEMICOLON, false),
        ':' => (KEY_SEMICOLON, true),
        '\'' => (KEY_QUOTE, false),
        '"' => (KEY_QUOTE, true),
        '-' => (KEY_MINUS, false),
        '_' => (KEY_MINUS, true),
        '/' => (KEY_SLASH, false),
        '?' => (KEY_SLASH, true),
        '!' => (KEY_1, true),
        '(' => (KEY_9, true),
        ')' => (KEY_0, true),
        '[' => (KEY_LEFT_BRACKET, false),
        ']' => (KEY_RIGHT_BRACKET, false),
        '0'..='9' => (
            [
                KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
            ][c as usize - '0' as usize],
            false,
        ),
        _ => return None,
    })
}
pub fn config_from_env() -> EngineConfig {
    let mut c = EngineConfig::default();
    if let Ok(v) = std::env::var("SEVI_TELEX_STYLE") {
        c.telex_style = match v.as_str() {
            "1" => TelexStyle::Simple1,
            "2" => TelexStyle::Simple2,
            _ => TelexStyle::Telex,
        }
    }
    if let Ok(v) = std::env::var("SEVI_TEENCODE") {
        c.teencode = v != "0"
    }
    if std::env::var_os("SEVI_AUTO_OFF").is_some() {
        c.auto_switch = false;
        c.restore_if_invalid = false
    }
    c
}

/// The historical rule harness deliberately has its own host logic, independent of Driver.
#[derive(Clone)]
pub struct Harness {
    pub engine: Engine,
    pub screen: Vec<u16>,
    pub switches: usize,
    en: Vec<u32>,
    valid: bool,
    previous: Vec<Vec<u32>>,
    previous_restored: bool,
    after_double: bool,
    nav: Option<EnglishNavigation>,
    offset: i32,
}
type EnglishNavigation = (Vec<u32>, bool, bool, Vec<Vec<u32>>);
impl Harness {
    pub fn new(config: EngineConfig) -> Self {
        Self {
            engine: Engine::new(config),
            screen: vec![],
            switches: 0,
            en: vec![],
            valid: true,
            previous: vec![],
            previous_restored: false,
            after_double: false,
            nav: None,
            offset: 0,
        }
    }
    pub fn reset(&mut self, language: Language) {
        self.screen.clear();
        self.switches = 0;
        self.en.clear();
        self.valid = true;
        self.previous.clear();
        self.after_double = false;
        self.nav = None;
        self.offset = 0;
        self.engine.new_session();
        self.engine.forget();
        self.engine.config.language = language;
    }
    fn output(&mut self, key: u32, caps: bool) {
        self.screen.truncate(
            self.screen
                .len()
                .saturating_sub(self.engine.hook.backspaces),
        );
        for &c in self.engine.hook.chars[..self.engine.hook.new_chars]
            .iter()
            .rev()
        {
            self.screen.push(rendered_char(c) as u16)
        }
        if matches!(
            self.engine.hook.code,
            HookCode::Restore | HookCode::RestoreAndReset
        ) {
            let ch = key_character(key | if caps { CAPS_MASK } else { 0 });
            if ch != 0 {
                self.screen.push(ch as u16)
            } else if key == KEY_RETURN {
                self.screen.push(10)
            }
        }
        if self.engine.hook.code == HookCode::RestoreAndReset {
            self.engine.new_session()
        }
    }
    pub fn press(&mut self, c: char) {
        if c != '{' && c != '}' {
            self.nav = None;
            self.offset = 0;
        }
        let vi = self.engine.config.language == Language::Vietnamese;
        if c == '{' || c == '}' {
            if vi {
                self.engine.handle_event(
                    Event::Keyboard,
                    EventState::KeyDown,
                    if c == '{' { KEY_LEFT } else { KEY_RIGHT },
                    0,
                    false,
                );
            } else if c == '{'
                && self.offset == 0
                && (!self.en.is_empty() || !self.previous.is_empty())
            {
                self.nav = Some((
                    self.en.clone(),
                    self.valid,
                    self.after_double,
                    self.previous.clone(),
                ));
                self.offset = -1;
                self.en.clear();
                self.valid = false;
                self.previous.clear();
                self.after_double = false
            } else if self.offset < 0 {
                if c == '{' {
                    self.offset -= 1
                } else {
                    self.offset += 1;
                    if self.offset == 0
                        && let Some((en, v, d, previous)) = self.nav.take()
                    {
                        self.en = en;
                        self.valid = v;
                        self.after_double = d;
                        self.previous = previous;
                    }
                }
            } else {
                self.en.clear();
                self.valid = false;
                self.previous.clear();
                self.after_double = false
            }
            return;
        }
        if c == '@' {
            if vi {
                self.engine
                    .handle_event(Event::Mouse, EventState::MouseDown, 0, 0, false);
            } else {
                self.en.clear();
                self.valid = true;
                self.previous.clear();
                self.after_double = false;
                self.engine.forget()
            }
            return;
        }
        if c == '#' {
            self.engine.config.language = self.engine.config.language.toggled();
            self.engine.new_session();
            self.engine.forget();
            self.previous.clear();
            self.en.clear();
            self.valid = true;
            self.after_double = false;
            return;
        }
        let (key, caps) = key_for(c).unwrap_or((0xffff, false));
        if vi {
            self.engine.switch_request = None;
            self.engine.handle_event(
                Event::Keyboard,
                EventState::KeyDown,
                key,
                if caps { 1 } else { 0 },
                false,
            );
            if self.engine.hook.code == HookCode::Nothing {
                if key == KEY_DELETE {
                    self.screen.pop();
                } else if key == KEY_RETURN {
                    self.screen.push(10)
                } else {
                    self.screen
                        .push(key_character(key | if caps { CAPS_MASK } else { 0 }) as u16)
                }
            } else {
                self.output(key, caps)
            }
            if self.engine.switch_request == Some(Language::English) {
                self.engine.switch_request = None;
                self.engine.config.language = Language::English;
                self.engine.new_session();
                self.engine.forget();
                self.previous.clear();
                self.switches += 1;
                if key == KEY_SPACE && !self.engine.restored_keys.is_empty() {
                    self.previous.push(self.engine.restored_keys.clone());
                    self.previous_restored = true
                }
                self.en.clear();
                self.valid = key == KEY_SPACE || key == KEY_RETURN;
                self.after_double = letter(key).is_some();
            }
            return;
        }
        if letter(key).is_some() {
            if self.en.len() < MAX_BUFF - 2 {
                self.en.push(key | if caps { CAPS_MASK } else { 0 })
            } else {
                self.valid = false
            }
            self.screen.push(c as u16);
            return;
        }
        if key == KEY_DELETE {
            if self.en.is_empty() {
                self.previous.clear();
                self.after_double = false
            }
            if self.en.pop().is_none() {
                self.valid = false
            }
            self.screen.pop();
            return;
        }
        if is_number(key) && c.is_ascii_digit() {
            self.valid = false;
            self.after_double = false;
            self.screen.push(c as u16);
            return;
        }
        let word_end = [KEY_SPACE, KEY_COMMA, KEY_DOT, KEY_RETURN, KEY_SEMICOLON].contains(&key)
            || (caps && [KEY_SLASH, KEY_1, KEY_QUOTE, KEY_0].contains(&key));
        let en = std::mem::take(&mut self.en);
        let valid = self.valid;
        self.valid = key == KEY_SPACE
            || key == KEY_RETURN
            || (key == KEY_9 && caps)
            || key == KEY_LEFT_BRACKET
            || key == KEY_QUOTE;
        let double = self.after_double;
        self.after_double = false;
        if double && word_end && self.engine.fix_doubled_key(&en) {
            self.previous.clear();
            self.output(key, caps);
            return;
        }
        self.engine.previous_restored = self.previous_restored && !self.previous.is_empty();
        let converted = word_end
            && valid
            && en.len() >= 2
            && self.engine.check_english_word(&en, &self.previous);
        if !converted && key == KEY_SPACE && valid && !en.is_empty() {
            self.previous_restored = false;
            self.previous.insert(0, en);
            self.previous.truncate(2)
        } else {
            self.previous.clear()
        }
        if converted {
            self.output(key, caps);
            self.engine.config.language = Language::Vietnamese;
            self.engine.new_session();
            self.engine.forget();
            self.switches += 1;
            return;
        }
        self.screen
            .push(if key == KEY_RETURN { 10 } else { c as u16 });
    }
    pub fn text(&self) -> String {
        // The historical harness prints the UTF-8 buffer through printf("%s").
        // Match its NUL boundary even for unsupported synthetic key sequences.
        let end = self
            .screen
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(self.screen.len());
        String::from_utf16_lossy(&self.screen[..end])
    }
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\t{}",
            self.text(),
            self.switches,
            self.engine.config.language as i32
        )
    }
}

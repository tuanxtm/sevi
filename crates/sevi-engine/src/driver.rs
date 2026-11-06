use crate::{
    data::key_character,
    engine::{is_number, letter},
    keys::*,
    *,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    pub delete_count: usize,
    pub text: Vec<u16>,
    pub language_changed: bool,
}
#[derive(Clone)]
struct Undo {
    remove: usize,
    text: Vec<u16>,
    language: Language,
    keys: Vec<u32>,
}
#[derive(Clone)]
pub struct Driver {
    pub engine: Engine,
    pub pause_after_manual: f64,
    pub auto_switch: bool,
    pub auto_switches: usize,
    pub undos: usize,
    en: Vec<u32>,
    valid: bool,
    after_double: bool,
    previous: Vec<Vec<u32>>,
    previous_restored: bool,
    undo: Option<Undo>,
    undone: Vec<u32>,
    pause_until: Option<f64>,
    #[cfg(feature = "simulation")]
    pub simulation: Simulation,
}
/// Controls retained by the experimental corpus simulator; desktop defaults are unaffected.
#[cfg(feature = "simulation")]
#[derive(Clone, Default)]
pub struct Simulation {
    pub no_lookback: bool,
    pub no_restored_previous: bool,
    pub early_words: i32,
    pub early_direction: Option<Language>,
    run_side: i32,
    run_count: i32,
}
fn whitespace(key: u32) -> bool {
    [KEY_SPACE, KEY_RETURN, KEY_ENTER, KEY_TAB].contains(&key)
}
fn word_start(key: u32, shift: bool) -> bool {
    whitespace(key) || (key == KEY_9 && shift) || key == KEY_LEFT_BRACKET || key == KEY_QUOTE
}
pub fn typed_char(key: u32, shift: bool) -> u16 {
    if key == KEY_RETURN || key == KEY_ENTER {
        10
    } else {
        key_character(key | if shift { CAPS_MASK } else { 0 }) as u16
    }
}
impl Default for Driver {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}
impl Driver {
    pub fn new(config: EngineConfig) -> Self {
        let auto_switch = config.auto_switch;
        Self {
            engine: Engine::new(config),
            pause_after_manual: 5.0,
            auto_switch,
            auto_switches: 0,
            undos: 0,
            en: vec![],
            valid: true,
            after_double: false,
            previous: vec![],
            previous_restored: false,
            undo: None,
            undone: vec![],
            pause_until: None,
            #[cfg(feature = "simulation")]
            simulation: Simulation::default(),
        }
    }
    pub fn language(&self) -> Language {
        self.engine.config.language
    }
    /// Simulator host: its Backspace behavior predates the desktop immediate-undo policy.
    #[cfg(feature = "simulation")]
    pub fn press_simulated(&mut self, key: u32, shift: bool) -> Edit {
        self.undo = None;
        self.engine.last_side = -2;
        self.press(key, shift, 0.0)
    }
    #[cfg(feature = "simulation")]
    fn early_word(&mut self, side: i32) -> bool {
        if side < 0 {
            self.simulation.run_count = 0;
            return false;
        }
        if side == self.simulation.run_side {
            self.simulation.run_count += 1;
        } else {
            self.simulation.run_side = side;
            self.simulation.run_count = 1;
        }
        let language = if side == 0 {
            Language::English
        } else {
            Language::Vietnamese
        };
        if self.simulation.early_words > 0
            && self.simulation.run_count >= self.simulation.early_words
            && language != self.language()
            && self
                .simulation
                .early_direction
                .is_none_or(|d| d == language)
        {
            self.switch_to(language);
            self.en.clear();
            self.valid = true;
            self.previous.clear();
            self.after_double = false;
            return true;
        }
        false
    }
    #[cfg(feature = "simulation")]
    pub fn has_english_context(&self) -> bool {
        !self.en.is_empty() || !self.previous.is_empty()
    }
    #[cfg(feature = "simulation")]
    pub fn suspend_english_context(&mut self) {
        self.en.clear();
        self.valid = false;
        self.previous.clear();
        self.after_double = false;
    }
    pub fn set_language(&mut self, language: Language) {
        if self.language() != language {
            self.engine.config.language = language;
            self.reset_context()
        }
    }
    pub fn reset_context(&mut self) {
        self.undo = None;
        self.undone.clear();
        self.en.clear();
        self.valid = true;
        self.previous.clear();
        self.after_double = false;
        self.engine.new_session();
        self.engine.forget();
    }
    pub fn manual_switch(&mut self, now: f64) {
        #[cfg(feature = "simulation")]
        {
            self.simulation.run_count = 0;
        }
        self.reset_context();
        self.engine.config.language = self.language().toggled();
        if self.pause_after_manual > 0.0 {
            self.pause_until = Some(now + self.pause_after_manual);
            self.engine.config.auto_switch = false
        }
    }
    fn switch_to(&mut self, language: Language) {
        self.engine.config.language = language;
        self.engine.new_session();
        self.engine.forget();
        self.auto_switches += 1;
    }
    fn output(&mut self, e: &mut Edit, key: u32, shift: bool) {
        e.delete_count = self.engine.hook.backspaces;
        for &c in self.engine.hook.chars[..self.engine.hook.new_chars]
            .iter()
            .rev()
        {
            e.text.push(rendered_char(c) as u16)
        }
        if matches!(
            self.engine.hook.code,
            HookCode::Restore | HookCode::RestoreAndReset
        ) {
            let c = typed_char(key, shift);
            if c != 0 {
                e.text.push(c)
            }
        }
        if self.engine.hook.code == HookCode::RestoreAndReset {
            self.engine.new_session()
        }
    }
    fn remember_fix(&mut self, corrected: usize, key: u32, shift: bool) {
        self.undo = None;
        let brk = typed_char(key, shift);
        if brk == 0 || brk == 10 || self.engine.undo_data.is_empty() || corrected == 0 {
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
        let keys = if self
            .engine
            .undo_data
            .iter()
            .any(|c| c & CHAR_CODE_MASK != 0)
        {
            vec![]
        } else {
            self.engine
                .undo_data
                .iter()
                .take(MAX_BUFF)
                .copied()
                .collect()
        };
        self.undo = Some(Undo {
            remove: corrected + 1,
            text: self
                .engine
                .undo_data
                .iter()
                .map(|c| rendered_char(*c) as u16)
                .collect(),
            language: self.language(),
            keys,
        });
    }
    pub fn press(&mut self, key: u32, shift: bool, now: f64) -> Edit {
        if self.pause_until.is_some_and(|until| now >= until) {
            self.pause_until = None;
            self.engine.config.auto_switch = self.auto_switch;
            self.en.clear();
            self.valid = false
        }
        if self.pause_until.is_none() {
            self.engine.config.auto_switch = self.auto_switch
        }
        let mut e = Edit::default();
        if key == KEY_DELETE
            && let Some(undo) = self.undo.take()
        {
            e.delete_count = undo.remove;
            e.text = undo.text;
            e.language_changed = self.language() != undo.language;
            self.engine.config.language = undo.language;
            self.engine.new_session();
            self.engine.forget();
            self.en.clear();
            self.valid = false;
            self.previous.clear();
            self.after_double = false;
            self.undone.clear();
            if self.language() == Language::English && !undo.keys.is_empty() {
                self.en = undo.keys.clone();
                self.valid = true;
                self.undone = undo.keys
            }
            self.undos += 1;
            return e;
        }
        self.undo = None;
        if self.language() == Language::Vietnamese {
            self.engine.switch_request = None;
            self.engine.handle_event(
                Event::Keyboard,
                EventState::KeyDown,
                key,
                if shift { 1 } else { 0 },
                false,
            );
            if self.engine.hook.code == HookCode::Nothing {
                if key == KEY_DELETE {
                    e.delete_count = 1
                } else {
                    let c = typed_char(key, shift);
                    if c != 0 {
                        e.text.push(c)
                    }
                }
                #[cfg(feature = "simulation")]
                if self.engine.last_side != -2 {
                    e.language_changed = self.early_word(self.engine.last_side);
                }
            } else {
                let at_break =
                    self.engine.switch_request == Some(Language::English) && letter(key).is_none();
                let corrected = self.engine.hook.new_chars;
                self.output(&mut e, key, shift);
                if self.engine.switch_request == Some(Language::English) {
                    self.engine.switch_request = None;
                    if at_break {
                        self.remember_fix(corrected, key, shift)
                    }
                    self.switch_to(Language::English);
                    e.language_changed = true;
                    self.en.clear();
                    self.valid = at_break && whitespace(key);
                    self.previous.clear();
                    let remember = key == KEY_SPACE && !self.engine.restored_keys.is_empty();
                    #[cfg(feature = "simulation")]
                    let remember = remember && !self.simulation.no_restored_previous;
                    if remember {
                        self.previous.push(self.engine.restored_keys.clone());
                        self.previous_restored = true
                    }
                    self.after_double = !at_break;
                }
            }
            return e;
        }
        if letter(key).is_some() {
            if self.en.len() < MAX_BUFF - 2 {
                self.en.push(key | if shift { CAPS_MASK } else { 0 })
            } else {
                self.valid = false
            }
            e.text.push(typed_char(key, shift));
            return e;
        }
        if key == KEY_DELETE {
            if self.en.is_empty() {
                self.previous.clear();
                self.after_double = false
            }
            if self.en.pop().is_none() {
                self.valid = false
            }
            e.delete_count = 1;
            return e;
        }
        if is_number(key) && !shift {
            self.valid = self.en.is_empty() && self.valid;
            self.en.clear();
            self.previous.clear();
            self.after_double = false;
            e.text.push(typed_char(key, shift));
            return e;
        }
        let word_end = [
            KEY_SPACE,
            KEY_COMMA,
            KEY_DOT,
            KEY_SEMICOLON,
            KEY_RETURN,
            KEY_ENTER,
        ]
        .contains(&key)
            || (shift && [KEY_SLASH, KEY_1, KEY_QUOTE, KEY_0].contains(&key));
        let en = std::mem::take(&mut self.en);
        let valid = self.valid;
        let after_double = self.after_double;
        self.after_double = false;
        if after_double && word_end && self.engine.fix_doubled_key(&en) {
            self.valid = whitespace(key);
            self.previous.clear();
            let corrected = self.engine.hook.new_chars;
            self.output(&mut e, key, shift);
            self.remember_fix(corrected, key, shift);
            return e;
        }
        self.valid = word_start(key, shift);
        self.engine.previous_restored = self.previous_restored && !self.previous.is_empty();
        let same = !self.undone.is_empty() && en == self.undone;
        if word_end {
            self.undone.clear()
        }
        let previous = self.previous.as_slice();
        #[cfg(feature = "simulation")]
        let previous = if self.simulation.no_lookback {
            &[]
        } else {
            previous
        };
        let converted = word_end
            && valid
            && en.len() >= 2
            && !same
            && self.engine.check_english_word(&en, previous);
        #[cfg(feature = "simulation")]
        if !converted && word_end && valid && en.len() >= 2 {
            e.language_changed = self.early_word(self.engine.word_side(&en));
        }
        if !converted && key == KEY_SPACE && valid && !en.is_empty() {
            self.previous_restored = false;
            self.previous.insert(0, en);
            self.previous.truncate(2)
        } else {
            self.previous.clear()
        }
        if converted {
            let corrected = self.engine.hook.new_chars;
            self.output(&mut e, key, shift);
            self.remember_fix(corrected, key, shift);
            self.switch_to(Language::Vietnamese);
            e.language_changed = true;
            return e;
        }
        let c = typed_char(key, shift);
        if c != 0 {
            e.text.push(c)
        }
        e
    }
}

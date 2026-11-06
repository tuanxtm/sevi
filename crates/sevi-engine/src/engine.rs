use crate::{
    data::{self, MISSING},
    keys::*,
    tables,
    types::*,
};
use std::{collections::HashMap, sync::Arc};

// The original experiment tools compiled these C++ macros per run.
// Cargo tracks option_env! inputs and rebuilds the isolated Rust harness likewise.
const AS_PREFER: i32 = compile_setting(option_env!("AS_PREFER"), 10);
const AS_VN_FLOOR: i32 = compile_setting(option_env!("AS_VN_FLOOR"), 10);
const AS_EN_FLOOR: i32 = compile_setting(option_env!("AS_EN_FLOOR"), 99);
const AS_LOOKBACK_WORDS: usize = compile_setting(option_env!("AS_LOOKBACK_WORDS"), 2) as usize;
const AS_PLAIN_MARGIN: i32 = compile_setting(option_env!("AS_PLAIN_MARGIN"), 15);
const AS_LOOKBACK_MARGIN: i32 = compile_setting(option_env!("AS_LOOKBACK_MARGIN"), 10);
const AS_CAP_MARGIN: i32 = compile_setting(option_env!("AS_CAP_MARGIN"), 10);
const AS_NEXT_PAIR: i32 = compile_setting(option_env!("AS_NEXT_PAIR"), 20);
const AS_PLAIN_AFTER_AMBIGUOUS: i32 = compile_setting(option_env!("AS_PLAIN_AFTER_AMBIGUOUS"), 1);
const AS_ACRONYM: i32 = compile_setting(option_env!("AS_ACRONYM"), 1);
const AS_ACRONYM_VN: i32 = compile_setting(option_env!("AS_ACRONYM_VN"), 5);
const AS_ACRONYM_VN_AMID: i32 = compile_setting(option_env!("AS_ACRONYM_VN_AMID"), 15);
const AS_RESTORE_UNKNOWN: i32 = compile_setting(option_env!("AS_RESTORE_UNKNOWN"), 3);
const AS_VN_EXTREME: i32 = compile_setting(option_env!("AS_VN_EXTREME"), 99);
const AS_TIE_PREV: i32 = compile_setting(option_env!("AS_TIE_PREV"), 5);
const AS_CAPS_VN_FLOOR: i32 = compile_setting(option_env!("AS_CAPS_VN_FLOOR"), 15);
const AS_PAIR_OVERRIDES_D22: i32 = compile_setting(option_env!("AS_PAIR_OVERRIDES_D22"), 10);
const fn compile_setting(value: Option<&str>, fallback: i32) -> i32 {
    let Some(value) = value else { return fallback };
    let bytes = value.as_bytes();
    let negative = matches!(bytes.first(), Some(b'-'));
    let mut index = if negative { 1 } else { 0 };
    let mut n = 0;
    if index == bytes.len() {
        panic!("auto-switch threshold must be an integer");
    }
    while index < bytes.len() {
        let digit = bytes[index];
        if digit < b'0' || digit > b'9' {
            panic!("auto-switch threshold must be an integer");
        }
        n = n * 10 + (digit - b'0') as i32;
        index += 1;
    }
    if negative { -n } else { n }
}

#[derive(Clone, Default)]
pub(crate) struct PreviousWord {
    raw: Vec<u32>,
    shown: Vec<u32>,
    en: i32,
    vn: i32,
    lookback_ok: bool,
    same: bool,
}

/// All typing state belongs to this instance. Dictionaries are immutable shared data.
#[derive(Clone)]
pub struct Engine {
    pub config: EngineConfig,
    pub hook: HookResult,
    pub switch_request: Option<Language>,
    pub undo_data: Vec<u32>,
    pub restored_keys: Vec<u32>,
    pub previous_restored: bool,
    pub last_side: i32,
    pub(crate) word: [u32; MAX_BUFF],
    pub(crate) index: usize,
    states: [u32; MAX_BUFF],
    state_index: usize,
    states_lost: bool,
    long_word: Vec<u32>,
    typing: Vec<Vec<u32>>,
    special: Vec<u32>,
    temp_disable: bool,
    use_spelling_before: bool,
    vowel_start: usize,
    vowel_end: usize,
    vowel_count: i32,
    mark_at: usize,
    drops: Vec<(usize, u32)>,
    space_count: usize,
    word_start_ok: bool,
    after_digit: bool,
    apostrophe: bool,
    previous: Vec<PreviousWord>,
    converted: Vec<u32>,
    double_keys: Vec<u32>,
    double_first: Option<usize>,
    double_both: bool,
    nav: Option<Box<Engine>>,
    nav_offset: i32,
    pub(crate) bigrams: Option<Arc<HashMap<String, i32>>>,
    pub(crate) bigram_weight: f32,
    pub(crate) bigram_min: i32,
    pub(crate) trigrams: Option<Arc<HashMap<String, i32>>>,
    pub(crate) trigram_weight: f32,
    pub(crate) trigram_min: i32,
    pub(crate) capitals: Option<Arc<HashMap<String, i32>>>,
    pub(crate) capitals_min: i32,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}
impl Engine {
    pub fn load_bigrams(&mut self, path: Option<&std::path::Path>, weight: f32, min: i32) {
        self.bigram_weight = weight;
        self.bigram_min = min;
        if let Some(path) = path
            && let Ok(source) = std::fs::read_to_string(path)
        {
            let map = Arc::make_mut(self.bigrams.get_or_insert_with(|| Arc::new(HashMap::new())));
            load_counts(map, &source, 2);
        }
    }
    pub fn load_trigrams(&mut self, path: Option<&std::path::Path>, weight: f32, min: i32) {
        self.trigram_weight = weight;
        self.trigram_min = min;
        if let Some(path) = path
            && let Ok(source) = std::fs::read_to_string(path)
        {
            let map = Arc::make_mut(
                self.trigrams
                    .get_or_insert_with(|| Arc::new(HashMap::new())),
            );
            load_counts(map, &source, 3);
        }
    }
    pub fn load_capitals(&mut self, path: Option<&std::path::Path>, min: i32) {
        self.capitals_min = min;
        if let Some(path) = path
            && let Ok(source) = std::fs::read_to_string(path)
        {
            let map = Arc::make_mut(
                self.capitals
                    .get_or_insert_with(|| Arc::new(HashMap::new())),
            );
            load_counts(map, &source, 1);
        }
    }
    pub fn new(config: EngineConfig) -> Self {
        let spelling = config.spelling;
        Self {
            config,
            hook: HookResult::default(),
            switch_request: None,
            undo_data: vec![],
            restored_keys: vec![],
            previous_restored: false,
            last_side: -1,
            word: [0; MAX_BUFF],
            index: 0,
            states: [0; MAX_BUFF],
            state_index: 0,
            states_lost: false,
            long_word: vec![],
            typing: vec![],
            special: vec![],
            temp_disable: false,
            use_spelling_before: spelling,
            vowel_start: 0,
            vowel_end: 0,
            vowel_count: 0,
            mark_at: 0,
            drops: vec![],
            space_count: 0,
            word_start_ok: true,
            after_digit: false,
            apostrophe: false,
            previous: vec![],
            converted: vec![],
            double_keys: vec![],
            double_first: None,
            double_both: false,
            nav: None,
            nav_offset: 0,
            bigrams: None,
            bigram_weight: option_env!("AS_BIGRAM_W")
                .map(|v| v.parse().expect("AS_BIGRAM_W must be a number"))
                .unwrap_or(2.0),
            bigram_min: 3,
            trigrams: None,
            trigram_weight: 0.0,
            trigram_min: 3,
            capitals: None,
            capitals_min: 5,
        }
    }
    pub(crate) fn ch(&self, i: usize) -> u32 {
        self.word[i] & CHAR_MASK
    }
    pub fn new_session(&mut self) {
        self.drops.clear();
        self.index = 0;
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.temp_disable = false;
        self.state_index = 0;
        self.states_lost = false;
        self.long_word.clear();
    }
    pub fn forget(&mut self) {
        self.nav = None;
        self.nav_offset = 0;
        self.previous.clear();
        self.word_start_ok = true;
        self.after_digit = false;
        self.space_count = 0;
        self.special.clear();
        self.typing.clear();
        if !self.converted.is_empty() {
            self.typing.push(std::mem::take(&mut self.converted));
            self.space_count = 1;
        }
    }
    pub fn set_check_spelling(&mut self) {
        self.use_spelling_before = self.config.spelling;
    }
    pub(crate) fn fits(&self) -> bool {
        !self.states_lost && self.index < MAX_BUFF
    }
    fn is_break(&self, event: Event, key: u32) -> bool {
        event == Event::Mouse
            || (self.config.telex_style != TelexStyle::Telex && is_bracket(key))
            || [
                KEY_ESC,
                KEY_TAB,
                KEY_ENTER,
                KEY_RETURN,
                KEY_LEFT,
                KEY_RIGHT,
                KEY_DOWN,
                KEY_UP,
                KEY_COMMA,
                KEY_DOT,
                KEY_SLASH,
                KEY_SEMICOLON,
                KEY_QUOTE,
                KEY_BACK_SLASH,
                KEY_MINUS,
                KEY_EQUALS,
                KEY_BACKQUOTE,
            ]
            .contains(&key)
    }
    fn special_key(&self, key: u32) -> bool {
        [
            KEY_W, KEY_E, KEY_R, KEY_O, KEY_A, KEY_S, KEY_D, KEY_F, KEY_J, KEY_Z, KEY_X,
        ]
        .contains(&key)
            || (is_bracket(key) && self.config.telex_style == TelexStyle::Telex)
    }
    fn teen_known(&self) -> bool {
        let mut w = String::new();
        let mut tone = None;
        for &c in &self.word[..self.index] {
            let Some(l) = letter(c) else { return false };
            w.push(l as char);
            if c & TONE_MASK != 0 {
                w.push('^')
            }
            if c & TONEW_MASK != 0 {
                w.push('+')
            }
            if let Some(m) = tone_index(c) {
                tone = Some(m)
            }
        }
        if let Some(m) = tone {
            w.push((b'1' + m as u8) as char)
        }
        tables::TEENCODE.binary_search(&w.as_str()).is_ok()
    }
    fn teen_vowels(&self, from: usize, to: usize) -> bool {
        if !self.config.teencode || to - from != 2 {
            return false;
        }
        let (a, b) = (self.ch(from), self.ch(from + 1));
        let plain = self.word[from] & (TONE_MASK | TONEW_MASK) == 0
            && self.word[from + 1] & (TONE_MASK | TONEW_MASK) == 0;
        let m = self.word[from] & MARK_MASK;
        plain
            && ((a == KEY_I && b == KEY_E)
                || (a == KEY_O && b == KEY_Y && (m == 0 || m == MARK2_MASK)))
    }
    fn spelling(&mut self, force: bool) {
        let mut teen = false;
        let mut ok = false;
        let mut vowel_ok = true;
        let mut end = self.index;
        if end > 0 && self.ch(end - 1) == KEY_RIGHT_BRACKET {
            end -= 1
        }
        if end > 0 {
            let mut j = 0;
            if self.config.teencode && self.ch(0) == KEY_Z {
                j = 1;
                teen = true
            } else if self.config.teencode && end > 1 && self.ch(0) == KEY_D && self.ch(1) == KEY_Z
            {
                j = 2;
                teen = true
            } else if consonant(self.ch(0)) {
                for row in tables::CONSONANTS {
                    let mut flag = end < row.len();
                    j = 0;
                    while j < row.len() {
                        if end > j && row[j] != self.ch(j) {
                            flag = true;
                            break;
                        }
                        j += 1
                    }
                    if !flag {
                        break;
                    }
                }
            }
            if j == end {
                ok = true
            }
            let mut k = j;
            self.vowel_start = k;
            if self.ch(self.vowel_start) == KEY_U
                && k > 0
                && k < end - 1
                && self.ch(self.vowel_start - 1) == KEY_Q
            {
                k += 1;
                j = k;
                self.vowel_start = k
            } else if self.index >= 2
                && self.ch(0) == KEY_G
                && self.ch(1) == KEY_I
                && consonant(self.ch(2))
            {
                self.vowel_start = 1;
                k = 1;
                j = 1
            }
            for _ in 0..3 {
                if k < end && !consonant(self.ch(k)) {
                    k += 1;
                    self.vowel_end = k
                }
            }
            if k > j {
                vowel_ok = false;
                if k - j > 1 && force {
                    for row in data::rows(tables::VOWEL_COMBINE, self.ch(j)) {
                        let mut flag = false;
                        let mut ii = 1;
                        while ii < row.len() {
                            let at = j + ii - 1;
                            if at < end
                                && row[ii]
                                    != (self.ch(at) | (self.word[at] & (TONEW_MASK | TONE_MASK)))
                            {
                                flag = true;
                                break;
                            }
                            ii += 1
                        }
                        if flag
                            || (k < end && row[0] == 0)
                            || (j + ii - 1 < end && !consonant(self.ch(j + ii - 1)))
                        {
                            continue;
                        }
                        vowel_ok = true;
                        break;
                    }
                } else if !consonant(self.ch(j)) {
                    vowel_ok = true
                }
                if !vowel_ok && self.teen_vowels(j, k) && k == end {
                    vowel_ok = true;
                    teen = true
                }
                for row in tables::END_CONSONANTS {
                    let mut flag = false;
                    j = 0;
                    while j < row.len() {
                        if end > k + j && row[j] != self.ch(k + j) {
                            flag = true;
                            break;
                        }
                        j += 1
                    }
                    if !flag && k + j >= end {
                        ok = true;
                        break;
                    }
                }
                if !ok
                    && self.config.teencode
                    && k + 1 == end
                    && self.ch(k) == KEY_K
                    && k > 0
                    && (self.ch(k - 1) == KEY_I
                        || (self.ch(k - 1) == KEY_U && self.word[k - 1] & TONEW_MASK != 0))
                {
                    ok = true;
                    teen = true
                }
                if !ok && self.config.teencode && self.teen_vowels(self.vowel_start, k) && k == end
                {
                    ok = true;
                    teen = true
                }
                if ok {
                    let n = self.index;
                    let invalid_ch = n >= 3
                        && self.ch(n - 1) == KEY_H
                        && self.ch(n - 2) == KEY_C
                        && self.word[n - 3] & (MARK1_MASK | MARK5_MASK) == 0
                        && self.word[n - 3] & MARK_MASK != 0;
                    let invalid_t = n >= 2
                        && self.ch(n - 1) == KEY_T
                        && self.word[n - 2] & (MARK1_MASK | MARK5_MASK) == 0
                        && self.word[n - 2] & MARK_MASK != 0;
                    if invalid_ch || invalid_t {
                        ok = false
                    }
                }
            }
        } else {
            ok = true
        }
        if force && teen && ok && vowel_ok && !self.teen_known() {
            ok = false
        }
        self.temp_disable = !(ok && vowel_ok);
    }
    fn find_vowels(&mut self, grammar: bool) {
        self.vowel_count = 0;
        self.vowel_start = 0;
        self.vowel_end = 0;
        for i in (0..self.index).rev() {
            if consonant(self.ch(i)) {
                if self.vowel_count > 0 {
                    break;
                }
            } else {
                if self.vowel_count == 0 {
                    self.vowel_end = i
                }
                if !grammar
                    && i > 0
                    && ((self.ch(i) == KEY_I && self.ch(i - 1) == KEY_G)
                        || (self.ch(i) == KEY_U && self.ch(i - 1) == KEY_Q))
                {
                    break;
                }
                self.vowel_start = i;
                self.vowel_count += 1
            }
        }
        let v = self.vowel_start;
        if v > 0 && self.ch(v) == KEY_U && self.ch(v - 1) == KEY_Q {
            self.vowel_start += 1;
            self.vowel_count -= 1
        }
    }
    fn can_end_consonant(&self) -> bool {
        for row in data::rows(tables::VOWEL_COMBINE, self.ch(self.vowel_start)) {
            let mut k = self.vowel_start;
            let mut i = 1;
            while i < row.len() {
                if k > self.vowel_end
                    || (self.ch(k) | (self.word[k] & (TONE_MASK | TONEW_MASK))) != row[i]
                {
                    break;
                }
                k += 1;
                i += 1
            }
            if i >= row.len() {
                return row[0] == 1;
            }
        }
        false
    }
    fn modern_mark(&mut self) {
        let (s, e) = (self.vowel_start, self.vowel_end);
        self.mark_at = e;
        if self.vowel_count == 3
            && [
                (KEY_O, KEY_A, KEY_I),
                (KEY_U, KEY_Y, KEY_U),
                (KEY_O, KEY_E, KEY_O),
                (KEY_U, KEY_Y, KEY_A),
            ]
            .contains(&(self.ch(s), self.ch(s + 1), self.ch(s + 2)))
        {
            self.mark_at = s + 1
        } else if [(KEY_O, KEY_I), (KEY_A, KEY_I), (KEY_U, KEY_I)]
            .contains(&(self.ch(s), self.ch(s + 1)))
        {
            self.mark_at = s
        } else if e > 0 && self.ch(e - 1) == KEY_A && self.ch(e) == KEY_Y {
            self.mark_at = e - 1
        } else if self.ch(s) == KEY_U && self.ch(s + 1) == KEY_O {
            self.mark_at = s + 1
        } else if self.ch(s + 1) == KEY_O || self.ch(s + 1) == KEY_U {
            self.mark_at = e.saturating_sub(1)
        } else if self.ch(s) == KEY_O || self.ch(s) == KEY_U {
            self.mark_at = e
        }
        if ((self.ch(s) == KEY_I || self.ch(s) == KEY_Y)
            && self.word[s + 1] & (KEY_E | TONE_MASK) != 0)
            || (self.ch(s) == KEY_U && self.word[s + 1] == KEY_O | TONE_MASK)
            || (self.word[s] == KEY_U | TONEW_MASK && self.word[s + 1] == KEY_O | TONEW_MASK)
        {
            self.mark_at = if s + 2 < self.index
                && [KEY_P, KEY_T, KEY_M, KEY_N, KEY_O, KEY_U, KEY_I, KEY_C]
                    .contains(&self.ch(s + 2))
            {
                s + 1
            } else {
                s
            };
        } else if (self.ch(s) == KEY_I && self.ch(s) == KEY_A)
            || (self.ch(s) == KEY_Y && self.ch(s) == KEY_A)
            || (self.ch(s) == KEY_U && self.ch(s) == KEY_A)
            || (self.ch(s) == KEY_U && self.word[s + 1] == KEY_U | TONEW_MASK)
        {
            self.mark_at = s
        }
        if self.vowel_count == 2 {
            if self.ch(s) == KEY_I && [KEY_A, KEY_U, KEY_O].contains(&self.ch(s + 1)) {
                self.mark_at = if s == 0 || self.ch(s - 1) != KEY_G {
                    s
                } else {
                    s + 1
                }
            } else if self.ch(s) == KEY_U && self.ch(s + 1) == KEY_A {
                if s == 0 || self.ch(s - 1) != KEY_Q {
                    if e + 1 >= self.index || !self.can_end_consonant() {
                        self.mark_at = s
                    }
                } else {
                    self.mark_at = s + 1
                }
            } else if self.ch(s) == KEY_O && self.ch(s + 1) == KEY_O {
                self.mark_at = e
            }
        }
        self.hook.backspaces = self.index - self.mark_at;
    }
    fn old_mark(&mut self) {
        let (s, e) = (self.vowel_start, self.vowel_end);
        self.mark_at = if self.vowel_count == 0 && self.ch(e) == KEY_I {
            e
        } else {
            s
        };
        if self.vowel_count == 3
            || (e + 1 < self.index && consonant(self.ch(e + 1)) && self.can_end_consonant())
        {
            self.mark_at = s + 1
        }
        for i in s..=e {
            if (self.ch(i) == KEY_E && self.word[i] & TONE_MASK != 0)
                || (self.ch(i) == KEY_O && self.word[i] & TONEW_MASK != 0)
            {
                self.mark_at = i;
                break;
            }
        }
        self.hook.backspaces = self.index - self.mark_at;
        self.hook.new_chars = self.hook.backspaces;
    }
    fn insert_mark(&mut self, mark: u32, modify: bool) {
        if modify {
            self.hook.code = HookCode::Process
        }
        self.hook.backspaces = 0;
        self.hook.new_chars = 0;
        self.find_vowels(false);
        self.mark_at = 0;
        if self.vowel_count == 1 {
            self.mark_at = self.vowel_end;
            self.hook.backspaces = self.index - self.vowel_end
        } else {
            if self.config.modern_orthography {
                self.modern_mark()
            } else {
                self.old_mark()
            }
            if self.config.teencode
                && self.vowel_count == 2
                && self.ch(self.vowel_start) == KEY_O
                && self.ch(self.vowel_end) == KEY_Y
                && self.vowel_end == self.vowel_start + 1
            {
                self.mark_at = self.vowel_start;
                self.hook.backspaces = self.index - self.mark_at
            }
            if self.word[self.vowel_end] & (TONE_MASK | TONEW_MASK) != 0 {
                self.mark_at = self.vowel_end
            }
        }
        if self.word[self.mark_at] & mark != 0 {
            self.word[self.mark_at] &= !MARK_MASK;
            if modify {
                self.hook.code = HookCode::Restore
            }
            for i in self.vowel_start..self.index {
                self.word[i] &= !MARK_MASK;
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
            self.temp_disable = true
        } else {
            self.word[self.mark_at] = (self.word[self.mark_at] & !MARK_MASK) | mark;
            for i in self.vowel_start..self.index {
                if i != self.mark_at {
                    self.word[i] &= !MARK_MASK
                }
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
            self.hook.backspaces = self.index - self.vowel_start
        }
        self.hook.new_chars = self.hook.backspaces;
    }
    fn grammar(&mut self, delta: i32) {
        if self.index <= 1 || self.index >= MAX_BUFF {
            return;
        }
        self.find_vowels(true);
        if self.vowel_count == 0 {
            return;
        }
        let start = self.vowel_start;
        let mut changed = false;
        if self.index >= 3 {
            for i in (0..self.index).rev() {
                if [KEY_N, KEY_C, KEY_I, KEY_M, KEY_P, KEY_T].contains(&self.ch(i))
                    && i >= 2
                    && self.ch(i - 1) == KEY_O
                    && self.ch(i - 2) == KEY_U
                    && (self.word[i - 1] & TONEW_MASK) ^ (self.word[i - 2] & TONEW_MASK) != 0
                {
                    self.word[i - 2] |= TONEW_MASK;
                    self.word[i - 1] |= TONEW_MASK;
                    changed = true;
                    break;
                }
            }
        }
        for i in start..=self.vowel_end {
            let mark = self.word[i] & MARK_MASK;
            if mark != 0 {
                self.word[i] &= !MARK_MASK;
                self.insert_mark(mark, false);
                if i != self.mark_at {
                    changed = true
                }
                break;
            }
        }
        if changed {
            if self.hook.code == HookCode::Nothing {
                self.hook.code = HookCode::Process
            }
            self.hook.backspaces = self.index - start;
            for i in (start..self.index).rev() {
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
            self.hook.new_chars = self.hook.backspaces;
            self.hook.backspaces = (self.hook.backspaces as i32 + delta) as usize;
            self.hook.ext_code = 4
        }
    }
    fn insert_key(&mut self, key: u32, caps: bool, check: bool) {
        let value = key | if caps { CAPS_MASK } else { 0 };
        if self.index >= MAX_BUFF {
            self.long_word.push(self.word[0]);
            self.word.copy_within(1..MAX_BUFF, 0);
            self.word[self.index - 1] = value
        } else {
            self.word[self.index] = value;
            self.index += 1
        }
        if self.config.spelling && check {
            self.spelling(false)
        }
        if key == KEY_D && self.index >= 2 && consonant(self.ch(self.index - 2)) {
            self.temp_disable = false
        }
    }
    fn insert_state(&mut self, key: u32, caps: bool) {
        let value = key | if caps { CAPS_MASK } else { 0 };
        if self.state_index >= MAX_BUFF {
            self.states_lost = true;
            self.states.copy_within(1..MAX_BUFF, 0);
            self.states[self.state_index - 1] = value
        } else {
            self.states[self.state_index] = value;
            self.state_index += 1
        }
    }
    fn resync_states(&mut self) -> bool {
        let mut n = 0;
        let mut tone = None;
        // An all-caps word also used capital vowel/tone keys. A lone capital
        // may have used Shift, so retain the lowercase extra keys in that case.
        let upper = if self.index > 1 && self.word[..self.index].iter().all(|c| c & CAPS_MASK != 0)
        {
            CAPS_MASK
        } else {
            0
        };
        let mut x = 0;
        while x < self.index && n < MAX_BUFF - 3 {
            let c = self.word[x];
            if c & CHAR_CODE_MASK != 0 {
                return false;
            }
            let caps = c & CAPS_MASK;
            if c & STANDALONE_MASK != 0 && c & TONEW_MASK != 0 {
                self.states[n] = KEY_W | caps;
                n += 1
            } else {
                self.states[n] = self.ch(x) | caps;
                n += 1;
                if c & TONE_MASK != 0 {
                    self.states[n] = self.ch(x) | upper;
                    n += 1
                }
                if c & TONEW_MASK != 0 {
                    self.states[n] = KEY_W | upper;
                    n += 1
                }
            }
            if let Some(m) = tone_index(c) {
                tone = Some(m)
            }
            x += 1
        }
        if let Some(m) = tone {
            self.states[n] = [KEY_S, KEY_F, KEY_R, KEY_X, KEY_J][m] | upper;
            n += 1
        }
        self.state_index = n;
        self.drops.clear();
        x == self.index
    }
    fn save_word(&mut self) {
        if self.index > 0 {
            for row in self.long_word.chunks(MAX_BUFF) {
                self.typing.push(row.to_vec())
            }
            self.long_word.clear();
            self.typing.push(self.word[..self.index].to_vec());
        }
    }
    fn save_special(&mut self) {
        self.typing.push(std::mem::take(&mut self.special));
    }
    fn restore_typing(&mut self) {
        if let Some(state) = self.typing.pop() {
            if state.is_empty() {
                return;
            }
            if state[0] == KEY_SPACE {
                self.space_count = state.len();
                self.index = 0
            } else if is_char_code(state[0] & CHAR_MASK) {
                self.index = 0;
                self.special = state;
                self.spelling(false)
            } else {
                self.word[..state.len()].copy_from_slice(&state);
                self.index = state.len();
                self.state_index = 0;
                self.states_lost = !self.resync_states();
            }
        }
    }
    fn correct_vowel(&self, row: &[u32], mark: u32) -> bool {
        if self.index >= 2 && self.ch(self.index - 1) == KEY_U && self.ch(self.index - 2) == KEY_Q {
            return false;
        }
        let mut k = self.index as i32 - 1;
        for &c in row.iter().rev() {
            let at = k as usize;
            let teen = self.config.teencode
                && c & END_CONSONANT_MASK != 0
                && (c & !END_CONSONANT_MASK) == self.ch(at)
                && (self.ch(at) == KEY_K || (self.ch(at) == KEY_Y && mark == KEY_F));
            if c != self.ch(at) && !teen {
                return false;
            }
            k -= 1;
            if k < 0 {
                break;
            }
        }
        if row.len() > 1
            && [KEY_F, KEY_X, KEY_R].contains(&mark)
            && ([KEY_C, KEY_T].contains(&row[1]) || (row.len() > 2 && row[2] == KEY_T))
        {
            return false;
        }
        !(k >= 0 && self.ch(k as usize) == self.ch(k as usize + 1))
    }
    fn remove_mark(&mut self) -> bool {
        self.find_vowels(true);
        let mut changed = false;
        if self.index > 0 {
            for i in self.vowel_start..=self.vowel_end {
                if self.word[i] & MARK_MASK != 0 {
                    self.word[i] &= !MARK_MASK;
                    changed = true
                }
            }
        }
        if changed {
            self.hook.code = HookCode::Process;
            self.hook.backspaces = self.index - self.vowel_start;
            for i in (self.vowel_start..self.index).rev() {
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
            self.hook.new_chars = self.hook.backspaces
        } else {
            self.hook.code = HookCode::Nothing
        }
        changed
    }
    fn insert_d(&mut self) {
        self.hook.code = HookCode::Process;
        self.hook.backspaces = 0;
        for i in (0..self.index).rev() {
            self.hook.backspaces += 1;
            if self.ch(i) == KEY_D {
                if self.word[i] & TONE_MASK != 0 {
                    self.hook.code = HookCode::Restore;
                    self.word[i] &= !TONE_MASK;
                    self.hook.chars[self.index - 1 - i] = self.word[i];
                    self.temp_disable = true
                } else {
                    self.word[i] |= TONE_MASK;
                    self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
                }
                break;
            } else {
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }
    fn insert_aoe(&mut self, key: u32) {
        self.find_vowels(false);
        for i in self.vowel_start..=self.vowel_end {
            self.word[i] &= !TONEW_MASK
        }
        self.hook.code = HookCode::Process;
        self.hook.backspaces = 0;
        for i in (0..self.index).rev() {
            self.hook.backspaces += 1;
            if self.ch(i) == key {
                if self.word[i] & TONE_MASK != 0 {
                    self.hook.code = HookCode::Restore;
                    self.word[i] &= !TONE_MASK;
                    self.hook.chars[self.index - 1 - i] = self.word[i];
                    if key != KEY_O {
                        self.temp_disable = true
                    }
                } else {
                    self.word[i] |= TONE_MASK;
                    if key != KEY_D {
                        self.word[i] &= !TONEW_MASK
                    }
                    self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
                }
                break;
            } else {
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }
    fn insert_w(&mut self) {
        self.find_vowels(false);
        let (s, e) = (self.vowel_start, self.vowel_end);
        for i in s..=e {
            self.word[i] &= !TONE_MASK
        }
        if self.vowel_count > 1 {
            self.hook.backspaces = self.index - s;
            self.hook.new_chars = self.hook.backspaces;
            if self.word[s] & TONEW_MASK != 0
                && (self.word[s + 1] & TONEW_MASK != 0 || [KEY_I, KEY_A].contains(&self.ch(s + 1)))
            {
                self.hook.code = HookCode::Restore;
                for i in s..self.index {
                    self.word[i] &= !TONEW_MASK;
                    self.hook.chars[self.index - 1 - i] =
                        character_code(self.word[i]) & !STANDALONE_MASK;
                }
                self.temp_disable = true
            } else {
                self.hook.code = HookCode::Process;
                if self.ch(s) == KEY_U && self.ch(s + 1) == KEY_O {
                    if s >= 2 && self.word[s - 2] == KEY_T && self.word[s - 1] == KEY_H {
                        self.word[s + 1] |= TONEW_MASK;
                        if s + 2 < self.index && self.ch(s + 2) == KEY_N {
                            self.word[s] |= TONEW_MASK
                        }
                    } else if s >= 1 && self.word[s - 1] == KEY_Q {
                        self.word[s + 1] |= TONEW_MASK
                    } else {
                        self.word[s] |= TONEW_MASK;
                        self.word[s + 1] |= TONEW_MASK
                    }
                } else if [
                    (KEY_U, KEY_A),
                    (KEY_U, KEY_I),
                    (KEY_U, KEY_U),
                    (KEY_O, KEY_I),
                ]
                .contains(&(self.ch(s), self.ch(s + 1)))
                {
                    self.word[s] |= TONEW_MASK
                } else if [(KEY_I, KEY_O), (KEY_O, KEY_A)].contains(&(self.ch(s), self.ch(s + 1))) {
                    self.word[s + 1] |= TONEW_MASK
                } else {
                    self.temp_disable = true;
                    self.hook.code = HookCode::Nothing
                }
                for i in s..self.index {
                    self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
                }
            }
            return;
        }
        self.hook.code = HookCode::Process;
        self.hook.backspaces = 0;
        for i in (s..self.index).rev() {
            self.hook.backspaces += 1;
            if [KEY_A, KEY_U, KEY_O].contains(&self.ch(i)) {
                if self.word[i] & TONEW_MASK != 0 {
                    if self.word[i] & STANDALONE_MASK != 0 {
                        self.hook.code = HookCode::Process;
                        if self.ch(i) == KEY_U {
                            self.word[i] = KEY_W | (self.word[i] & CAPS_MASK)
                        } else if self.ch(i) == KEY_O {
                            self.hook.code = HookCode::Restore;
                            self.word[i] = KEY_O | (self.word[i] & CAPS_MASK)
                        }
                        self.hook.chars[self.index - 1 - i] = self.word[i]
                    } else {
                        self.hook.code = HookCode::Restore;
                        self.word[i] &= !TONEW_MASK;
                        self.hook.chars[self.index - 1 - i] = self.word[i]
                    }
                    self.temp_disable = true
                } else {
                    self.word[i] |= TONEW_MASK;
                    self.word[i] &= !TONE_MASK;
                    self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
                }
            } else {
                self.hook.chars[self.index - 1 - i] = character_code(self.word[i]);
            }
        }
        self.hook.new_chars = self.hook.backspaces;
    }
    fn standalone(&mut self, key: u32, caps: bool, reverse: u32) -> bool {
        if self.index > 0
            && self.ch(self.index - 1) == reverse
            && self.word[self.index - 1] & TONEW_MASK != 0
        {
            self.hook.code = HookCode::Process;
            self.hook.backspaces = 1;
            self.hook.new_chars = 1;
            self.word[self.index - 1] = key | if caps { CAPS_MASK } else { 0 };
            self.hook.chars[0] = character_code(self.word[self.index - 1]);
            return false;
        }
        let append = (self.index > 0 && self.ch(self.index - 1) == KEY_U && reverse == KEY_O)
            || (self.index == 0 && !(self.after_digit && key == KEY_W))
            || (self.index == 1 && !tables::STANDALONE_W_BAD.contains(&self.ch(0)))
            || (self.index == 2
                && tables::DOUBLE_W
                    .iter()
                    .any(|r| r[0] == self.ch(0) && r[1] == self.ch(1)));
        // When [ follows u, the spelling pass sees an o before it acquires
        // its standalone horn. This differs from the other standalone paths.
        let after_u = self.index > 0 && self.ch(self.index - 1) == KEY_U && reverse == KEY_O;
        self.insert_key(
            if after_u { reverse } else { key },
            caps,
            after_u || !append,
        );
        if append {
            self.hook.code = HookCode::Process;
            self.hook.backspaces = 0;
            self.hook.new_chars = 1;
            self.hook.ext_code = 4;
            self.word[self.index - 1] =
                reverse | TONEW_MASK | STANDALONE_MASK | if caps { CAPS_MASK } else { 0 };
            self.hook.chars[0] = character_code(self.word[self.index - 1]);
        }
        append
    }
    fn main_key(&mut self, key: u32, caps: bool) -> bool {
        if key == KEY_Z {
            if !self.remove_mark() {
                self.insert_key(key, caps, true)
            }
            return false;
        }
        if is_bracket(key) {
            return self.standalone(
                key,
                caps,
                if key == KEY_LEFT_BRACKET {
                    KEY_O
                } else {
                    KEY_U
                },
            );
        }
        if key == KEY_D {
            for row in tables::CONSONANT_D {
                if self.index < row.len() {
                    continue;
                }
                if self.correct_vowel(row, key)
                    || (self.index >= 2
                        && self.ch(self.index - 1) == KEY_D
                        && consonant(self.ch(self.index - 2)))
                {
                    self.insert_d();
                    return false;
                }
            }
            self.insert_key(key, caps, true);
            return false;
        }
        if let Some(mark) = mark_key(key) {
            for (_, set) in tables::VOWEL_FOR_MARK {
                for row in *set {
                    if self.index >= row.len() && self.correct_vowel(row, key) {
                        self.insert_mark(mark, true);
                        return false;
                    }
                }
            }
            self.insert_key(key, caps, true);
            return false;
        }
        for row in data::rows(tables::VOWEL, key) {
            if self.index >= row.len() && self.correct_vowel(row, key) {
                if [KEY_A, KEY_O, KEY_E].contains(&key) {
                    self.insert_aoe(key)
                } else if key == KEY_W {
                    self.insert_w();
                    if self.hook.code == HookCode::Nothing {
                        break;
                    }
                }
                return false;
            }
        }
        if key == KEY_W && self.config.telex_style != TelexStyle::Simple1 {
            return self.standalone(key, caps, KEY_U);
        }
        self.insert_key(key, caps, true);
        false
    }
    fn restore_invalid(&mut self, code: HookCode) -> bool {
        if !self.fits() {
            return false;
        }
        for i in 0..self.index {
            if !consonant(self.ch(i)) && self.word[i] & (MARK_MASK | TONE_MASK | TONEW_MASK) != 0 {
                self.hook.code = code;
                self.hook.backspaces = self.index;
                self.hook.new_chars = self.state_index;
                for i in 0..self.state_index {
                    self.word[i] = self.states[i];
                    self.hook.chars[self.state_index - 1 - i] = self.word[i];
                }
                self.index = self.state_index;
                return true;
            }
        }
        false
    }
}

fn load_counts(map: &mut HashMap<String, i32>, source: &str, columns: usize) {
    for line in source.lines() {
        if columns == 1 && line.starts_with('#') {
            continue;
        }
        let mut cursor = 0;
        let mut valid = true;
        for _ in 0..columns {
            if let Some(at) = line[cursor..].find('\t') {
                cursor += at + 1
            } else {
                valid = false;
                break;
            }
        }
        if valid {
            let key = &line[..cursor - 1];
            let count = line[cursor..].trim().parse::<i32>().unwrap_or(0);
            let value = map.entry(key.into()).or_insert(0);
            *value = value.wrapping_add(count);
        }
    }
}

pub fn consonant(key: u32) -> bool {
    ![KEY_A, KEY_E, KEY_U, KEY_Y, KEY_I, KEY_O].contains(&key)
}
pub fn is_bracket(key: u32) -> bool {
    key == KEY_LEFT_BRACKET || key == KEY_RIGHT_BRACKET
}
pub fn is_number(key: u32) -> bool {
    [
        KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
    ]
    .contains(&key)
}
pub fn is_char_code(key: u32) -> bool {
    [
        KEY_BACKQUOTE,
        KEY_1,
        KEY_2,
        KEY_3,
        KEY_4,
        KEY_5,
        KEY_6,
        KEY_7,
        KEY_8,
        KEY_9,
        KEY_0,
        KEY_MINUS,
        KEY_EQUALS,
        KEY_LEFT_BRACKET,
        KEY_RIGHT_BRACKET,
        KEY_BACK_SLASH,
        KEY_SEMICOLON,
        KEY_QUOTE,
        KEY_COMMA,
        KEY_DOT,
        KEY_SLASH,
    ]
    .contains(&key)
}
pub fn letter(value: u32) -> Option<u8> {
    LETTER_KEYS
        .iter()
        .position(|k| *k == value & CHAR_MASK)
        .map(|i| b'a' + i as u8)
}
pub fn tone_index(value: u32) -> Option<usize> {
    [MARK1_MASK, MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK]
        .iter()
        .position(|m| *m == value & MARK_MASK)
}
pub fn mark_key(key: u32) -> Option<u32> {
    [KEY_S, KEY_F, KEY_R, KEY_X, KEY_J]
        .iter()
        .position(|k| *k == key)
        .map(|i| [MARK1_MASK, MARK2_MASK, MARK3_MASK, MARK4_MASK, MARK5_MASK][i])
}
pub fn character_code(value: u32) -> u32 {
    let caps = if value & CAPS_MASK != 0 { 0 } else { 1 };
    let mut key = value & CHAR_MASK;
    if let Some(m) = tone_index(value) {
        let mut at = m * 2 + caps;
        if [KEY_A, KEY_O, KEY_U, KEY_E].contains(&key) && value & (TONE_MASK | TONEW_MASK) == 0 {
            at += 4
        }
        if value & TONE_MASK != 0 {
            key |= TONE_MASK
        } else if value & TONEW_MASK != 0 {
            key |= TONEW_MASK
        }
        if let Some(row) = data::unicode(key) {
            return row[at] | CHAR_CODE_MASK;
        }
    } else if let Some(row) = data::unicode(key) {
        if value & TONE_MASK != 0 {
            return row[caps] | CHAR_CODE_MASK;
        } else if value & TONEW_MASK != 0 {
            return row[caps + 2] | CHAR_CODE_MASK;
        }
    }
    value & !STANDALONE_MASK
}
pub fn rendered_char(value: u32) -> u32 {
    if value & CHAR_CODE_MASK != 0 {
        value & CHAR_MASK
    } else {
        data::key_character(value)
    }
}

fn raw_word(keys: &[u32]) -> Option<String> {
    if keys.is_empty() || keys.len() >= 40 {
        return None;
    }
    keys.iter().map(|k| letter(*k).map(char::from)).collect()
}
fn few_capitals(keys: &[u32]) -> bool {
    let mut n = 0;
    for (i, k) in keys.iter().enumerate() {
        if k & CAPS_MASK == 0 {
            continue;
        }
        if i > 0
            && k & CHAR_MASK == KEY_D
            && keys[i - 1] & CHAR_MASK == KEY_D
            && keys[i - 1] & CAPS_MASK != 0
        {
            continue;
        }
        n += 1
    }
    n < 2
}
fn capitalised_only(keys: &[u32]) -> bool {
    keys.len() >= 2 && keys[0] & CAPS_MASK != 0 && keys[1..].iter().all(|k| k & CAPS_MASK == 0)
}
fn vowel_groups(keys: &[u32]) -> usize {
    let (mut n, mut inside) = (0, false);
    for k in keys {
        let v = letter(*k).is_some_and(|c| b"aeiouy".contains(&c));
        if v && !inside {
            n += 1
        }
        inside = v
    }
    n
}
fn ratio(vn: i32, en: i32, min: i32, weight: f32) -> i32 {
    if vn + en < min {
        return 0;
    }
    (10.0 * ((vn as f64 + 0.5) / (en as f64 + 0.5)).log10())
        .clamp(-30.0, 30.0)
        .mul_add(weight as f64, 0.0)
        .round() as i32
}

impl Engine {
    pub fn vietnamese_score(&self, word: &str) -> i32 {
        let score = data::VN.score(word.as_bytes());
        if score == MISSING && self.config.teencode {
            tables::TEENCODE_VN
                .binary_search_by_key(&word, |(w, _)| *w)
                .map(|i| tables::TEENCODE_VN[i].1)
                .unwrap_or(MISSING)
        } else {
            score
        }
    }
    fn word_key(&self) -> Option<(String, bool)> {
        if self.index == 0 || !self.fits() {
            return None;
        }
        let (mut w, mut transformed, mut tone) = (String::new(), false, None);
        for &c in &self.word[..self.index] {
            let l = letter(c)?;
            if w.len() + 3 >= 40 {
                return None;
            }
            w.push(l as char);
            if c & TONE_MASK != 0 && b"aeod".contains(&l) {
                w.push(l as char);
                transformed = true
            } else if c & TONEW_MASK != 0 && b"aou".contains(&l) {
                w.push('w');
                transformed = true
            }
            if let Some(m) = tone_index(c) {
                tone = Some(m)
            }
        }
        if let Some(m) = tone {
            w.push(b"sfrxj"[m] as char);
            transformed = true
        }
        Some((w, transformed))
    }
    fn scratch_session(&self) -> Self {
        // The legacy replay moves history out of the live engine before replaying.
        // Do not clone the unbounded history for every dictionary lookup.
        let mut s = Self::new(self.config.clone());
        s.word = self.word;
        s.states = self.states;
        s.use_spelling_before = self.use_spelling_before;
        s.word_start_ok = self.word_start_ok;
        s.after_digit = self.after_digit;
        s.bigrams = self.bigrams.clone();
        s.bigram_weight = self.bigram_weight;
        s.bigram_min = self.bigram_min;
        s.trigrams = self.trigrams.clone();
        s.trigram_weight = self.trigram_weight;
        s.trigram_min = self.trigram_min;
        s.capitals = self.capitals.clone();
        s.capitals_min = self.capitals_min;
        s
    }
    fn replay(&self, keys: &[u32]) -> (i32, Vec<u32>, bool, Vec<u32>) {
        let mut scratch = self.scratch_session();
        for &k in keys {
            scratch.handle_event(
                Event::Keyboard,
                EventState::KeyDown,
                k & CHAR_MASK,
                if k & CAPS_MASK != 0 { 1 } else { 0 },
                false,
            );
        }
        let normalized = scratch.word_key();
        let changed = normalized.as_ref().map(|(_, t)| *t).unwrap_or(true);
        if let Some((w, true)) = normalized {
            let score = self.vietnamese_score(&w);
            if score != MISSING {
                let states = scratch.word[..scratch.index].to_vec();
                let text = states.iter().copied().map(character_code).collect();
                return (score, text, changed, states);
            }
        }
        (MISSING, vec![], changed, vec![])
    }
    pub fn word_score(&self, vietnamese: bool, word: &str) -> (i32, Vec<u16>) {
        if word.is_empty() || word.len() >= 39 {
            return (MISSING, vec![]);
        }
        if !vietnamese {
            let s = data::EN.score(word.as_bytes());
            return (
                s,
                if s == MISSING {
                    vec![]
                } else {
                    word.encode_utf16().collect()
                },
            );
        }
        if !word.bytes().all(|b| b.is_ascii_lowercase()) {
            return (MISSING, vec![]);
        }
        let keys: Vec<u32> = word
            .bytes()
            .map(|b| LETTER_KEYS[(b - b'a') as usize])
            .collect();
        let (s, r, changed, _) = self.replay(&keys);
        if s != MISSING {
            return (s, r.into_iter().map(|c| rendered_char(c) as u16).collect());
        }
        if !changed {
            let s = self.vietnamese_score(word);
            return (
                s,
                if s == MISSING {
                    vec![]
                } else {
                    word.encode_utf16().collect()
                },
            );
        }
        (s, vec![])
    }
    fn surface(&self, word: &str, vietnamese: bool) -> Option<String> {
        if !vietnamese {
            return Some(word.into());
        }
        let (s, text) = self.word_score(true, word);
        if s <= MISSING || text.is_empty() {
            None
        } else {
            String::from_utf16(&text).ok()
        }
    }
    fn pair_count(&self, key: &str) -> i32 {
        if let Some(table) = &self.bigrams {
            *table.get(key).unwrap_or(&0)
        } else {
            data::BIGRAM
                .index(key.as_bytes())
                .map(|i| 2f64.powf(data::BIGRAM.scores[i] as f64 / 8.0).round() as i32)
                .unwrap_or(0)
        }
    }
    fn bigram_vote(&self, prev: &str, prev_vi: bool, word: &str) -> i32 {
        if self.bigram_weight <= 0.0 {
            return 0;
        }
        let (Some(p), Some(v)) = (self.surface(prev, prev_vi), self.surface(word, true)) else {
            return 0;
        };
        ratio(
            self.pair_count(&format!("{p}\t{v}")),
            self.pair_count(&format!("{p}\t{word}")),
            self.bigram_min,
            self.bigram_weight,
        )
    }
    fn next_pair_vote(&self, prev: &str, next: &str) -> i32 {
        if self.bigram_weight <= 0.0 {
            return 0;
        }
        let (Some(v), Some(n)) = (self.surface(prev, true), self.surface(next, true)) else {
            return 0;
        };
        ratio(
            self.pair_count(&format!("{v}\t{n}")),
            self.pair_count(&format!("{prev}\t{n}")),
            self.bigram_min,
            self.bigram_weight,
        )
    }
    fn trigram_vote(&self, p2: &str, p2vi: bool, p1: &str, p1vi: bool, word: &str) -> i32 {
        let Some(t) = &self.trigrams else { return 0 };
        if self.trigram_weight <= 0.0 {
            return 0;
        }
        let (Some(a), Some(b), Some(v)) = (
            self.surface(p2, p2vi),
            self.surface(p1, p1vi),
            self.surface(word, true),
        ) else {
            return 0;
        };
        ratio(
            *t.get(&format!("{a}\t{b}\t{v}")).unwrap_or(&0),
            *t.get(&format!("{a}\t{b}\t{word}")).unwrap_or(&0),
            self.trigram_min,
            self.trigram_weight,
        )
    }
    fn cap_vote(&self, word: &str) -> i32 {
        let Some(t) = &self.capitals else { return 0 };
        let Some(v) = self.surface(word, true) else {
            return 0;
        };
        ratio(
            *t.get(&v).unwrap_or(&0),
            *t.get(word).unwrap_or(&0),
            self.capitals_min,
            1.0,
        )
    }
    fn full_states(&self, max: usize) -> Vec<u32> {
        let mut out = self.states[..self.state_index.min(max)].to_vec();
        for &(p, k) in self.drops.iter().rev() {
            if p <= out.len() && out.len() < max {
                out.insert(p, k)
            }
        }
        out
    }
    fn restore_english_core(&mut self) -> bool {
        if !self.config.auto_switch
            || !self.word_start_ok
            || self.state_index < 2
            || self.state_index < self.index
        {
            return false;
        }
        let normalized = self.word_key();
        if !self.drops.is_empty()
            && self.state_index >= 3
            && self.state_index + self.drops.len() < MAX_BUFF - 1
        {
            let full = self.full_states(MAX_BUFF);
            let saved = self.temp_disable;
            self.spelling(true);
            let invalid = self.temp_disable;
            self.temp_disable = saved;
            if raw_word(&full).is_some_and(|raw| data::EN.score(raw.as_bytes()) != MISSING)
                && normalized.as_ref().is_none_or(|(word, t)| {
                    !*t || invalid || self.vietnamese_score(word) == MISSING
                })
            {
                self.states[..full.len()].copy_from_slice(&full);
                self.state_index = full.len();
                self.drops.clear();
                return true;
            }
        }
        let Some((word, transformed)) = normalized else {
            return false;
        };
        let raw = raw_word(&self.states[..self.state_index]);
        if !transformed {
            if raw
                .as_ref()
                .is_some_and(|r| r != &word && data::EN.score(r.as_bytes()) != MISSING)
            {
                return true;
            }
            if AS_PLAIN_AFTER_AMBIGUOUS == 0 {
                return false;
            }
            let Some(prev) = self.previous.first() else {
                return false;
            };
            if !prev.lookback_ok || prev.same || prev.en < prev.vn + AS_PLAIN_MARGIN {
                return false;
            }
            return raw.as_ref().is_some_and(|r| {
                data::EN.score(r.as_bytes()) != MISSING && self.vietnamese_score(r) == MISSING
            });
        }
        let Some(raw) = raw else { return false };
        let mut en = data::EN.score(raw.as_bytes());
        let vn = self.vietnamese_score(&word);
        if self.capitals.is_some()
            && vn != MISSING
            && !self.previous.is_empty()
            && capitalised_only(&self.states[..self.state_index])
        {
            let vote = self.cap_vote(&raw);
            if vote <= -AS_CAP_MARGIN {
                return true;
            }
            if vote >= AS_CAP_MARGIN {
                return false;
            }
        }
        let after_lower = self
            .previous
            .first()
            .is_none_or(|p| p.raw.iter().all(|k| k & CAPS_MASK == 0));
        if AS_ACRONYM != 0
            && !few_capitals(&self.states[..self.state_index])
            && (vn == MISSING
                || vn
                    < if after_lower {
                        AS_ACRONYM_VN_AMID
                    } else {
                        AS_ACRONYM_VN
                    })
            && self.word[..self.index].iter().any(|c| {
                letter(*c).is_some_and(|l| b"aeiouy".contains(&l))
                    && c & (MARK_MASK | TONE_MASK | TONEW_MASK) != 0
            })
        {
            return true;
        }
        if en == MISSING && self.apostrophe && raw.len() >= 3 && raw.ends_with('n') {
            en = data::EN.score(&raw.as_bytes()[..raw.len() - 1])
        }
        if en == MISSING {
            if AS_RESTORE_UNKNOWN <= 0
                || vn != MISSING
                || self.state_index < AS_RESTORE_UNKNOWN as usize
            {
                return false;
            }
            if option_env!("AS_UNKNOWN_ALLOW_MODIFIERS").is_none()
                && vowel_groups(&self.states[..self.state_index]) <= 1
                && self.word[..self.index]
                    .iter()
                    .any(|c| c & (TONE_MASK | TONEW_MASK) != 0)
            {
                return false;
            }
            let saved = self.temp_disable;
            self.spelling(true);
            let invalid = self.temp_disable;
            self.temp_disable = saved;
            return invalid;
        }
        if (self.bigram_weight > 0.0 || self.trigrams.is_some())
            && vn != MISSING
            && !self.previous.is_empty()
            && let Some(p) = raw_word(&self.previous[0].raw)
        {
            let mut vote = 0;
            if self.previous.len() >= 2
                && let Some(p2) = raw_word(&self.previous[1].raw)
            {
                vote = self.trigram_vote(
                    &p2,
                    !self.previous[1].same,
                    &p,
                    !self.previous[0].same,
                    &raw,
                )
            }
            if vote == 0 {
                vote = self.bigram_vote(&p, !self.previous[0].same, &raw)
            }
            if vote != 0 {
                return en - vn - vote >= AS_PREFER && (vn < AS_VN_FLOOR || vote <= -AS_PREFER);
            }
        }
        if vn != MISSING
            && en - vn >= AS_TIE_PREV
            && self
                .previous
                .first()
                .is_some_and(|p| p.same && p.en != MISSING && p.vn == MISSING)
        {
            return true;
        }
        vn == MISSING || (vn < AS_VN_FLOOR && en - vn >= AS_PREFER) || en - vn >= AS_VN_EXTREME
    }
    fn should_restore_english(&mut self) -> bool {
        if !self.restore_english_core() {
            return false;
        }
        if !self.drops.is_empty()
            && self.state_index >= 3
            && self.state_index + self.drops.len() < MAX_BUFF - 1
        {
            let full = self.full_states(MAX_BUFF - 1);
            if vowel_groups(&full) >= 2
                && raw_word(&self.states[..self.state_index])
                    .is_none_or(|r| data::EN.score(r.as_bytes()) == MISSING)
            {
                self.states[..full.len()].copy_from_slice(&full);
                self.state_index = full.len();
                self.drops.clear();
            }
        }
        true
    }
    fn hyphen_restore(&self) -> bool {
        if !self.config.auto_switch
            || !self.word_start_ok
            || self.state_index < 2
            || self.state_index < self.index
        {
            return false;
        }
        let Some((w, true)) = self.word_key() else {
            return false;
        };
        if letter(self.states[0]) != Some(b'w') {
            return false;
        }
        let v = self.vietnamese_score(&w);
        v == MISSING || v < AS_VN_FLOOR
    }
    fn remember_word(&mut self) {
        if !self.config.auto_switch {
            return;
        }
        if self.state_index == 0
            || self.index == 0
            || self.state_index >= MAX_BUFF
            || !self.long_word.is_empty()
        {
            self.previous.clear();
            return;
        }
        let mut w = PreviousWord {
            raw: self.states[..self.state_index].to_vec(),
            shown: self.word[..self.index]
                .iter()
                .copied()
                .map(character_code)
                .collect(),
            en: MISSING,
            vn: MISSING,
            ..Default::default()
        };
        self.last_side = if self.word_start_ok {
            self.word_side(&w.raw)
        } else {
            -1
        };
        if self.word_start_ok
            && self.state_index >= self.index
            && let (Some((word, t)), Some(raw)) = (self.word_key(), raw_word(&w.raw))
        {
            w.en = data::EN.score(raw.as_bytes());
            if t {
                w.vn = self.vietnamese_score(&word);
                w.lookback_ok =
                    w.en != MISSING && w.vn != MISSING && w.en >= w.vn - AS_LOOKBACK_MARGIN
            } else {
                w.lookback_ok = w.en != MISSING && self.vietnamese_score(&raw) == MISSING;
                w.same = true
            }
        }
        self.previous.insert(0, w);
        self.previous.truncate(AS_LOOKBACK_WORDS);
    }
    fn restore_english(&mut self, code: HookCode) {
        self.restored_keys.clear();
        if self.state_index > 0 && self.state_index < MAX_BUFF {
            self.restored_keys
                .extend_from_slice(&self.states[..self.state_index])
        }
        let (mut back, mut n, mut delete) = (0, self.state_index, self.index);
        let acronym = !few_capitals(&self.states[..self.state_index]);
        while !acronym
            && back < self.previous.len()
            && self.previous[back].lookback_ok
            && n + self.previous[back].raw.len() + 1 < MAX_BUFF
            && delete + self.previous[back].shown.len() + 1 < MAX_BUFF
        {
            n += self.previous[back].raw.len() + 1;
            delete += self.previous[back].shown.len() + 1;
            back += 1
        }
        while back > 0 && self.previous[back - 1].same {
            back -= 1;
            delete -= self.previous[back].shown.len() + 1
        }
        let mut out = vec![];
        self.undo_data.clear();
        for p in self.previous[..back].iter().rev() {
            out.extend_from_slice(&p.raw);
            out.push(KEY_SPACE);
            self.undo_data.extend_from_slice(&p.shown);
            self.undo_data.push(KEY_SPACE)
        }
        for &c in &self.word[..self.index] {
            if self.undo_data.len() < MAX_BUFF {
                self.undo_data.push(character_code(c))
            }
        }
        self.word[..self.state_index].copy_from_slice(&self.states[..self.state_index]);
        out.extend_from_slice(&self.states[..self.state_index]);
        self.hook.code = code;
        self.hook.backspaces = delete;
        self.hook.new_chars = out.len();
        for (i, c) in out.into_iter().rev().enumerate() {
            self.hook.chars[i] = c
        }
        self.index = self.state_index;
        self.previous.clear();
        self.switch_request = Some(Language::English);
    }
    pub fn check_english_word(&mut self, keys: &[u32], previous: &[Vec<u32>]) -> bool {
        let count = keys.len();
        if !self.config.auto_switch || !(2..MAX_BUFF - 1).contains(&count) {
            return false;
        }
        let Some(raw) = raw_word(keys) else {
            return false;
        };
        let all_caps = keys.iter().all(|k| k & CAPS_MASK != 0);
        if !few_capitals(keys) && !all_caps {
            return false;
        }
        let en = data::EN.score(raw.as_bytes());
        let (vn, result, _, states) = self.replay(keys);
        let mut pair = 0;
        if (self.bigram_weight > 0.0 || self.trigrams.is_some())
            && en != MISSING
            && vn != MISSING
            && !previous.is_empty()
            && let Some(p) = raw_word(&previous[0])
        {
            if previous.len() >= 2
                && let Some(p2) = raw_word(&previous[1])
            {
                pair = self.trigram_vote(&p2, false, &p, false, &raw)
            }
            if pair == 0 {
                pair = self.bigram_vote(&p, false, &raw)
            }
        }
        let mut cap = 0;
        if self.capitals.is_some()
            && vn != MISSING
            && !previous.is_empty()
            && capitalised_only(keys)
        {
            cap = self.cap_vote(&raw);
            if cap <= -AS_CAP_MARGIN {
                return false;
            }
        }
        if vn == MISSING
            || !(en == MISSING
                || cap >= AS_CAP_MARGIN
                || (en < AS_EN_FLOOR && vn - en + pair >= AS_PREFER))
        {
            return false;
        }
        if count > 1 && all_caps && (en != MISSING || vn < AS_CAPS_VN_FLOOR) {
            return false;
        }
        let marked = raw.as_bytes().windows(2).any(|p| {
            (p[0] == p[1] && b"aeod".contains(&p[0])) || (p[1] == b'w' && b"aou".contains(&p[0]))
        });
        if count > 1
            && all_caps
            && !(count >= 4 && marked)
            && !previous
                .first()
                .is_some_and(|p| p.len() >= 2 && p.iter().all(|k| k & CAPS_MASK != 0))
        {
            return false;
        }
        if en != MISSING
            && !previous.is_empty()
            && pair < AS_PAIR_OVERRIDES_D22
            && !self.previous_restored
            && cap < AS_CAP_MARGIN
            && let Some(p) = raw_word(&previous[0])
        {
            let pen = data::EN.score(p.as_bytes());
            let (pvn, _, _, _) = self.replay(&previous[0]);
            if pen != MISSING && (pvn == MISSING || pen - pvn >= AS_PREFER) {
                return false;
            }
        }
        let (mut delete, mut n) = (count, 0);
        let mut earlier: Vec<(Vec<u32>, bool)> = vec![];
        for (back, pk) in previous.iter().take(AS_LOOKBACK_WORDS).enumerate() {
            let pc = pk.len();
            let Some(p) = raw_word(pk) else { break };
            if pc < 2 || !few_capitals(pk) {
                break;
            }
            let pen = data::EN.score(p.as_bytes());
            let (pvn, mut text, changed, _) = self.replay(pk);
            if !changed {
                if pen != MISSING || self.vietnamese_score(&p) == MISSING {
                    break;
                }
                text = pk.clone()
            } else if pen == MISSING
                || pvn == MISSING
                || if back == 0 && self.previous_restored {
                    self.next_pair_vote(&p, &raw) < AS_NEXT_PAIR
                } else {
                    pvn < pen - AS_LOOKBACK_MARGIN
                        && !(back == 0 && self.next_pair_vote(&p, &raw) >= AS_NEXT_PAIR)
                }
            {
                break;
            }
            if result.len() + n + text.len() + 1 >= MAX_BUFF || delete + pc + 1 >= MAX_BUFF {
                break;
            }
            n += text.len() + 1;
            delete += pc + 1;
            earlier.push((text, !changed));
        }
        while earlier.last().is_some_and(|(_, same)| *same) {
            let i = earlier.len() - 1;
            delete -= previous[i].len() + 1;
            earlier.pop();
        }
        let mut out = vec![];
        self.undo_data.clear();
        for b in (0..earlier.len()).rev() {
            out.extend_from_slice(&earlier[b].0);
            out.push(KEY_SPACE);
            self.undo_data.extend_from_slice(&previous[b]);
            self.undo_data.push(KEY_SPACE)
        }
        out.extend_from_slice(&result);
        self.undo_data.extend_from_slice(keys);
        self.converted = states;
        self.hook.code = HookCode::Restore;
        self.hook.backspaces = delete;
        self.hook.new_chars = out.len();
        self.hook.ext_code = 3;
        for (i, c) in out.into_iter().rev().enumerate() {
            self.hook.chars[i] = c
        }
        true
    }
    pub fn fix_doubled_key(&mut self, keys: &[u32]) -> bool {
        let mut full = std::mem::take(&mut self.double_keys);
        let n = full.len();
        let Some(first) = self.double_first else {
            return false;
        };
        if !self.config.auto_switch || n < 2 || n + keys.len() >= MAX_BUFF {
            return false;
        }
        full.extend_from_slice(keys);
        let mut single = full.clone();
        single.remove(first);
        let (Some(raw_full), Some(raw_single)) = (raw_word(&full), raw_word(&single)) else {
            return false;
        };
        let (en_full, en_single) = (
            data::EN.score(raw_full.as_bytes()) != MISSING,
            data::EN.score(raw_single.as_bytes()) != MISSING,
        );
        let fixed = if self.double_both && !en_full && en_single {
            &single
        } else if !self.double_both && !en_single && (en_full || !few_capitals(&full)) {
            &full
        } else {
            return false;
        };
        self.undo_data = if self.double_both {
            full.clone()
        } else {
            single.clone()
        };
        self.hook.code = HookCode::Restore;
        self.hook.backspaces = self.undo_data.len();
        self.hook.new_chars = fixed.len();
        self.hook.ext_code = 3;
        for (i, c) in fixed.iter().rev().enumerate() {
            self.hook.chars[i] = *c
        }
        true
    }
    pub fn word_side(&self, keys: &[u32]) -> i32 {
        if !self.config.auto_switch
            || !(2..MAX_BUFF - 1).contains(&keys.len())
            || !few_capitals(keys)
        {
            return -1;
        }
        let Some(raw) = raw_word(keys) else { return -1 };
        let en = data::EN.score(raw.as_bytes());
        let (vn, _, _, _) = self.replay(keys);
        if en != MISSING && vn == MISSING {
            0
        } else if vn != MISSING && en == MISSING {
            1
        } else {
            -1
        }
    }
    pub fn is_vietnamese_spelling(&self, word: &str) -> bool {
        if word.is_empty() || word.len() >= 39 || !word.bytes().all(|b| b.is_ascii_lowercase()) {
            return false;
        }
        let mut s = self.scratch_session();
        for l in word.bytes() {
            s.handle_event(
                Event::Keyboard,
                EventState::KeyDown,
                LETTER_KEYS[(l - b'a') as usize],
                0,
                false,
            );
        }
        let disabled = s.temp_disable;
        s.spelling(true);
        !disabled && !s.temp_disable
    }
    fn switch_break(&self, event: Event, key: u32, caps: u8, control: bool) -> bool {
        event == Event::Keyboard
            && !control
            && ([
                KEY_COMMA,
                KEY_DOT,
                KEY_SEMICOLON,
                KEY_ENTER,
                KEY_RETURN,
                KEY_QUOTE,
            ]
            .contains(&key)
                || (caps == 1 && [KEY_SLASH, KEY_1, KEY_0, KEY_9].contains(&key))
                || (self.config.telex_style != TelexStyle::Telex && is_bracket(key)))
    }
    pub fn handle_event(
        &mut self,
        event: Event,
        state: EventState,
        key: u32,
        caps: u8,
        control: bool,
    ) -> &HookResult {
        if event == Event::Keyboard
            && (key == KEY_LEFT || key == KEY_RIGHT)
            && caps != 1
            && !control
        {
            if key == KEY_LEFT && self.nav_offset == 0 && (self.index > 0 || self.space_count > 0) {
                self.nav = None;
                self.nav = Some(Box::new(self.clone()));
                self.previous.clear();
                self.typing.clear();
                self.special.clear();
                self.long_word.clear();
                self.nav_offset = -1;
                self.new_session();
                self.hook.code = HookCode::Nothing;
                self.hook.backspaces = 0;
                self.hook.new_chars = 0;
                self.hook.ext_code = 1;
                return &self.hook;
            }
            if self.nav.is_some() {
                if key == KEY_LEFT {
                    self.nav_offset -= 1
                } else {
                    self.nav_offset += 1;
                    if self.nav_offset == 0 {
                        let saved = self.nav.take().unwrap();
                        // Match AsEngineState: a navigation restore must not resurrect a
                        // consumed switch request, undo data, or pending doubled-key fix.
                        self.word = saved.word;
                        self.states = saved.states;
                        self.index = saved.index;
                        self.state_index = saved.state_index;
                        self.states_lost = saved.states_lost;
                        self.temp_disable = saved.temp_disable;
                        self.use_spelling_before = saved.use_spelling_before;
                        self.word_start_ok = saved.word_start_ok;
                        self.drops = saved.drops;
                        self.space_count = saved.space_count;
                        self.config.spelling = saved.config.spelling;
                        self.config.language = saved.config.language;
                        self.long_word = saved.long_word;
                        self.special = saved.special;
                        self.typing = saved.typing;
                        self.hook = saved.hook;
                        self.previous = saved.previous;
                    }
                }
                self.hook.code = HookCode::Nothing;
                self.hook.backspaces = 0;
                self.hook.new_chars = 0;
                self.hook.ext_code = 1;
                return &self.hook;
            }
        }
        self.nav = None;
        self.nav_offset = 0;
        let is_caps = caps == 1 || caps == 2;
        if (is_number(key) && caps == 1)
            || control
            || self.is_break(event, key)
            || (is_number(key) && (self.index == 0 || self.space_count > 0))
        {
            self.hook.code = HookCode::Nothing;
            self.hook.backspaces = 0;
            self.hook.new_chars = 0;
            self.hook.ext_code = 1;
            self.apostrophe = key == KEY_QUOTE && caps != 1;
            let switch = (self.switch_break(event, key, caps, control)
                && self.space_count == 0
                && self.should_restore_english())
                || (key == KEY_MINUS
                    && caps != 1
                    && !control
                    && self.space_count == 0
                    && self.hyphen_restore());
            self.apostrophe = false;
            if switch {
                self.restore_english(HookCode::RestoreAndReset)
            } else if self.config.restore_if_invalid && self.is_break(event, key) {
                if !self.temp_disable && self.config.spelling {
                    self.spelling(true)
                }
                if self.temp_disable && !self.restore_invalid(HookCode::RestoreAndReset) {
                    self.hook.code = HookCode::Nothing
                }
            }
            if state != EventState::KeyDown || !is_char_code(key) {
                self.special.clear();
                self.typing.clear()
            } else {
                if self.space_count > 0 {
                    self.typing.push(vec![KEY_SPACE; self.space_count]);
                    self.space_count = 0
                } else {
                    self.save_word()
                }
                self.special.push(key | if is_caps { CAPS_MASK } else { 0 });
                self.hook.ext_code = 3
            }
            if self.hook.code == HookCode::Nothing || event == Event::Mouse {
                self.new_session();
                self.config.spelling = self.use_spelling_before
            }
            self.previous.clear();
            self.after_digit = is_number(key) && caps != 1;
            self.word_start_ok = event == Event::Mouse
                || control
                || [KEY_ENTER, KEY_RETURN, KEY_TAB, KEY_LEFT_BRACKET, KEY_QUOTE].contains(&key)
                || (key == KEY_9 && caps == 1);
        } else if key == KEY_SPACE {
            self.after_digit = false;
            if !self.temp_disable && self.config.spelling {
                self.spelling(true)
            }
            if self.should_restore_english() {
                self.restore_english(HookCode::Restore);
                self.space_count += 1
            } else if self.config.restore_if_invalid && self.temp_disable {
                if !self.restore_invalid(HookCode::Restore) {
                    self.hook.code = HookCode::Nothing
                }
                self.space_count += 1
            } else {
                self.hook.code = HookCode::Nothing;
                self.space_count += 1
            }
            if self.hook.code == HookCode::Nothing && self.space_count == 1 {
                self.remember_word()
            } else {
                self.previous.clear()
            }
            if self.space_count == 1 {
                if !self.special.is_empty() {
                    self.save_special()
                } else {
                    self.save_word()
                }
            }
            self.config.spelling = self.use_spelling_before;
            self.word_start_ok = true;
        } else if key == KEY_DELETE {
            self.previous.clear();
            self.hook.code = HookCode::Nothing;
            self.hook.ext_code = 2;
            if !self.special.is_empty() {
                self.special.pop();
                if self.special.is_empty() {
                    self.restore_typing()
                }
            } else if self.space_count > 0 {
                self.space_count -= 1;
                if self.space_count == 0 {
                    self.restore_typing()
                }
            } else {
                if self.state_index > 0 {
                    self.state_index -= 1;
                    while self
                        .drops
                        .last()
                        .is_some_and(|(p, _)| self.state_index < *p)
                    {
                        self.drops.pop();
                    }
                }
                if self.index > 0 {
                    self.index -= 1;
                    let several = self.word[self.index] & (TONE_MASK | TONEW_MASK | MARK_MASK) != 0;
                    let plain = self.word[..self.index].iter().all(|c| {
                        c & (TONE_MASK | TONEW_MASK | MARK_MASK | STANDALONE_MASK | CHAR_CODE_MASK)
                            == 0
                    });
                    if several && plain && self.long_word.is_empty() {
                        self.resync_states();
                    }
                    if let Some(c) = self.long_word.pop() {
                        self.word.copy_within(0..MAX_BUFF - 1, 1);
                        self.word[0] = c;
                        self.index += 1
                    }
                    if self.config.spelling {
                        self.spelling(false)
                    }
                }
                self.hook.backspaces = 0;
                self.hook.new_chars = 0;
                self.hook.ext_code = 2;
                if self.index == 0 {
                    self.new_session();
                    self.special.clear();
                    self.restore_typing()
                } else {
                    self.grammar(1)
                }
            }
        } else {
            if self.space_count > 0 {
                self.hook.backspaces = 0;
                self.hook.new_chars = 0;
                self.hook.ext_code = 0;
                self.new_session();
                self.typing.push(vec![KEY_SPACE; self.space_count]);
                self.space_count = 0
            } else if !self.special.is_empty() {
                self.save_special()
            }
            self.insert_state(key, is_caps);
            let mut appended = false;
            if !self.special_key(key) || self.temp_disable {
                self.hook.code = HookCode::Nothing;
                self.hook.backspaces = 0;
                self.hook.new_chars = 0;
                self.hook.ext_code = 3;
                self.insert_key(key, is_caps, true)
            } else {
                self.hook.code = HookCode::Nothing;
                self.hook.ext_code = 3;
                appended = self.main_key(key, is_caps);
                if self.hook.code == HookCode::Restore
                    && self.config.auto_switch
                    && mark_key(key).is_some()
                {
                    self.switch_request = Some(Language::English);
                    if self.state_index >= 2 && self.fits() {
                        let first = (0..self.state_index - 1)
                            .rev()
                            .find(|&i| self.states[i] & CHAR_MASK == key);
                        let both = first.is_none()
                            || raw_word(&self.states[..self.state_index])
                                .is_some_and(|r| data::EN.score(r.as_bytes()) != MISSING);
                        self.hook.backspaces = self.index;
                        self.hook.new_chars = 0;
                        for i in (0..self.state_index - 1).rev() {
                            if both || Some(i) != first {
                                self.hook.chars[self.hook.new_chars] = self.states[i];
                                self.hook.new_chars += 1
                            }
                        }
                        self.double_keys = self.states[..self.state_index].to_vec();
                        self.double_first = first;
                        self.double_both = both;
                    }
                }
            }
            if key != KEY_D {
                self.grammar(if self.hook.code == HookCode::Nothing || appended {
                    -1
                } else {
                    0
                })
            }
            if self.hook.code == HookCode::Restore {
                self.insert_key(key, is_caps, true);
                if self.state_index > 0 && self.switch_request != Some(Language::English) {
                    if self.drops.len() == 4 {
                        self.drops.remove(0);
                    }
                    self.drops
                        .push((self.state_index - 1, self.states[self.state_index - 1]));
                }
                self.state_index = self.state_index.wrapping_sub(1);
            }
            if is_bracket(key) && is_bracket(self.hook.chars[0] & CHAR_MASK) {
                if self.index
                    > if self.hook.code == HookCode::Process {
                        self.hook.backspaces
                    } else {
                        0
                    }
                {
                    self.index -= 1;
                    self.save_word()
                }
                self.index = 0;
                self.temp_disable = false;
                self.state_index = 0;
                self.hook.ext_code = 3;
                self.special.push(key | if is_caps { CAPS_MASK } else { 0 });
            }
        }
        &self.hook
    }
}

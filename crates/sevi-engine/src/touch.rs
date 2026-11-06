//! Existing touch-key selection and correction model. No desktop UI uses it.
use crate::{Engine, data};
use std::sync::LazyLock;
#[derive(Clone, Copy, Debug)]
pub struct TouchKey {
    pub letter: u8,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub letters: String,
    pub text: Vec<u16>,
    pub vietnamese: bool,
    pub score: f64,
}
#[derive(Clone, Debug)]
pub struct TouchModel {
    pub sigma: f32,
    pub sigma_y: f32,
    pub weight: f32,
    pub sure_zone: f32,
    pub mode_share: f32,
    pub correct_margin: f32,
    pub correct_floor: f32,
    pub delete_cost: f32,
    pub correct_floor_long: f32,
    pub keep_words: String,
}
impl Default for TouchModel {
    fn default() -> Self {
        Self {
            sigma: 0.35,
            sigma_y: 0.25,
            weight: 0.5,
            sure_zone: 0.8,
            mode_share: 0.8,
            correct_margin: 1.0,
            correct_floor: -2.0,
            delete_cost: -7.0,
            correct_floor_long: -4.0,
            keep_words: String::new(),
        }
    }
}
impl TouchModel {
    fn prior(&self, prefix: &str, vietnamese: bool) -> [f64; 26] {
        let (vn, en) = (
            data::next_key_mass(true, prefix),
            data::next_key_mass(false, prefix),
        );
        let (tv, te) = (vn[..26].iter().sum::<f64>(), en[..26].iter().sum::<f64>());
        let mut wv = if vietnamese {
            self.mode_share
        } else {
            1.0 - self.mode_share
        } as f64;
        let mut we = 1.0 - wv;
        if tv <= 0.0 {
            wv = 0.0;
            we = 1.0
        }
        if te <= 0.0 {
            we = 0.0;
            wv = if tv > 0.0 { 1.0 } else { 0.0 }
        }
        let mut p = [0.0; 26];
        for i in 0..26 {
            let v = if tv > 0.0 { vn[i] / tv } else { 0.0 } * wv
                + if te > 0.0 { en[i] / te } else { 0.0 } * we;
            p[i] = v * 0.97 + 0.03 / 26.0
        }
        let total = p.iter().sum::<f64>();
        for v in &mut p {
            *v /= total
        }
        p
    }
    pub fn pick(
        &self,
        x: f32,
        y: f32,
        prefix: &str,
        vietnamese: bool,
        keys: &[TouchKey],
    ) -> Option<u8> {
        let mut nearest = None;
        let mut distance = 1e30f32;
        for k in keys {
            let dx = (x - (k.x + k.width / 2.0)) / k.width;
            let dy = (y - (k.y + k.height / 2.0)) / k.height;
            if dx.abs() * 2.0 <= self.sure_zone && dy.abs() * 2.0 <= self.sure_zone {
                return Some(k.letter);
            }
            let d = dx * dx + dy * dy;
            if d < distance {
                distance = d;
                nearest = Some(k.letter)
            }
        }
        let mut letter = nearest?;
        if self.weight <= 0.0 {
            return Some(letter);
        }
        let p = self.prior(prefix, vietnamese);
        let mut best = -1e300;
        for k in keys {
            let dx = (x - (k.x + k.width / 2.0)) / (k.width * self.sigma);
            let dy = (y - (k.y + k.height / 2.0)) / (k.height * self.sigma_y);
            let spatial = -0.5 * (dx * dx + dy * dy) as f64;
            if spatial < -8.0 || !k.letter.is_ascii_lowercase() {
                continue;
            }
            let score = spatial + self.weight as f64 * p[(k.letter - b'a') as usize].ln();
            if score > best {
                best = score;
                letter = k.letter
            }
        }
        Some(letter)
    }
    fn spatial(&self, typed: u8, c: u8, touch: Option<(f32, f32)>, keys: &[TouchKey]) -> f64 {
        let Some(k) = keys.iter().find(|k| k.letter == c) else {
            return -20.0;
        };
        let (x, y) = if let Some(p) = touch {
            p
        } else {
            let Some(t) = keys.iter().find(|k| k.letter == typed) else {
                return -20.0;
            };
            (t.x + t.width / 2.0, t.y + t.height / 2.0)
        };
        let dx = (x - (k.x + k.width / 2.0)) / (k.width * self.sigma);
        let dy = (y - (k.y + k.height / 2.0)) / (k.height * self.sigma_y);
        -0.5 * (dx * dx + dy * dy) as f64
    }
    #[allow(clippy::approx_constant)] // Preserve the original model's calibrated decimal constant.
    fn add(
        engine: &Engine,
        letters: &str,
        cost: f64,
        mode: bool,
        out: &mut Vec<Candidate>,
        max: usize,
    ) {
        if max == 0 {
            return;
        }
        for v in [false, true] {
            let (s, text) = engine.word_score(v, letters);
            if s <= -1000 || text.is_empty() || text.len() > 47 {
                continue;
            }
            let score = s as f64 / 10.0 * 2.302585 + cost + if v == mode { 0.0 } else { -1.5 };
            let c = Candidate {
                letters: letters.into(),
                text,
                vietnamese: v,
                score,
            };
            if let Some(i) = out.iter().position(|old| old.text == c.text) {
                if out[i].score < score {
                    out[i] = c
                }
                continue;
            }
            if out.len() < max {
                out.push(c)
            } else {
                let mut worst = 0;
                for i in 1..out.len() {
                    if out[i].score < out[worst].score {
                        worst = i
                    }
                }
                if out[worst].score < score {
                    out[worst] = c
                }
            }
        }
    }
    pub fn candidates(
        &self,
        engine: &Engine,
        typed: &str,
        touches: Option<&[(f32, f32)]>,
        vietnamese: bool,
        keys: &[TouchKey],
        max: usize,
    ) -> Vec<Candidate> {
        let len = typed.len();
        if !(1..=24).contains(&len) || !typed.is_ascii() || touches.is_some_and(|t| t.len() < len) {
            return vec![];
        }
        let mut out = vec![];
        Self::add(engine, typed, 0.0, vietnamese, &mut out, max);
        for i in 0..len {
            let touch = touches.map(|t| t[i]);
            let here = self.spatial(typed.as_bytes()[i], typed.as_bytes()[i], touch, keys);
            for c in b'a'..=b'z' {
                if c == typed.as_bytes()[i] {
                    continue;
                }
                let cost = self.spatial(typed.as_bytes()[i], c, touch, keys) - here - 1.0;
                if cost < -9.0 {
                    continue;
                }
                let mut word = typed.as_bytes().to_vec();
                word[i] = c;
                Self::add(
                    engine,
                    std::str::from_utf8(&word).unwrap(),
                    cost,
                    vietnamese,
                    &mut out,
                    max,
                )
            }
            if len > 3 {
                let mut word = typed.to_owned();
                word.remove(i);
                Self::add(
                    engine,
                    &word,
                    self.delete_cost as f64,
                    vietnamese,
                    &mut out,
                    max,
                )
            }
            if i + 1 < len && typed.as_bytes()[i] != typed.as_bytes()[i + 1] {
                let mut word = typed.as_bytes().to_vec();
                word.swap(i, i + 1);
                Self::add(
                    engine,
                    std::str::from_utf8(&word).unwrap(),
                    -4.0,
                    vietnamese,
                    &mut out,
                    max,
                )
            }
        }
        if len < 23 {
            for i in 0..=len {
                for c in b'a'..=b'z' {
                    let mut word = typed.as_bytes().to_vec();
                    word.insert(i, c);
                    Self::add(
                        engine,
                        std::str::from_utf8(&word).unwrap(),
                        -6.5,
                        vietnamese,
                        &mut out,
                        max,
                    )
                }
            }
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out
    }
    pub fn should_correct(&self, engine: &Engine, typed: &str, candidates: &[Candidate]) -> bool {
        let Some(best) = candidates.first() else {
            return false;
        };
        if candidates.iter().any(|c| c.letters == typed) || typed.len() < 3 {
            return false;
        }
        if inflected(engine, typed) {
            let word = best.letters.as_bytes();
            let t = typed.as_bytes();
            let same = word.len() >= 2
                && t.len() >= 2
                && word[word.len() - 2..] == t[t.len() - 2..]
                && word.get(..3) == t.get(..3);
            if !same {
                return false;
            }
        }
        if engine.is_vietnamese_spelling(typed) {
            return false;
        }
        static KEEP: LazyLock<Vec<&'static str>> =
            LazyLock::new(|| include_str!("data/keep.words").lines().collect());
        if KEEP.binary_search(&typed).is_ok() || self.keep_words.split('\n').any(|w| w == typed) {
            return false;
        }
        let margin = if candidates.len() > 1 {
            best.score - candidates[1].score
        } else {
            10.0
        };
        let floor = if typed.len() >= 6 {
            self.correct_floor_long
        } else if typed.len() == 5 {
            (self.correct_floor + self.correct_floor_long) / 2.0
        } else {
            self.correct_floor
        };
        best.score > floor as f64 && margin > self.correct_margin as f64
    }
}
fn inflected(engine: &Engine, w: &str) -> bool {
    for suffix in ["s", "es", "ed", "d", "ing", "er", "ers", "ly", "est"] {
        if let Some(base) = w.strip_suffix(suffix) {
            if base.len() < 3 || w.len() >= 31 {
                continue;
            }
            if engine.word_score(false, base).0 > -1000 {
                return true;
            }
            if ["ing", "ed", "er"].contains(&suffix)
                && engine.word_score(false, &format!("{base}e")).0 > -1000
            {
                return true;
            }
        }
    }
    false
}

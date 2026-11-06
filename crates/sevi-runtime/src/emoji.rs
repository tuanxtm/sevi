use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
#[derive(Clone, Debug)]
struct Term {
    folded: String,
    words: Vec<String>,
    exact: BTreeSet<String>,
    weight: i32,
}
impl Term {
    fn new(text: &str, weight: i32) -> Self {
        let folded = fold(text);
        Self {
            words: folded
                .split(' ')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            exact: text
                .to_lowercase()
                .split(' ')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            folded,
            weight,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Emoji {
    pub text: String,
    pub group: usize,
    pub version: f64,
    pub name_en: String,
    pub name_vi: String,
    pub skins: Vec<String>,
    pub popularity: i32,
    terms: Vec<Term>,
}
impl Emoji {
    pub fn with_skin(&self, tone: usize) -> &str {
        if (1..=5).contains(&tone) && self.skins.len() == 5 {
            &self.skins[tone - 1]
        } else {
            &self.text
        }
    }
}
pub struct EmojiIndex {
    pub all: Vec<Emoji>,
}
impl Default for EmojiIndex {
    fn default() -> Self {
        Self::parse(include_str!("../../../App/Resources/Emoji.json"))
            .expect("checked-in emoji data")
    }
}
impl EmojiIndex {
    pub fn parse(source: &str) -> serde_json::Result<Self> {
        let rows: Vec<Vec<Value>> = serde_json::from_str(source)?;
        let mut all = vec![];
        for row in rows {
            if row.len() < 8 {
                continue;
            }
            let (Some(text), Some(group)) = (row[0].as_str(), row[1].as_u64()) else {
                continue;
            };
            let string = |i: usize| row.get(i).and_then(Value::as_str).unwrap_or("");
            let (en, vi) = (string(3), string(4));
            let mut terms = vec![];
            for name in [en, vi] {
                if !name.is_empty() {
                    terms.push(Term::new(name, 3))
                }
            }
            for keywords in [string(5), string(6)] {
                for keyword in keywords.split(" | ").filter(|s| !s.is_empty()) {
                    terms.push(Term::new(keyword, 2))
                }
            }
            for emoticon in string(9).split(' ').filter(|s| !s.is_empty()) {
                terms.push(Term::new(emoticon, 3))
            }
            all.push(Emoji {
                text: text.into(),
                group: group as usize,
                version: row[2].as_f64().unwrap_or(0.0),
                name_en: en.into(),
                name_vi: vi.into(),
                skins: row[7]
                    .as_array()
                    .map(|v| {
                        v.iter()
                            .filter_map(|s| s.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                popularity: row.get(8).and_then(Value::as_i64).unwrap_or(0) as i32,
                terms,
            });
        }
        Ok(Self { all })
    }
    pub fn search(&self, raw: &str, limit: usize, among: Option<&[usize]>) -> Vec<usize> {
        let q = raw.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let converted = convert_telex(&q);
        let mut variants = vec![q.clone()];
        if converted != q {
            variants.push(converted)
        }
        let queries: Vec<Vec<(String, String)>> = variants
            .iter()
            .map(|v| {
                v.split(' ')
                    .filter(|w| !w.is_empty())
                    .map(|w| (w.into(), fold(w)))
                    .collect()
            })
            .collect();
        let mut scored = vec![];
        let list: Vec<usize> = among
            .map(|l| l.to_vec())
            .unwrap_or_else(|| (0..self.all.len()).collect());
        for (order, &i) in list.iter().enumerate() {
            let Some(e) = self.all.get(i) else { continue };
            let best = queries
                .iter()
                .map(|q| score(q, &e.terms))
                .max()
                .unwrap_or(0);
            if best > 0 {
                scored.push((best * 2 + e.popularity, order, i))
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().take(limit).map(|(_, _, i)| i).collect()
    }
}
fn score(words: &[(String, String)], terms: &[Term]) -> i32 {
    if words.is_empty() {
        return 0;
    }
    let whole = words
        .iter()
        .map(|(_, folded)| folded.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut total = 0;
    for (exact, folded) in words {
        let mut best = 0;
        for term in terms {
            let name = term.weight == 3;
            let mut s = if term.folded == whole {
                if name { 100 } else { 65 }
            } else {
                0
            };
            if s == 0 {
                for w in &term.words {
                    if w.starts_with(folded) {
                        let same = w.chars().count() == folded.chars().count();
                        s = s.max(match (name, same) {
                            (true, true) => 70,
                            (false, true) => 55,
                            (true, false) => 45,
                            (false, false) => 30,
                        });
                        if same {
                            break;
                        }
                    }
                }
            }
            if s <= 0 {
                continue;
            }
            if term.exact.contains(exact) {
                s += 10
            } else if exact != folded {
                s -= 40
            }
            if name {
                s += (6 - term.words.len() as i32).max(0)
            }
            best = best.max(s)
        }
        if best <= 0 {
            return 0;
        }
        total += best
    }
    total / words.len() as i32
}
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .map(|c| {
            if c == 'đ' {
                'd'
            } else if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
                char::from_u32(c as u32 - 0xfee0).unwrap()
            } else if c == '\u{3000}' {
                ' '
            } else {
                c
            }
        })
        .collect()
}
const VOWELS: &str = "aăâeêioôơuưy";
pub fn convert_telex(text: &str) -> String {
    text.split(' ')
        .map(telex_word)
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn telex_word(keys: &str) -> String {
    let mut out: Vec<char> = vec![];
    let (mut tone, mut tone_key) = (0, None);
    for k in keys.to_lowercase().chars() {
        let last = out.last().copied();
        match (k, last) {
            ('d', Some('d')) => *out.last_mut().unwrap() = 'đ',
            ('a', Some('a')) => *out.last_mut().unwrap() = 'â',
            ('e', Some('e')) => *out.last_mut().unwrap() = 'ê',
            ('o', Some('o')) => *out.last_mut().unwrap() = 'ô',
            ('w', _) => {
                let at = out.iter().rposition(|c| "aou".contains(*c));
                if let Some(i) = at.filter(|&i| {
                    out[i + 1..].iter().all(|c| !VOWELS.contains(*c)) || i == out.len() - 1
                }) {
                    let c = out[i];
                    out[i] = match c {
                        'a' => 'ă',
                        'o' => 'ơ',
                        _ => 'ư',
                    };
                    if c == 'o' && i > 0 && out[i - 1] == 'u' {
                        out[i - 1] = 'ư'
                    }
                    if c == 'u' && i + 1 < out.len() && out[i + 1] == 'o' {
                        out[i + 1] = 'ơ'
                    }
                } else {
                    out.push('ư')
                }
            }
            ('z', _) => {
                tone = 0;
                tone_key = None
            }
            _ => {
                if let Some(t) = "sfrxj"
                    .chars()
                    .position(|c| c == k)
                    .filter(|_| out.iter().any(|c| VOWELS.contains(*c)))
                {
                    if tone_key == Some(k) {
                        tone = 0;
                        tone_key = None;
                        out.push(k)
                    } else {
                        tone = t + 1;
                        tone_key = Some(k)
                    }
                } else {
                    out.push(k)
                }
            }
        }
    }
    if tone > 0 {
        let start = if out.len() > 2
            && ((out[0] == 'q' && out[1] == 'u')
                || (out[0] == 'g' && out[1] == 'i' && VOWELS.contains(out[2])))
        {
            2
        } else {
            0
        };
        let idx: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(i, c)| *i >= start && VOWELS.contains(**c))
            .map(|(i, _)| i)
            .collect();
        if let Some(&first) = idx.first() {
            let last = *idx.last().unwrap();
            let at = if let Some(&i) = idx.iter().rev().find(|&&i| "ăâêôơư".contains(out[i]))
            {
                i
            } else if idx.len() == 1 {
                first
            } else if last < out.len() - 1 || idx.len() == 3 {
                idx[1]
            } else {
                let pair: String = [out[idx[0]], out[idx[1]]].iter().collect();
                if ["oa", "oe", "uy"].contains(&pair.as_str()) {
                    idx[1]
                } else {
                    idx[0]
                }
            };
            if let Some(group) = VOWELS.chars().position(|c| c == out[at]) {
                let marks = [
                    "áàảãạ",
                    "ắằẳẵặ",
                    "ấầẩẫậ",
                    "éèẻẽẹ",
                    "ếềểễệ",
                    "íìỉĩị",
                    "óòỏõọ",
                    "ốồổỗộ",
                    "ớờởỡợ",
                    "úùủũụ",
                    "ứừửữự",
                    "ýỳỷỹỵ",
                ];
                out[at] = marks[group].chars().nth(tone - 1).unwrap();
            }
        }
    }
    out.into_iter().collect()
}
pub const STARTER: [&str; 30] = [
    "😂", "❤️", "🥰", "😍", "😭", "👍", "🙏", "😊", "🔥", "😘", "🤣", "🥺", "😅", "🎉", "💕", "😁",
    "🙂", "😉", "🤔", "👏", "😎", "💪", "✨", "😢", "🙌", "💯", "😆", "🤗", "😴", "👌",
];
pub fn record_usage(scores: &mut BTreeMap<String, f64>, emoji: &str) {
    for score in scores.values_mut() {
        *score *= 0.97
    }
    *scores.entry(emoji.into()).or_default() += 1.0;
    if scores.len() > 80 {
        let mut list: Vec<_> = scores.iter().map(|(k, v)| (k.clone(), *v)).collect();
        list.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (k, _) in list.into_iter().take(scores.len() - 80) {
            scores.remove(&k);
        }
    }
}
pub fn frequent(scores: &BTreeMap<String, f64>, count: usize) -> Vec<String> {
    let mut list: Vec<_> = scores.iter().collect();
    list.sort_by(|a, b| b.1.total_cmp(a.1).then(a.0.cmp(b.0)));
    let mut out: Vec<String> = list.into_iter().map(|(k, _)| k.clone()).collect();
    for e in STARTER {
        if out.len() >= count {
            break;
        }
        if !out.iter().any(|s| s == e) {
            out.push(e.into())
        }
    }
    out.truncate(count);
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multilingual_search() {
        let index = EmojiIndex::default();
        for q in ["horse", "nguwaj", "ngựa", "ngua"] {
            let results = index.search(q, 10, None);
            assert!(
                results
                    .iter()
                    .any(|&i| index.all[i].text == "🐴" || index.all[i].text == "🐎"),
                "{q}"
            );
        }
        assert_eq!(convert_telex("dduwowngf nguwaj"), "đường ngựa");
    }
    #[test]
    fn frequently_used_decay() {
        let mut scores = BTreeMap::new();
        record_usage(&mut scores, "🐴");
        record_usage(&mut scores, "🐱");
        assert_eq!(frequent(&scores, 2), ["🐱", "🐴"]);
    }
}

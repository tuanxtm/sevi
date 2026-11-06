use crate::tables;
pub const MISSING: i32 = -1000;
pub fn rows(
    table: &'static [(u32, &'static [&'static [u32]])],
    key: u32,
) -> &'static [&'static [u32]] {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(&[])
}
pub fn unicode(key: u32) -> Option<&'static [u32]> {
    tables::UNICODE
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
pub fn key_character(key: u32) -> u32 {
    tables::CHARACTERS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(0)
}
pub struct Dictionary {
    pub words: &'static [u8],
    pub offsets: &'static [u8],
    pub scores: &'static [u8],
}
impl Dictionary {
    pub fn len(&self) -> usize {
        self.scores.len()
    }
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }
    pub fn word(&self, i: usize) -> &'static [u8] {
        let off = u32::from_le_bytes(self.offsets[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
        let rest = &self.words[off..];
        &rest[..rest.iter().position(|b| *b == 0).unwrap()]
    }
    pub fn index(&self, word: &[u8]) -> Result<usize, usize> {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.word(mid).cmp(word) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }
        Err(lo)
    }
    pub fn score(&self, word: &[u8]) -> i32 {
        self.index(word)
            .map(|i| self.scores[i] as i8 as i32)
            .unwrap_or(MISSING)
    }
}
pub static VN: Dictionary = Dictionary {
    words: include_bytes!("data/vn.words"),
    offsets: include_bytes!("data/vn.offsets"),
    scores: include_bytes!("data/vn.scores"),
};
pub static EN: Dictionary = Dictionary {
    words: include_bytes!("data/en.words"),
    offsets: include_bytes!("data/en.offsets"),
    scores: include_bytes!("data/en.scores"),
};
pub static BIGRAM: Dictionary = Dictionary {
    words: include_bytes!("data/bigram.words"),
    offsets: include_bytes!("data/bigram.offsets"),
    scores: include_bytes!("data/bigram.scores"),
};

pub fn next_key_mass(vietnamese: bool, prefix: &str) -> [f64; 27] {
    static VN_SUM: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    static EN_SUM: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    let (dict, cache) = if vietnamese {
        (&VN, &VN_SUM)
    } else {
        (&EN, &EN_SUM)
    };
    let cumulative = cache.get_or_init(|| {
        let mut out = Vec::with_capacity(dict.len() + 1);
        out.push(0.0);
        for &s in dict.scores {
            out.push(out.last().unwrap() + 10f64.powf(s as i8 as f64 / 10.0))
        }
        out
    });
    let mut out = [0.0; 27];
    if prefix.len() >= 38 {
        return out;
    }
    for (i, value) in out[..26].iter_mut().enumerate() {
        let mut key = prefix.as_bytes().to_vec();
        key.push(b'a' + i as u8);
        let lo = dict.index(&key).unwrap_or_else(|i| i);
        key.push(127);
        let hi = dict.index(&key).unwrap_or_else(|i| i);
        *value = cumulative[hi] - cumulative[lo]
    }
    let s = dict.score(prefix.as_bytes());
    if s != MISSING {
        out[26] = 10f64.powf(s as f64 / 10.0)
    }
    out
}
pub fn completions(vietnamese: bool, prefix: &str, max: usize) -> Vec<(String, i32)> {
    if prefix.is_empty() || prefix.len() >= 30 || max == 0 {
        return vec![];
    }
    let dict = if vietnamese { &VN } else { &EN };
    let lo = dict.index(prefix.as_bytes()).unwrap_or_else(|i| i);
    let mut out = vec![];
    for i in lo..dict.len() {
        let word = dict.word(i);
        if !word.starts_with(prefix.as_bytes()) {
            break;
        }
        if word.len() >= 32 {
            continue;
        }
        let s = dict.scores[i] as i8 as i32;
        if out.len() == max && s <= out.last().map(|p: &(String, i32)| p.1).unwrap_or(MISSING) {
            continue;
        }
        let pos = out.iter().position(|p| p.1 < s).unwrap_or(out.len());
        out.insert(pos, (String::from_utf8_lossy(word).into_owned(), s));
        out.truncate(max)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_tables_preserve_sorted_word_index() {
        for (dict, count) in [(&VN, 8143), (&EN, 41225), (&BIGRAM, 160386)] {
            assert_eq!(dict.len(), count);
            assert_eq!(dict.offsets.len(), count * 4);
            for i in 0..count {
                let w = dict.word(i);
                assert!(!w.is_empty());
                assert_eq!(dict.index(w), Ok(i));
                if i > 0 {
                    assert!(dict.word(i - 1) < w)
                }
            }
        }
    }
}

use crate::rng::Mt19937;
use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;
static TOKENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\W\d_]+|[^\w]|\d|_").unwrap());
pub type DecomposedWord = (Vec<String>, Vec<bool>, Option<(char, usize)>);
pub fn decompose(word: &str) -> Option<DecomposedWord> {
    let mut keys = vec![];
    let mut vowels = vec![];
    let mut tone = None;
    for ch in word.chars() {
        let decomposed: Vec<char> = ch.to_string().nfd().collect();
        let base = *decomposed.first()?;
        if base == 'đ' || base == 'Đ' {
            keys.push(if base == 'đ' { "dd" } else { "Dd" }.to_owned());
            vowels.push(false);
            continue;
        }
        let mut k = base.to_string();
        let low = base.to_lowercase().next()?;
        for m in &decomposed[1..] {
            if let Some(i) = "\u{301}\u{300}\u{309}\u{303}\u{323}"
                .chars()
                .position(|c| c == *m)
            {
                tone = Some((b"sfrxj"[i] as char, keys.len()))
            } else if *m == '\u{302}' {
                k.push(low)
            } else if *m == '\u{306}' || *m == '\u{31b}' {
                k.push('w')
            } else {
                return None;
            }
        }
        keys.push(k);
        vowels.push("aeiouy".contains(low))
    }
    Some((keys, vowels, tone))
}
pub fn telex(word: &str, rng: &mut Mt19937) -> Option<String> {
    let (mut keys, vowels, tone) = decompose(word)?;
    if keys.concat().to_lowercase().contains("uwow") && rng.random() < 0.5 {
        for i in 0..keys.len().saturating_sub(1) {
            if keys[i].eq_ignore_ascii_case("uw") && keys[i + 1].eq_ignore_ascii_case("ow") {
                keys[i].truncate(1)
            }
        }
    }
    if let Some((tone, at)) = tone {
        let r = rng.random();
        if r < 0.6 {
            keys.push(tone.to_string())
        } else if r < 0.85 {
            keys[at].push(tone)
        } else {
            let last = vowels.iter().rposition(|v| *v)?;
            keys[last].push(tone)
        }
    }
    let mut output = keys.concat();
    if word.chars().count() > 1
        && word.chars().any(char::is_uppercase)
        && word.chars().all(|c| !c.is_lowercase())
    {
        output = output.to_uppercase()
    }
    Some(output)
}
fn typo(keys: &str, rng: &mut Mt19937) -> String {
    let mut bytes = keys.as_bytes().to_vec();
    if bytes.is_empty() {
        return keys.into();
    }
    let i = rng.below(bytes.len());
    let c = bytes[i];
    if !c.is_ascii_alphabetic() {
        return keys.into();
    }
    let r = rng.random();
    if r < 0.6 {
        let mut neighbours = vec![];
        for row in [b"qwertyuiop".as_slice(), b"asdfghjkl", b"zxcvbnm"] {
            if let Some(at) = row.iter().position(|b| *b == c.to_ascii_lowercase()) {
                if at > 0 {
                    neighbours.push(row[at - 1])
                }
                if at + 1 < row.len() {
                    neighbours.push(row[at + 1])
                }
                break;
            }
        }
        if neighbours.is_empty() {
            return keys.into();
        }
        let mut replacement = neighbours[rng.below(neighbours.len())];
        if c.is_ascii_uppercase() {
            replacement = replacement.to_ascii_uppercase()
        }
        bytes[i] = replacement
    } else if r < 0.8 && bytes.len() > 2 {
        bytes.remove(i);
    } else {
        bytes.insert(i, c)
    }
    String::from_utf8(bytes).unwrap()
}
pub fn prepare(contents: &[String], rate: f64) -> String {
    let mut rng = Mt19937::python(42);
    let mut out = String::new();
    for source in contents {
        for line in source.lines() {
            let line: String = line.trim().nfc().collect();
            if line.is_empty() {
                continue;
            }
            for tok in TOKENS.find_iter(&line).map(|m| m.as_str()) {
                if tok.chars().next().unwrap().is_alphabetic() {
                    if !tok.is_ascii() {
                        let Some(mut k) = telex(tok, &mut rng).filter(|s| s.is_ascii()) else {
                            out.push_str("S\t \n");
                            continue;
                        };
                        if rate != 0.0 && rng.random() < rate {
                            k = typo(&k, &mut rng)
                        }
                        out.push_str(&format!("W\t{k}\t1\n"))
                    } else {
                        out.push_str(&format!("W\t{tok}\t0\n"))
                    }
                } else {
                    out.push_str("S\t");
                    out.push(
                        if tok.len() == 1 && " ,.;:?!'\"()-/0123456789".contains(tok) {
                            tok.chars().next().unwrap()
                        } else {
                            ' '
                        },
                    );
                    out.push('\n')
                }
            }
            out.push_str("N\n")
        }
    }
    out
}

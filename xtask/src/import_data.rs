//! Reproducible transfer of the frozen legacy tables; never retrains or downloads data.
use crate::{Result, root};
use std::{collections::BTreeMap, fs};
#[derive(Debug, Clone)]
enum Token {
    Number(u32),
    Name(String),
    Bytes(Vec<u8>),
    Char(u32),
    Symbol(char),
}
#[derive(Debug, Clone)]
enum Node {
    Number(u32),
    List(Vec<Node>),
    Bytes(Vec<u8>),
}
fn quoted(bytes: &[u8], at: &mut usize, quote: u8) -> Result<Vec<u8>> {
    *at += 1;
    let mut out = vec![];
    while *at < bytes.len() {
        let c = bytes[*at];
        *at += 1;
        if c == quote {
            return Ok(out);
        }
        if c != b'\\' {
            out.push(c);
            continue;
        }
        let c = *bytes.get(*at).ok_or("unterminated escape")?;
        *at += 1;
        match c {
            b'0'..=b'7' => {
                let mut value = (c - b'0') as u32;
                for _ in 0..2 {
                    if let Some(d @ b'0'..=b'7') = bytes.get(*at) {
                        value = value * 8 + (d - b'0') as u32;
                        *at += 1
                    } else {
                        break;
                    }
                }
                out.push(u8::try_from(value)?);
            }
            b'n' => out.push(10),
            b'r' => out.push(13),
            b't' => out.push(9),
            b'\\' | b'\'' | b'"' => out.push(c),
            _ => return Err(format!("unsupported C escape: {}", c as char).into()),
        }
    }
    Err("unterminated C literal".into())
}
fn lex(source: &str) -> Result<Vec<Token>> {
    let bytes = source.as_bytes();
    let mut at = 0;
    let mut out = vec![];
    while at < bytes.len() {
        let c = bytes[at];
        if c.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if bytes.get(at..at + 2) == Some(b"//") {
            while at < bytes.len() && bytes[at] != 10 {
                at += 1
            }
            continue;
        }
        if bytes.get(at..at + 2) == Some(b"/*") {
            at += 2;
            while at + 1 < bytes.len() && &bytes[at..at + 2] != b"*/" {
                at += 1
            }
            at += 2;
            continue;
        }
        if c == b';' {
            break;
        }
        if c == b'"' || c == b'\'' {
            let data = quoted(bytes, &mut at, c)?;
            out.push(if c == b'\'' {
                if data.len() != 1 {
                    return Err("non-byte character literal".into());
                }
                Token::Char(data[0] as u32)
            } else {
                Token::Bytes(data)
            });
            continue;
        }
        if c.is_ascii_digit() {
            let start = at;
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_hexdigit() {
                at += 1
            }
            if bytes.get(at) == Some(&b'x') && at == start + 1 {
                at += 1;
                while at < bytes.len() && bytes[at].is_ascii_hexdigit() {
                    at += 1
                }
            }
            let text = std::str::from_utf8(&bytes[start..at])?;
            let number = if let Some(hex) = text.strip_prefix("0x") {
                u32::from_str_radix(hex, 16)?
            } else {
                text.parse()?
            };
            out.push(Token::Number(number));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = at;
            at += 1;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1
            }
            out.push(Token::Name(std::str::from_utf8(&bytes[start..at])?.into()));
            continue;
        }
        out.push(Token::Symbol(c as char));
        at += 1
    }
    Ok(out)
}
fn initializer(source: &str, name: &str) -> Result<Vec<Token>> {
    let re = regex::Regex::new(&format!(r"\b{}\b[^=;]*=", regex::escape(name)))?;
    let matched = re
        .find(source)
        .ok_or_else(|| format!("missing table {name}"))?;
    lex(&source[matched.end()..])
}
fn number(tokens: &[Token], at: &mut usize, constants: &BTreeMap<String, u32>) -> Result<u32> {
    let negative = matches!(tokens.get(*at), Some(Token::Symbol('-')));
    if negative {
        *at += 1
    }
    let mut value = match tokens.get(*at) {
        Some(Token::Number(n) | Token::Char(n)) => *n,
        Some(Token::Name(n)) => *constants
            .get(n)
            .ok_or_else(|| format!("unknown constant {n}"))?,
        t => return Err(format!("expected number, got {t:?}").into()),
    };
    *at += 1;
    if negative {
        value = 0u32.wrapping_sub(value)
    }
    while matches!(tokens.get(*at), Some(Token::Symbol('|'))) {
        *at += 1;
        value |= number(tokens, at, constants)?
    }
    Ok(value)
}
fn node(tokens: &[Token], at: &mut usize, constants: &BTreeMap<String, u32>) -> Result<Node> {
    if matches!(tokens.get(*at), Some(Token::Symbol('{'))) {
        *at += 1;
        let mut values = vec![];
        while !matches!(tokens.get(*at), Some(Token::Symbol('}'))) {
            values.push(node(tokens, at, constants)?);
            if matches!(tokens.get(*at), Some(Token::Symbol(','))) {
                *at += 1
            } else if !matches!(tokens.get(*at), Some(Token::Symbol('}'))) {
                return Err("expected initializer separator".into());
            }
        }
        *at += 1;
        Ok(Node::List(values))
    } else if let Some(Token::Bytes(value)) = tokens.get(*at) {
        *at += 1;
        Ok(Node::Bytes(value.clone()))
    } else {
        Ok(Node::Number(number(tokens, at, constants)?))
    }
}
fn table(source: &str, name: &str, constants: &BTreeMap<String, u32>) -> Result<Node> {
    let tokens = initializer(source, name)?;
    node(&tokens, &mut 0, constants)
}
fn list(n: &Node) -> Result<&[Node]> {
    match n {
        Node::List(v) => Ok(v),
        _ => Err("expected table list".into()),
    }
}
fn value(n: &Node) -> Result<u32> {
    match n {
        Node::Number(v) => Ok(*v),
        _ => Err("expected numeric table entry".into()),
    }
}
fn literal(n: &Node) -> Result<String> {
    Ok(match n {
        Node::Number(v) => v.to_string(),
        Node::List(v) => format!(
            "&[{}]",
            v.iter().map(literal).collect::<Result<Vec<_>>>()?.join(",")
        ),
        Node::Bytes(v) => format!("{:?}", std::str::from_utf8(v)?),
    })
}
fn numeric(source: &str, name: &str) -> Result<Vec<u32>> {
    list(&table(source, name, &BTreeMap::new())?)?
        .iter()
        .map(value)
        .collect()
}
fn strings(source: &str, name: &str) -> Result<Vec<Vec<u8>>> {
    list(&table(source, name, &BTreeMap::new())?)?
        .iter()
        .map(|n| match n {
            Node::Bytes(v) => Ok(v.clone()),
            _ => Err("expected byte string".into()),
        })
        .collect()
}
pub fn run(check: bool) -> Result {
    let engine = root().join("Engine");
    let dest = root().join("crates/sevi-engine/src");
    let mut files = BTreeMap::<String, Vec<u8>>::new();
    let mut constants = BTreeMap::new();
    let mut key_source = String::from("// Canonical key codes and flags inherited from OpenKey.\n");
    let defs = regex::Regex::new(r"(?m)^#define\s+(KEY_\w+|\w+_MASK)\s+(0x[\da-fA-F]+|\d+)\s*$")?;
    for file in ["platforms/mac.h", "DataType.h"] {
        let source = fs::read_to_string(engine.join(file))?;
        for c in defs.captures_iter(&source) {
            let n = if let Some(hex) = c[2].strip_prefix("0x") {
                u32::from_str_radix(hex, 16)?
            } else {
                c[2].parse()?
            };
            constants.insert(c[1].into(), n);
            key_source.push_str(&format!("pub const {}: u32 = {};\n", &c[1], &c[2]));
        }
    }
    key_source.push_str("pub const MAX_BUFF: usize = 32;\npub const LETTER_KEYS: [u32;26] = [KEY_A,KEY_B,KEY_C,KEY_D,KEY_E,KEY_F,KEY_G,KEY_H,KEY_I,KEY_J,KEY_K,KEY_L,KEY_M,KEY_N,KEY_O,KEY_P,KEY_Q,KEY_R,KEY_S,KEY_T,KEY_U,KEY_V,KEY_W,KEY_X,KEY_Y,KEY_Z];\n");
    files.insert("keys.rs".into(), key_source.into_bytes());
    let source = fs::read_to_string(engine.join("Vietnamese.cpp"))?;
    let mut output = String::from(
        "// Generated verbatim from the existing OpenKey/Sevi tables. See ACKNOWLEDGEMENTS.md.\n",
    );
    for (name, rust_name) in [
        ("_vowel", "VOWEL"),
        ("_vowelCombine", "VOWEL_COMBINE"),
        ("_vowelForMark", "VOWEL_FOR_MARK"),
    ] {
        let data = table(&source, name, &constants)?;
        let mut rows = list(&data)?.to_vec();
        rows.sort_by_key(|n| value(&list(n).unwrap()[0]).unwrap());
        output.push_str(&format!("pub const {rust_name}: &[(u32, &[&[u32]])] = &["));
        for row in &rows {
            let pair = list(row)?;
            output.push_str(&format!("({},{}),", value(&pair[0])?, literal(&pair[1])?))
        }
        output.push_str("];\n");
    }
    for (name, rust_name) in [
        ("_consonantD", "CONSONANT_D"),
        ("_consonantTable", "CONSONANTS"),
        ("_endConsonantTable", "END_CONSONANTS"),
        ("_doubleWAllowed", "DOUBLE_W"),
    ] {
        output.push_str(&format!(
            "pub const {rust_name}: &[&[u32]] = {};\n",
            literal(&table(&source, name, &constants)?)?
        ));
    }
    output.push_str(&format!(
        "pub const STANDALONE_W_BAD: &[u32] = {};\n",
        literal(&table(&source, "_standaloneWbad", &constants)?)?
    ));
    let data = table(&source, "_unicodeTable", &constants)?;
    let mut rows = list(&data)?.to_vec();
    rows.sort_by_key(|n| value(&list(n).unwrap()[0]).unwrap());
    output.push_str("pub const UNICODE: &[(u32, &[u32])] = &[");
    for row in &rows {
        let p = list(row)?;
        output.push_str(&format!("({},{}),", value(&p[0])?, literal(&p[1])?))
    }
    output.push_str("];\n");
    let mut characters = BTreeMap::new();
    for row in list(&table(&source, "_characterMap", &constants)?)? {
        let p = list(row)?;
        characters.insert(value(&p[1])?, value(&p[0])?);
    }
    output.push_str("pub const CHARACTERS: &[(u32,u32)] = &[");
    for key in 0..256 {
        for caps in [0, constants["CAPS_MASK"]] {
            if let Some(ch) = characters.get(&(key | caps)) {
                output.push_str(&format!("({},{}),", key | caps, ch))
            }
        }
    }
    output.push_str("];\n");
    let teen = fs::read_to_string(engine.join("TeencodeData.h"))?;
    let words = strings(&teen, "kTeencodeWords")?;
    output.push_str("pub const TEENCODE: &[&str] = &[");
    for w in words {
        output.push_str(&format!("{:?},", std::str::from_utf8(&w)?))
    }
    output.push_str("];\n");
    let words = strings(&teen, "kTeencodeVnKeys")?;
    let scores = numeric(&teen, "kTeencodeVnScores")?;
    if words.len() != scores.len() {
        return Err("teencode key/score mismatch".into());
    }
    output.push_str("pub const TEENCODE_VN: &[(&str,i32)] = &[");
    for (w, s) in words.iter().zip(scores) {
        output.push_str(&format!("({:?},{}),", std::str::from_utf8(w)?, s as i32))
    }
    output.push_str("];\n");
    files.insert("tables.rs".into(), output.into_bytes());
    for (header, prefix, words, offsets, scores) in [
        (
            "AutoSwitchData.h",
            "vn",
            "kAutoSwitchVnWords",
            "kAutoSwitchVnOffsets",
            "kAutoSwitchVnScores",
        ),
        (
            "AutoSwitchData.h",
            "en",
            "kAutoSwitchEnWords",
            "kAutoSwitchEnOffsets",
            "kAutoSwitchEnScores",
        ),
        (
            "BigramData.h",
            "bigram",
            "kBigramKeys",
            "kBigramOffsets",
            "kBigramLogCounts",
        ),
    ] {
        let source = fs::read_to_string(engine.join(header))?;
        let mut bytes = vec![];
        for token in initializer(&source, words)? {
            match token {
                Token::Bytes(v) => bytes.extend(v),
                _ => return Err("unexpected word-blob token".into()),
            }
        }
        bytes.push(0);
        let offsets = numeric(&source, offsets)?;
        let scores = numeric(&source, scores)?;
        if offsets.len() != scores.len() {
            return Err("dictionary offset/score mismatch".into());
        }
        files.insert(format!("data/{prefix}.words"), bytes);
        files.insert(
            format!("data/{prefix}.offsets"),
            offsets.into_iter().flat_map(u32::to_le_bytes).collect(),
        );
        files.insert(
            format!("data/{prefix}.scores"),
            scores.into_iter().map(|v| v as u8).collect(),
        );
    }
    let keep = fs::read_to_string(engine.join("KeepWords.inc"))?;
    let words = strings(&keep, "kKeepWords")?;
    let mut bytes = vec![];
    for w in words {
        bytes.extend(w);
        bytes.push(10)
    }
    files.insert("data/keep.words".into(), bytes);
    let trailing_comma = regex::Regex::new(r",\s*([\]\)])")?;
    for (name, bytes) in files {
        let path = dest.join(&name);
        if check {
            let current = fs::read(&path)?;
            let equal = if name.ends_with(".rs") {
                let normalize = |b: &[u8]| {
                    trailing_comma
                        .replace_all(&String::from_utf8_lossy(b), "$1")
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                };
                normalize(&current) == normalize(&bytes)
            } else {
                current == bytes
            };
            if !equal {
                let artifact = root().join("target/migration/import");
                fs::create_dir_all(&artifact)?;
                fs::write(artifact.join(name.replace('/', "_")), &bytes)?;
                return Err(format!("legacy data mismatch: {}", path.display()).into());
            }
            println!("verified {name}");
        } else {
            fs::write(path, bytes)?;
            println!("imported {name}");
        }
    }
    Ok(())
}

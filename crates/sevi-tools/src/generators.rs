use crate::{Args, Result, data_files, dictionary::score};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use unicode_normalization::UnicodeNormalization;
pub fn keep(a: &Args<'_>) -> Result {
    let source = a
        .args
        .first()
        .filter(|s| !s.starts_with('-'))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| a.data().join("keep/keep_3.txt"));
    let mut words = BTreeSet::new();
    for w in fs::read_to_string(source)?.lines().map(str::trim) {
        if w.len() >= 3 && w.bytes().all(|c| c.is_ascii_alphabetic()) {
            words.insert(w.to_owned());
        }
    }
    let directory = data_files::directory(a);
    fs::create_dir_all(&directory)?;
    let path = directory.join("keep.words");
    let out = words
        .iter()
        .map(|word| format!("{word}\n"))
        .collect::<String>();
    fs::write(&path, out)?;
    println!("{} words -> {}", words.len(), path.display());
    Ok(())
}
pub fn bigrams(a: &Args<'_>) -> Result {
    let data = a.data();
    let mut ties = BTreeSet::new();
    for line in fs::read_to_string(data.join("bigram/tie_forms.tsv"))?.lines() {
        let (a, b) = line.split_once('\t').ok_or("invalid tie form")?;
        ties.insert(a.to_owned());
        ties.insert(b.nfc().collect());
    }
    let min: usize = std::env::var("SEVI_BIGRAM_MIN")
        .unwrap_or_else(|_| "10".into())
        .parse()?;
    let next_min: usize = std::env::var("SEVI_BIGRAM_NEXT_MIN")
        .unwrap_or_else(|_| "20".into())
        .parse()?;
    let mut counts = BTreeMap::<(String, String), usize>::new();
    for file in [
        "bulk/merged/bigram_vi.tsv",
        "bulk/merged/bigram_en.tsv",
        "bigram/crawl_vi.tsv",
    ] {
        for line in fs::read_to_string(data.join(file))?.lines() {
            let p: Vec<_> = line.split('\t').collect();
            if p.len() == 3 && (ties.contains(p[0]) || ties.contains(p[1])) {
                *counts.entry((p[0].into(), p[1].into())).or_default() += p[2].parse::<usize>()?;
            }
        }
    }
    let pairs: Vec<_> = counts
        .into_iter()
        .filter(|((a, b), v)| {
            (ties.contains(b) && *v >= min) || (ties.contains(a) && *v >= next_min)
        })
        .map(|((a, b), v)| (format!("{a}\t{b}"), v))
        .collect();
    let directory = data_files::directory(a);
    data_files::write_dictionary(
        &directory,
        "bigram",
        pairs.iter().map(|(word, count)| {
            (
                word.as_str(),
                (8. * (*count as f64).log2()).round_ties_even().min(255.) as u8,
            )
        }),
    )?;
    println!("{} pairs -> {}", pairs.len(), directory.display());
    Ok(())
}
fn canon(w: &str) -> String {
    let mut out = String::new();
    let mut tone = None;
    for c in w.chars() {
        if c == 'đ' {
            out.push_str("d^");
            continue;
        }
        let mut d = c.to_string().nfd().collect::<Vec<_>>().into_iter();
        if let Some(c) = d.next() {
            out.push(c)
        }
        for m in d {
            match m {
                '\u{302}' => out.push('^'),
                '\u{306}' | '\u{31b}' => out.push('+'),
                '\u{301}' => tone = Some('1'),
                '\u{300}' => tone = Some('2'),
                '\u{309}' => tone = Some('3'),
                '\u{303}' => tone = Some('4'),
                '\u{323}' => tone = Some('5'),
                _ => (),
            }
        }
    }
    if let Some(t) = tone {
        out.push(t)
    }
    out
}
fn corpus_files(path: &Path) -> Result<Vec<std::path::PathBuf>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut files = fs::read_dir(path)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|p| p.extension().is_some_and(|s| s == "txt") && p.is_file());
    files.sort();
    Ok(files)
}
pub fn teencode(a: &Args<'_>) -> Result {
    let mut words = BTreeSet::new();
    let vis = a.path("--visolex", a.data().join("visolex/dictionary.json"));
    if vis.exists() {
        let data: serde_json::Value = serde_json::from_slice(&fs::read(vis)?)?;
        if let Some(map) = data.as_object() {
            words.extend(
                map.keys()
                    .map(|k| k.to_lowercase().nfc().collect::<String>()),
            );
        }
    }
    let letters = regex::Regex::new(r"[\p{L}\p{Nl}\p{No}]+")?;
    let mut counts = BTreeMap::<String, usize>::new();
    let real = a.data().join("real");
    for path in corpus_files(&real)?
        .into_iter()
        .chain(corpus_files(&real.join("fresh"))?)
    {
        let text = fs::read(path)?
            .utf8_chunks()
            .map(|c| c.valid())
            .collect::<String>()
            .to_lowercase();
        for m in letters.find_iter(&text) {
            *counts.entry(m.as_str().nfc().collect()).or_default() += 1;
        }
    }
    words.extend(
        counts
            .iter()
            .filter(|(_, c)| **c >= 2)
            .map(|(w, _)| w.clone()),
    );
    let named: BTreeSet<_> = [
        "zui", "zẻ", "zô", "zậy", "dzô", "bíe", "thík", "gòy", "ừk", "mịe", "bik", "mik", "thik",
        "zìa",
    ]
    .into_iter()
    .collect();
    let pattern = regex::Regex::new(r"^(z|dz)|òy$|[iìíỉĩị]e$|[iíìịỉĩ]k$|[ưừứửữự]k$")?;
    let vi = regex::Regex::new(
        r"^[a-zàáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]+$",
    )?;
    let mark = regex::Regex::new(
        r"[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]",
    )?;
    let z = regex::Regex::new(r"^(z|dz)[aeiouy]")?;
    let ok = |w: &str| {
        let chars: Vec<_> = w.chars().collect();
        vi.is_match(w)
            && pattern.is_match(w)
            && !chars.windows(3).any(|s| s[0] == s[1] && s[1] == s[2])
            && chars.len() <= 6
            && (mark.is_match(w) || (z.is_match(w) && chars.len() <= 4))
    };
    let mut keep: BTreeSet<String> = words.iter().filter(|w| ok(w)).map(|w| canon(w)).collect();
    keep.extend(named.iter().map(|w| canon(w)));
    let total = counts.values().sum::<usize>().max(1) as f64;
    let mut scored = BTreeMap::<String, i32>::new();
    for w in words {
        if ((ok(&w) && mark.is_match(&w)) || named.contains(w.as_str()))
            && let Some(c) = counts.get(&w).filter(|c| **c >= 2)
        {
            let k = teencode_key(&w);
            let value = score(*c as f64, total);
            scored
                .entry(k)
                .and_modify(|v| *v = (*v).max(value))
                .or_insert(value);
        }
    }
    let out = render_teencode(&keep, &scored);
    let directory = data_files::directory(a);
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("teencode.rs"), out)?;
    println!("{} teencode words, {} scores", keep.len(), scored.len());
    Ok(())
}
fn render_teencode(words: &BTreeSet<String>, scores: &BTreeMap<String, i32>) -> String {
    let mut out =
        String::from("// Generated by cargo xtask tool build-teencode. See ACKNOWLEDGEMENTS.md.\n");
    out.push_str("pub const TEENCODE: &[&str] = &[\n");
    for word in words {
        out.push_str(&format!("    {word:?},\n"));
    }
    out.push_str("];\npub const TEENCODE_VN: &[(&str, i32)] = &[\n");
    for (word, score) in scores {
        out.push_str(&format!("    ({word:?}, {score}),\n"));
    }
    out.push_str("];\n");
    out
}
fn teencode_key(w: &str) -> String {
    let mut out = String::new();
    let mut tone = None;
    for c in w.chars() {
        if c == 'đ' {
            out.push_str("dd");
            continue;
        }
        let units: Vec<_> = c.to_string().nfd().collect();
        out.push(units[0]);
        for m in &units[1..] {
            match m {
                '\u{302}' => out.push(units[0]),
                '\u{306}' | '\u{31b}' => out.push('w'),
                '\u{301}' => tone = Some('s'),
                '\u{300}' => tone = Some('f'),
                '\u{309}' => tone = Some('r'),
                '\u{303}' => tone = Some('x'),
                '\u{323}' => tone = Some('j'),
                _ => (),
            }
        }
    }
    if let Some(t) = tone {
        out.push(t)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generators_write_native_data_with_utf8_signed_scores_and_protected_words() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let scratch = root.join("target/generator-tests").join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let data = scratch.join("inputs");
        let output = scratch.join("output");
        for directory in ["bigram", "bulk/merged", "real"] {
            fs::create_dir_all(data.join(directory)).unwrap();
        }
        fs::write(data.join("bigram/tie_forms.tsv"), "bus\tbú\n").unwrap();
        fs::write(data.join("bulk/merged/bigram_vi.tsv"), "a\tbú\t16\n").unwrap();
        fs::write(
            data.join("bulk/merged/bigram_en.tsv"),
            "a\tbus\t32\nbus\tstop\t32\n",
        )
        .unwrap();
        fs::write(data.join("bigram/crawl_vi.tsv"), "a\tbú\t16\n").unwrap();
        fs::write(
            data.join("real/chat.txt"),
            "zui zui zui mịe mịe đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi đi",
        )
        .unwrap();
        let arguments = vec![
            "--data".into(),
            data.to_str().unwrap().into(),
            "--output".into(),
            output.to_str().unwrap().into(),
        ];
        let args = Args {
            root,
            args: &arguments,
        };
        bigrams(&args).unwrap();
        assert_eq!(
            fs::read(output.join("bigram.words")).unwrap(),
            "a\tbus\0a\tbú\0bus\tstop\0\0".as_bytes()
        );
        assert_eq!(
            fs::read(output.join("bigram.scores")).unwrap(),
            [40, 40, 40]
        );
        teencode(&args).unwrap();
        let generated = fs::read_to_string(output.join("teencode.rs")).unwrap();
        assert!(generated.contains("pub const TEENCODE: &[&str]"));
        assert!(generated.contains("\"mie5\""));
        assert!(generated.contains("(\"miej\","));
        assert!(generated.contains("pub const TEENCODE_VN: &[(&str, i32)]"));
        let source = scratch.join("protected.txt");
        fs::write(&source, "zui\nABC\nfoo\nfoo\nx\n123\n").unwrap();
        let mut keep_args = vec![source.to_str().unwrap().into()];
        keep_args.extend(arguments);
        keep(&Args {
            root,
            args: &keep_args,
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(output.join("keep.words")).unwrap(),
            "ABC\nfoo\nzui\n"
        );
        assert!(!output.join("Engine").exists());
    }
}

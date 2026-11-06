use crate::{Args, Result, counts, dictionary::vn_key};
use sevi_engine::Language;
use sevi_test_tools::{Harness, config_from_env};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
fn telex(w: &str) -> String {
    let forms = [
        ("a", "aàáảãạ"),
        ("aw", "ăằắẳẵặ"),
        ("aa", "âầấẩẫậ"),
        ("e", "eèéẻẽẹ"),
        ("ee", "êềếểễệ"),
        ("i", "iìíỉĩị"),
        ("o", "oòóỏõọ"),
        ("oo", "ôồốổỗộ"),
        ("ow", "ơờớởỡợ"),
        ("u", "uùúủũụ"),
        ("uw", "ưừứửữự"),
        ("y", "yỳýỷỹỵ"),
    ];
    let (mut out, mut tone) = (String::new(), "");
    for c in w.chars() {
        let lower = c.to_lowercase().collect::<String>();
        if lower == "đ" {
            out.push_str(if c == 'đ' { "dd" } else { "DD" });
            continue;
        }
        let found = forms.iter().find_map(|(key, letters)| {
            letters
                .chars()
                .position(|x| x.to_string() == lower)
                .map(|i| (*key, i))
        });
        if let Some((key, i)) = found {
            if i != 0 {
                tone = ["", "f", "s", "r", "x", "j"][i];
            }
            if c.to_string() != lower {
                out.push_str(&key[..1].to_ascii_uppercase());
                out.push_str(&key[1..]);
            } else {
                out.push_str(key)
            }
        } else {
            out.push(c)
        }
    }
    out.push_str(tone);
    out
}
pub fn teen(a: &Args<'_>) -> Result {
    let path = a.path(
        "--source",
        a.root.join("Tools/autoswitch/teen/teen_viettelex.txt"),
    );
    let text = fs::read_to_string(path)?;
    let mut h = Harness::new(config_from_env());
    let (mut good_lines, mut good_words, mut words, mut lines) = (0, 0, 0, 0);
    let tokens = regex::Regex::new(r"\s+|[,.?!]|[^\s,.?!]+")?;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let want = line.replace("/e", "");
        let mut keys = tokens
            .find_iter(line)
            .map(|m| {
                let w = m.as_str();
                w.strip_suffix("/e")
                    .map(str::to_owned)
                    .unwrap_or_else(|| telex(w))
            })
            .collect::<String>();
        keys.push(' ');
        let got = if let Some(path) = a.option("--harness") {
            crate::quality::output(std::process::Command::new(path).args(["1", &keys]))?
                .split("\nswitches=")
                .next()
                .unwrap_or("")
                .trim_end_matches(' ')
                .to_owned()
        } else {
            h.reset(Language::Vietnamese);
            for c in keys.chars() {
                h.press(c)
            }
            h.text().trim_end_matches(' ').to_owned()
        };
        let ws: Vec<_> = want.split_whitespace().collect();
        let gs: Vec<_> = got.split_whitespace().collect();
        let good = if ws.len() == gs.len() {
            ws.iter().zip(gs).filter(|(a, b)| **a == *b).count()
        } else {
            0
        };
        good_words += good;
        words += ws.len();
        lines += 1;
        good_lines += usize::from(got == want);
        if a.args.iter().any(|s| s == "-v") && got != want {
            println!("  {keys:?}\n     got  {got}\n     want {want}");
        }
    }
    println!(
        "teencode (VietTelex teen lines): lines right {good_lines}/{lines}, words right {good_words}/{words} ({:.1}%)",
        100. * good_words as f64 / words as f64
    );
    Ok(())
}
pub fn margins(a: &Args<'_>) -> Result {
    let source = a.path("--source", a.root.join("Tools/autoswitch"));
    let vi = counts(&source.join("vi_50k.txt"))?;
    let en = counts(&source.join("en_50k.txt"))?;
    let vt = vi.iter().map(|(_, n)| n).sum::<f64>();
    let et = en.iter().map(|(_, n)| n).sum::<f64>();
    let syllable = regex::Regex::new(
        r"^(ngh|ch|gh|gi|kh|ng|nh|ph|qu|th|tr|b|c|dd|d|g|h|k|l|m|n|p|r|s|t|v|x)?(a|aw|aa|e|ee|i|o|oo|ow|u|uw|y)+(ch|ng|nh|c|m|n|p|t)?[sfrxj]?$",
    )?;
    let mut vn = Vec::<(String, f64)>::new();
    for (w, c) in vi {
        if let Some(k) = vn_key(&w)
            && syllable.is_match(&k)
            && w.chars().count() <= 7
            && c >= 5.
        {
            if let Some((_, count)) = vn.iter_mut().find(|(key, _)| *key == k) {
                *count += c
            } else {
                vn.push((k, c));
            }
        }
    }
    let english: BTreeMap<_, _> = en
        .into_iter()
        .filter(|(w, _)| w.len() >= 2 && w.bytes().all(|c| c.is_ascii_lowercase()))
        .map(|(w, c)| (w, (10. * (c * 1e6 / et).log10()).round_ties_even() as i32))
        .collect();
    let mut pairs = vec![];
    for (k, c) in vn {
        if k.ends_with(['s', 'f', 'r', 'x', 'j'])
            && let Some(e) = english.get(&k)
        {
            let v = (10. * (c * 1e6 / vt).log10()).round_ties_even() as i32;
            pairs.push((k, *e, v));
        }
    }
    pairs.sort_by_key(|(_, e, v)| e - v);
    for (k, e, v) in pairs {
        println!("{k:10} en={e:4} vn={v:4} diff(en-vn)={:4}", e - v);
    }
    Ok(())
}
pub fn cases(a: &Args<'_>) -> Result {
    let path = a.path("--source", a.root.join("Tools/autoswitch/test/cases.tsv"));
    let text = fs::read_to_string(path)?;
    let rows: Vec<_> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| l.split('\t').map(str::to_owned).collect::<Vec<_>>())
        .collect();
    let (mut i, mut passed, mut count) = (0, 0, 0);
    let mut h = Harness::new(config_from_env());
    while i < rows.len() {
        let mut row = rows[i].clone();
        if row.len() == 3
            && i + 1 < rows.len()
            && rows[i + 1].len() == 2
            && rows[i + 1][0].is_empty()
        {
            row[2].push('\n');
            row.push(rows[i + 1][1].clone());
            i += 1;
        }
        if row.len() < 4 {
            return Err("invalid cases.tsv row".into());
        }
        h.reset(if row[0] == "0" {
            Language::English
        } else {
            Language::Vietnamese
        });
        for c in row[1].chars() {
            h.press(if c == '|' { '\n' } else { c })
        }
        let got = h.text();
        let lang = (h.engine.config.language as i32).to_string();
        let ok = got == row[2] && lang == row[3];
        passed += usize::from(ok);
        count += 1;
        println!(
            "{} {:?} -> {:?} lang={}{}",
            if ok { "PASS" } else { "FAIL" },
            row[1],
            got,
            lang,
            if ok {
                String::new()
            } else {
                format!("   expected {:?} lang={}", row[2], row[3])
            }
        );
        i += 1;
    }
    println!("{passed}/{count} passed");
    if passed < count {
        return Err("case regressions".into());
    }
    Ok(())
}
struct WordSets {
    vi: Vec<(String, String, f64)>,
    en: Vec<(String, f64)>,
}
fn word_sets(a: &Args<'_>) -> Result<WordSets> {
    let source = a.path("--source", a.root.join("Tools/autoswitch"));
    let syllable = regex::Regex::new(
        r"^(ngh|ch|gh|gi|kh|ng|nh|ph|qu|th|tr|b|c|dd|d|g|h|k|l|m|n|p|r|s|t|v|x)?(a|aw|aa|e|ee|i|o|oo|ow|u|uw|y)+(ch|ng|nh|c|m|n|p|t)?[sfrxj]?$",
    )?;
    let mut seen = BTreeSet::new();
    let vi = counts(&source.join("vi_50k.txt"))?
        .into_iter()
        .filter_map(|(w, c)| {
            let k = vn_key(&w)?;
            if syllable.is_match(&k) && c >= 50. && k != w && seen.insert(k.clone()) {
                Some((w, k, c))
            } else {
                None
            }
        })
        .collect();
    let en = counts(&source.join("en_50k.txt"))?
        .into_iter()
        .filter(|(w, _)| w.len() >= 2 && w.bytes().all(|c| c.is_ascii_lowercase()))
        .take(20000)
        .collect();
    Ok(WordSets { vi, en })
}
fn run_words(
    start: Language,
    words: impl IntoIterator<Item = String>,
    harness: Option<&Path>,
) -> Result<Vec<(String, usize)>> {
    let words: Vec<_> = words.into_iter().collect();
    if let Some(path) = harness {
        let input = words
            .iter()
            .map(|w| format!("{}\t{w} \n", start as i32))
            .collect::<String>();
        let mut child = Command::new(path)
            .arg("lines")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().ok_or("missing harness stdin")?;
        let output = std::thread::scope(|scope| -> Result<_> {
            let writer = scope.spawn(move || {
                let mut stdin = stdin;
                stdin.write_all(input.as_bytes())
            });
            let output = child.wait_with_output()?;
            writer.join().map_err(|_| "harness writer panicked")??;
            Ok(output)
        })?;
        if !output.status.success() {
            return Err("word harness failed".into());
        }
        let text = String::from_utf8(output.stdout)?;
        let results = text
            .lines()
            .map(|line| {
                let mut f = line.split('\t');
                let text = f.next().ok_or("missing harness text")?.to_owned();
                let switches = f.next().ok_or("missing harness switch count")?.parse()?;
                Ok((text, switches))
            })
            .collect::<Result<Vec<_>>>()?;
        if results.len() != words.len() {
            return Err("word harness returned too few rows".into());
        }
        return Ok(results);
    }
    let mut h = Harness::new(config_from_env());
    Ok(words
        .into_iter()
        .map(|word| {
            h.reset(start);
            for c in word.chars().chain(std::iter::once(' ')) {
                h.press(c)
            }
            (h.text(), h.switches)
        })
        .collect())
}
struct WordMetrics {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    bad_a: Vec<usize>,
    bad_b: Vec<usize>,
    a_out: Vec<(String, usize)>,
    b_out: Vec<(String, usize)>,
    c_out: Vec<(String, usize)>,
    d_out: Vec<(String, usize)>,
}
fn word_metrics(words: &WordSets, harness: Option<&Path>) -> Result<WordMetrics> {
    let a_out = run_words(
        Language::Vietnamese,
        words.vi.iter().map(|(_, k, _)| k.clone()),
        harness,
    )?;
    let d_out = run_words(
        Language::English,
        words.vi.iter().map(|(_, k, _)| k.clone()),
        harness,
    )?;
    let b_out = run_words(
        Language::English,
        words.en.iter().map(|(w, _)| w.clone()),
        harness,
    )?;
    let c_out = run_words(
        Language::Vietnamese,
        words.en.iter().map(|(w, _)| w.clone()),
        harness,
    )?;
    let vt = words.vi.iter().map(|(_, _, c)| c).sum::<f64>();
    let et = words.en.iter().map(|(_, c)| c).sum::<f64>();
    let bad_a: Vec<_> = a_out
        .iter()
        .enumerate()
        .filter(|(_, r)| r.1 != 0)
        .map(|(i, _)| i)
        .collect();
    let bad_b: Vec<_> = b_out
        .iter()
        .enumerate()
        .filter(|(_, r)| r.1 != 0)
        .map(|(i, _)| i)
        .collect();
    let a = 100. * bad_a.iter().map(|&i| words.vi[i].2).sum::<f64>() / vt;
    let b = 100. * bad_b.iter().map(|&i| words.en[i].1).sum::<f64>() / et;
    let c = 100.
        * c_out
            .iter()
            .enumerate()
            .filter(|(i, r)| r.0.trim() != words.en[*i].0)
            .map(|(i, _)| words.en[i].1)
            .sum::<f64>()
        / et;
    let d = 100.
        * d_out
            .iter()
            .enumerate()
            .filter(|(_, r)| r.1 != 1)
            .map(|(i, _)| words.vi[i].2)
            .sum::<f64>()
        / vt;
    Ok(WordMetrics {
        a,
        b,
        c,
        d,
        bad_a,
        bad_b,
        a_out,
        b_out,
        c_out,
        d_out,
    })
}
fn quoted(s: &str) -> String {
    format!("'{s}'")
}
pub fn bulk(a: &Args<'_>) -> Result {
    let words = word_sets(a)?;
    let m = word_metrics(&words, a.option("--harness").map(Path::new))?;
    println!(
        "A VN words in VN mode: {}/{} switched to EN, {:.3}% of VN usage",
        m.bad_a.len(),
        words.vi.len(),
        m.a
    );
    println!(
        "   [{}]",
        m.bad_a
            .iter()
            .take(40)
            .map(|&i| {
                let (w, k, c) = &words.vi[i];
                format!(
                    "({}, {}, {}, {})",
                    quoted(w),
                    quoted(k),
                    *c as u64,
                    quoted(&m.a_out[i].0)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    );
    let converted = m.d_out.iter().filter(|r| r.1 == 1).count();
    println!(
        "D VN words typed in EN mode converted: {converted}/{} words, {:.1}% of VN usage",
        words.vi.len(),
        100. - m.d
    );
    let mut misses = words
        .vi
        .iter()
        .enumerate()
        .filter(|(i, _)| m.d_out[*i].1 != 1)
        .map(|(_, w)| w)
        .collect::<Vec<_>>();
    misses.sort_by(|a, b| b.2.total_cmp(&a.2).then(b.0.cmp(&a.0)).then(b.1.cmp(&a.1)));
    println!(
        "   top misses [{}]",
        misses
            .into_iter()
            .take(25)
            .map(|(w, k, c)| format!("({}, {}, {})", *c as u64, quoted(w), quoted(k)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "B EN words in EN mode: {}/{} wrongly converted, {:.4}% of EN usage",
        m.bad_b.len(),
        words.en.len(),
        m.b
    );
    println!(
        "   [{}]",
        m.bad_b
            .iter()
            .take(60)
            .map(|&i| format!("({}, {})", quoted(&words.en[i].0), quoted(&m.b_out[i].0)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let changed = m
        .c_out
        .iter()
        .enumerate()
        .filter(|(i, r)| r.0.trim() != words.en[*i].0)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    println!(
        "C EN words typed in VN mode: {}/{} still wrong after space ({:.2}% of usage)",
        changed.len(),
        words.en.len(),
        m.c
    );
    println!(
        "   top [{}]",
        changed
            .into_iter()
            .take(40)
            .map(|i| format!("({}, {})", quoted(&words.en[i].0), quoted(&m.c_out[i].0)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}
pub fn sweep(a: &Args<'_>) -> Result {
    let words = word_sets(a)?;
    let target = a.root.join("target/experiment-sweep");
    for (p, vf, ef) in [
        (10, 0, 0),
        (10, 100, 100),
        (10, 15, 5),
        (10, 10, 5),
        (10, 10, 0),
        (10, 20, 5),
        (15, 15, 5),
        (10, 15, 10),
    ] {
        let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(a.root)
            .args([
                "build",
                "--locked",
                "--release",
                "-p",
                "sevi-test-tools",
                "--bin",
                "harness",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env("AS_PREFER", p.to_string())
            .env("AS_VN_FLOOR", vf.to_string())
            .env("AS_EN_FLOOR", ef.to_string())
            .status()?;
        if !status.success() {
            return Err("building sweep harness failed".into());
        }
        let harness = crate::quality::executable(&target.join("release"), "harness");
        let m = word_metrics(&words, Some(&harness))?;
        println!(
            "prefer={p} vnFloor={vf} enFloor={ef}: wrongVN->EN {} ({:.3}%)  wrongEN->VN {} ({:.4}%)  | missed EN-in-VN {:.2}%  missed VN-in-EN {:.2}%",
            m.bad_a.len(),
            m.a,
            m.bad_b.len(),
            m.b,
            m.c,
            m.d
        );
    }
    Ok(())
}
pub fn compare_driver(a: &Args<'_>) -> Result {
    use sevi_engine::Driver;
    use sevi_test_tools::sim::{Simulator, units};
    let path = a.args.first().ok_or("compare-driver <units> [en|vi]")?;
    let source = fs::read_to_string(path)?;
    let mut config = config_from_env();
    if a.args.get(1).is_some_and(|v| v == "en") {
        config.language = Language::English;
    }
    let mut driver = Driver::new(config.clone());
    let mut simulator = Simulator::new(config);
    let mut screen = Vec::<u16>::new();
    for unit in units(&source) {
        for c in unit.keys.chars().chain(unit.separators.chars()) {
            if let Some((key, shift)) = sevi_test_tools::key_for(c) {
                let edit = driver.press(key, shift, 0.);
                screen.truncate(screen.len().saturating_sub(edit.delete_count));
                screen.extend(edit.text);
                simulator.press(key, shift);
            }
        }
    }
    let normalize = |s: &[u16]| {
        String::from_utf16_lossy(s)
            .chars()
            .filter(|c| c.is_alphabetic())
            .collect::<String>()
    };
    let actual = normalize(&screen);
    let expected = normalize(&simulator.screen);
    println!(
        "whole text equal: {} {} {}",
        if actual == expected { "True" } else { "False" },
        actual.chars().count(),
        expected.chars().count()
    );
    if actual != expected {
        return Err("driver/simulator output differs".into());
    }
    Ok(())
}

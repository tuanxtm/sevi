use crate::{key_for, rng::Mt19937};
use sevi_engine::{
    Driver, Engine,
    touch::{TouchKey, TouchModel},
};
use std::{env, error::Error, fs, time::Instant};
pub fn layout() -> Vec<TouchKey> {
    let (side, gap) = (6.67f32, 6.0f32);
    let width = (402.0 - 2.0 * side - 9.0 * gap) / 10.0;
    let starts = [side, side + (width + gap) / 2.0, side + (width + gap) * 1.5];
    let mut out = vec![];
    for (r, row) in [b"qwertyuiop".as_slice(), b"asdfghjkl", b"zxcvbnm"]
        .iter()
        .enumerate()
    {
        for (i, &letter) in row.iter().enumerate() {
            out.push(TouchKey {
                letter,
                x: starts[r] + i as f32 * (width + gap),
                y: r as f32 * 54.0,
                width,
                height: 43.0,
            })
        }
    }
    out
}
pub fn model() -> Result<TouchModel, Box<dyn Error>> {
    let mut m = TouchModel::default();
    if let Ok(v) = env::var("TOUCH_DELETE") {
        m.delete_cost = v.parse()?
    }
    if let Ok(v) = env::var("TOUCH_FLOOR_LONG") {
        m.correct_floor_long = v.parse()?
    }
    if let Ok(p) = env::var("TOUCH_KEEP") {
        m.keep_words = fs::read_to_string(p)?
    }
    Ok(m)
}
pub fn normeval(args: &[String]) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(args.first().ok_or("usage: normeval pairs.tsv")?)?;
    let m = model()?;
    let engine = Engine::default();
    let keys = layout();
    let (mut right, mut wrong, mut broke, mut fixed, mut other) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut broken = std::collections::BTreeMap::<String, u64>::new();
    for line in source.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() < 3 {
            continue;
        }
        let (typed, want, same) = (fields[0], fields[1], fields[2].starts_with('1'));
        if same {
            right += 1
        } else {
            wrong += 1
        }
        if typed.len() > 24 {
            continue;
        }
        let candidates = m.candidates(&engine, typed, None, true, &keys, 8);
        if !m.should_correct(&engine, typed, &candidates) {
            continue;
        }
        let got = String::from_utf16_lossy(&candidates[0].text).to_ascii_lowercase();
        if same {
            broke += 1;
            *broken.entry(format!("{typed} -> {got}")).or_default() += 1;
        } else if got == want {
            fixed += 1
        } else {
            other += 1
        }
    }
    println!(
        "vilexnorm words: {right} typed right, {wrong} not | broke {broke} ({:.2}% of right words) | fixed {fixed} ({:.1}% of the rest), changed to something else {other}",
        100.0 * broke as f64 / right as f64,
        100.0 * fixed as f64 / wrong as f64
    );
    if env::var_os("TOUCH_SHOW").is_some() {
        let mut list: Vec<_> = broken.into_iter().map(|(word, n)| (n, word)).collect();
        list.sort_by(|a, b| b.cmp(a));
        for (n, word) in list.into_iter().take(25) {
            eprintln!("{n:4} {word}");
        }
    }
    Ok(())
}
#[derive(Default)]
struct Normal32 {
    saved: Option<f32>,
}
impl Normal32 {
    fn sample(&mut self, rng: &mut Mt19937) -> f32 {
        if let Some(v) = self.saved.take() {
            return v;
        }
        let uniform = |r: &mut Mt19937| {
            let v = r.next_u32() as f32 / 4294967296.0f32;
            if v >= 1.0 {
                f32::from_bits(1.0f32.to_bits() - 1)
            } else {
                v
            }
        };
        loop {
            let x = 2.0 * uniform(rng) - 1.0;
            let y = 2.0 * uniform(rng) - 1.0;
            let radius = x * x + y * y;
            if radius > 1.0 || radius == 0.0 {
                continue;
            }
            let mult = (-2.0 * radius.ln() / radius).sqrt();
            self.saved = Some(x * mult);
            return y * mult;
        }
    }
}
pub fn touchsim(args: &[String]) -> Result<(), Box<dyn Error>> {
    let source =
        fs::read_to_string(args.first().ok_or(
            "usage: touchsim units [noise] [weight] [sigma] [sure] [mode_share] [sigma_y]",
        )?)?;
    let noise: f64 = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(0.30);
    let mut m = model()?;
    for (i, field) in [
        &mut m.weight,
        &mut m.sigma,
        &mut m.sure_zone,
        &mut m.mode_share,
        &mut m.sigma_y,
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(v) = args.get(i + 2) {
            *field = v.parse()?
        }
    }
    let noise_y: f64 = env::var("TOUCH_NOISE_Y")
        .ok()
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(noise * 2.0 / 3.0);
    let real = env::var_os("TOUCH_LABEL_MODE").is_none();
    let correct = env::var_os("TOUCH_CORRECT").is_some();
    let keys = layout();
    let mut rng = Mt19937::new(42);
    let (mut nx, mut ny) = (Normal32::default(), Normal32::default());
    let mut driver = Driver::default();
    let (
        mut key_count,
        mut err_n,
        mut err_m,
        mut words,
        mut word_n,
        mut word_m,
        mut rescued,
        mut broken,
        mut corrected_err,
        mut corrections,
        mut good,
        mut bad,
    ) = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let mut ns = 0.0;
    for line in source.lines() {
        let Some(rest) = line.strip_prefix("W\t") else {
            continue;
        };
        let Some((raw, lang)) = rest.split_once('\t') else {
            continue;
        };
        let vi = if real {
            driver.language() == sevi_engine::Language::Vietnamese
        } else {
            lang.starts_with('1')
        };
        let word: String = raw
            .chars()
            .filter(char::is_ascii_alphabetic)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if word.is_empty() {
            continue;
        }
        words += 1;
        let (mut dn, mut dm) = (String::new(), String::new());
        let mut touches = vec![];
        for c in word.bytes() {
            let k = keys.iter().find(|k| k.letter == c).unwrap();
            let x = k.x + k.width / 2.0 + nx.sample(&mut rng) * (noise * k.width as f64) as f32;
            let y = k.y + k.height / 2.0 + ny.sample(&mut rng) * (noise_y * k.height as f64) as f32;
            let mut nearest = 0;
            let mut distance = 1e30;
            for k in &keys {
                let dx = (x - (k.x + k.width / 2.0)) / k.width;
                let dy = (y - (k.y + k.height / 2.0)) / k.height;
                let d = dx * dx + dy * dy;
                if d < distance {
                    distance = d;
                    nearest = k.letter
                }
            }
            let start = Instant::now();
            let picked = m.pick(x, y, &dm, vi, &keys).unwrap();
            ns += start.elapsed().as_nanos() as f64;
            dn.push(nearest as char);
            dm.push(picked as char);
            touches.push((x, y));
            key_count += 1;
            if nearest != c {
                err_n += 1
            }
            if picked != c {
                err_m += 1
            }
        }
        for c in raw.chars().chain(std::iter::once(' ')) {
            if (c.is_ascii_alphabetic() || " ,.".contains(c))
                && let Some((k, s)) = key_for(c)
            {
                driver.press(k, s, 0.0);
            }
        }
        let (ok_n, ok_m) = (dn == word, dm == word);
        if correct {
            let mut result = dm.clone();
            if raw.chars().filter(char::is_ascii_uppercase).count() < 2 {
                let candidates = m.candidates(&driver.engine, &dm, Some(&touches), vi, &keys, 8);
                if m.should_correct(&driver.engine, &dm, &candidates) {
                    result = candidates[0].letters.clone();
                    corrections += 1;
                    if ok_m {
                        bad += 1;
                        if env::var_os("TOUCH_SHOW").is_some() {
                            eprintln!("BROKE {dm} -> {result}")
                        }
                    } else if result == word {
                        good += 1
                    }
                }
            }
            if result != word {
                corrected_err += 1
            }
        }
        if !ok_n {
            word_n += 1
        }
        if !ok_m {
            word_m += 1
        }
        if !ok_n && ok_m {
            rescued += 1
        }
        if ok_n && !ok_m {
            broken += 1
        }
    }
    let pct = |n: usize, d: usize| 100.0 * n as f64 / d as f64;
    println!(
        "noise {noise:.2} weight {:.2} sigma {:.2} sure {:.2} | keys {key_count}: nearest {:.2}% model {:.2}% | words {words}: nearest {:.2}% wrong, model {:.2}% wrong (rescued {rescued}, broken {broken})",
        m.weight,
        m.sigma,
        m.sure_zone,
        pct(err_n, key_count),
        pct(err_m, key_count),
        pct(word_n, words),
        pct(word_m, words)
    );
    println!("pick: {:.0} ns per touch on average", ns / key_count as f64);
    if correct {
        println!(
            "auto-correct: {:.2}% wrong words after correction; {corrections} corrections: {good} fixed a wrong word, {bad} broke a right one",
            pct(corrected_err, words)
        )
    }
    Ok(())
}
pub fn typoeval(args: &[String]) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(
        args.first()
            .ok_or("usage: typoeval pairs.tsv [mode] [keep.txt]")?,
    )?;
    let vi = args.get(1).is_some_and(|s| s != "0");
    let mut m = model()?;
    if let Some(p) = args.get(2) {
        m.keep_words = fs::read_to_string(p)?
    }
    let engine = Engine::default();
    let keys = layout();
    let (mut pairs, mut right, mut wrong, mut left, mut known, mut wr, mut ww, mut wl, mut all) =
        (0, 0, 0, 0, 0, 0u64, 0u64, 0u64, 0u64);
    for line in source.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 2 {
            continue;
        }
        let (typed, want) = (p[0], p[1]);
        if !(3..=20).contains(&typed.len())
            || !typed
                .bytes()
                .chain(want.bytes())
                .all(|b| b.is_ascii_lowercase())
        {
            continue;
        }
        let count = p
            .get(2)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1)
            .max(1);
        let c = m.candidates(&engine, typed, None, vi, &keys, 8);
        if c.iter().any(|c| c.letters == typed) {
            known += 1;
            continue;
        }
        pairs += 1;
        all += count;
        if m.should_correct(&engine, typed, &c) {
            if c[0].letters == want {
                right += 1;
                wr += count
            } else {
                wrong += 1;
                ww += count;
                if env::var_os("TYPO_SHOW").is_some() {
                    eprintln!("WRONG {typed} -> {} (meant {want})", c[0].letters)
                }
            }
        } else {
            left += 1;
            wl += count;
            if env::var_os("TYPO_SHOW").is_some() {
                eprintln!("LEFT {typed} (meant {want})")
            }
        }
    }
    let pct = |n: u64, d: u64| 100.0 * n as f64 / d as f64;
    println!(
        "typos {pairs} (skipped {known} that are words): fixed {:.1}%, wrong fix {:.1}%, left {:.1}% | by count: fixed {:.1}%, wrong {:.1}%, left {:.1}%",
        pct(right, pairs),
        pct(wrong, pairs),
        pct(left, pairs),
        pct(wr, all),
        pct(ww, all),
        pct(wl, all)
    );
    Ok(())
}

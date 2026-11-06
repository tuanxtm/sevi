use crate::{Args, Result, counts, emit_words, env_f64};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
use unicode_normalization::UnicodeNormalization;
pub fn vn_key(word: &str) -> Option<String> {
    let mut out = String::new();
    let mut tone = None;
    for c in word.to_lowercase().nfd() {
        match c {
            '\u{301}' | '\u{300}' | '\u{309}' | '\u{303}' | '\u{323}' => {
                if tone.is_some() {
                    return None;
                }
                tone = Some(match c {
                    '\u{301}' => 's',
                    '\u{300}' => 'f',
                    '\u{309}' => 'r',
                    '\u{303}' => 'x',
                    _ => 'j',
                });
            }
            '\u{302}' => {
                let last = out.chars().last()?;
                if !"aeo".contains(last) {
                    return None;
                }
                out.push(last)
            }
            '\u{306}' => {
                if !out.ends_with('a') {
                    return None;
                }
                out.push('w')
            }
            '\u{31b}' => {
                if !out.ends_with(['o', 'u']) {
                    return None;
                }
                out.push('w')
            }
            'đ' => out.push_str("dd"),
            c if "abcdeghiklmnopqrstuvxy".contains(c) => out.push(c),
            _ => return None,
        }
    }
    if let Some(t) = tone {
        out.push(t)
    }
    Some(out)
}
pub fn score(count: f64, total: f64) -> i32 {
    (10. * (count * 1e6 / total).log10())
        .round_ties_even()
        .clamp(-120., 120.) as i32
}
fn foreign(word: &str) -> bool {
    word.to_lowercase()
        .nfd()
        .last()
        .is_some_and(|c| "srx".contains(c))
}
fn lower_only(name: &str) -> bool {
    std::env::var(name).unwrap_or_else(|_| "1".into()) == "1"
}
fn blend(
    vn: &mut BTreeMap<String, f64>,
    extra: &BTreeMap<String, f64>,
    total: f64,
    extra_total: f64,
    share: f64,
    only_lower: bool,
) {
    let keys: BTreeSet<_> = vn.keys().chain(extra.keys()).cloned().collect();
    for key in keys {
        let previous = vn.get(&key).copied().unwrap_or(0.) / total;
        let mixed =
            (1. - share) * previous + share * extra.get(&key).copied().unwrap_or(0.) / extra_total;
        let rate = if previous > 0. && only_lower {
            previous.min(mixed)
        } else {
            mixed
        };
        vn.insert(key, rate * total);
    }
    vn.retain(|_, count| *count > 0.);
}
pub fn run(a: &Args<'_>) -> Result {
    let source = a.path("--source", a.root.join("Tools/autoswitch"));
    let output = a.output("Engine/AutoSwitchData.h");
    let vi = counts(&source.join("vi_50k.txt"))?;
    let vi_total = vi.iter().map(|(_, c)| c).sum::<f64>();
    let mut vn = BTreeMap::<String, f64>::new();
    let syllable = regex::Regex::new(
        r"^(ngh|ch|gh|gi|kh|ng|nh|ph|qu|th|tr|b|c|dd|d|g|h|k|l|m|n|p|r|s|t|v|x)?(a|aw|aa|e|ee|i|o|oo|ow|u|uw|y)+(ch|ng|nh|c|m|n|p|t)?[sfrxj]?$",
    )?;
    let shorthand = regex::Regex::new(r"^dd[bcdghklmnpqrstvx]{0,2}$")?;
    for (w, c) in &vi {
        if foreign(w) {
            continue;
        }
        if let Some(k) = vn_key(w)
            && w.chars().count() <= 7
            && *c >= 5.
            && syllable.is_match(&k)
        {
            *vn.entry(k).or_default() += c;
        }
    }
    let informal = std::env::var_os("SEVI_INFORMAL")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| a.data().join("freq"));
    let inf_share = env_f64("SEVI_INFORMAL_SHARE", 0.5)?;
    let mut inf = BTreeMap::<String, f64>::new();
    let mut inf_total = 0.;
    if inf_share > 0. && informal.join("vi_informal.txt").exists() {
        inf_total = fs::read_to_string(informal.join("total.txt"))?
            .trim()
            .parse()?;
        for (w, c) in counts(&informal.join("vi_informal.txt"))? {
            if let Some(k) = vn_key(&w)
                && !foreign(&w)
                && ((w.chars().count() <= 7 && c >= 5. && syllable.is_match(&k))
                    || (shorthand.is_match(&k) && c >= 50.))
            {
                *inf.entry(k).or_default() += c;
            }
        }
    }
    let bulk = a.data().join("bulk/merged");
    let bulk_share = env_f64("SEVI_BULK_SHARE", 0.5)?;
    let bulk_total = || -> Result<f64> {
        let v: serde_json::Value = serde_json::from_slice(&fs::read(bulk.join("sources.json"))?)?;
        v["totals"]["tokens_vi"]
            .as_f64()
            .ok_or_else(|| "missing bulk token total".into())
    };
    if bulk_share > 0. && bulk.join("vi_counts.txt").exists() {
        let mut words = BTreeMap::<String, f64>::new();
        for (w, c) in counts(&bulk.join("vi_counts.txt"))? {
            if let Some(k) = vn_key(&w)
                && !foreign(&w)
                && w.chars().count() <= 7
                && c >= 20.
                && syllable.is_match(&k)
            {
                *words.entry(k).or_default() += c;
            }
        }
        blend(
            &mut vn,
            &words,
            vi_total,
            bulk_total()?,
            bulk_share,
            lower_only("SEVI_BULK_LOWER_ONLY"),
        );
    }
    if inf_total > 0. {
        for (w, c) in &vi {
            if let Some(k) = vn_key(w)
                && shorthand.is_match(&k)
                && *c >= 50.
                && inf.contains_key(&k)
            {
                *vn.entry(k).or_default() += c;
            }
        }
        blend(
            &mut vn,
            &inf,
            vi_total,
            inf_total,
            inf_share,
            lower_only("SEVI_INFORMAL_LOWER_ONLY"),
        );
    }
    let mut vn: BTreeMap<String, i32> = vn
        .into_iter()
        .map(|(k, c)| (k, score(c, vi_total)))
        .collect();
    vn.remove("see");
    let english = counts(&source.join("en_50k.txt"))?;
    let en_total = english.iter().map(|(_, c)| c).sum::<f64>();
    let accepted = fs::read_to_string(source.join("en_spell_ok.txt"))?;
    let accepted: BTreeSet<_> = accepted.split_whitespace().collect();
    let mut en = BTreeMap::<String, i32>::new();
    for (w, c) in english {
        if w.len() >= 2
            && w.bytes().all(|b| b.is_ascii_lowercase())
            && accepted.contains(w.as_str())
        {
            let v = score(c, en_total);
            en.entry(w).and_modify(|n| *n = (*n).max(v)).or_insert(v);
        }
    }
    for w in ["aj", "ej", "oj", "ix"] {
        en.remove(w);
    }
    let mut boost = |path: &std::path::Path, min: f64, total: f64| -> Result {
        for (w, c) in counts(path)? {
            if c >= min
                && !vn_key(&w).is_some_and(|k| vn.contains_key(&k))
                && let Some(v) = en.get_mut(&w)
            {
                *v = (*v).max(score(c, total));
            }
        }
        Ok(())
    };
    if inf_total > 0. && lower_only("SEVI_EN_IN_VI") {
        boost(&informal.join("en_in_vi.txt"), 5., inf_total)?;
    }
    if lower_only("SEVI_BULK_EN") && bulk.join("en_in_vi_counts.txt").exists() {
        boost(&bulk.join("en_in_vi_counts.txt"), 20., bulk_total()?)?;
    }
    for name in ["en_extra.txt", "en_code.txt"] {
        for line in fs::read_to_string(source.join(name))?.lines() {
            if line.starts_with('#') {
                continue;
            }
            for w in line.split_whitespace() {
                en.entry(w.into())
                    .and_modify(|v| *v = (*v).max(40))
                    .or_insert(40);
            }
        }
    }
    for (word, score) in [("os", 0), ("docs", 20)] {
        if let Some(v) = en.get_mut(word) {
            *v = score;
        }
    }
    let text = format!(
        "// Generated by Tools/autoswitch/build_dict.py. Do not edit.\n// Word frequencies: FrequencyWords (Hermit Dave), OpenSubtitles 2018, CC BY-SA 4.0.\n#ifndef AutoSwitchData_h\n#define AutoSwitchData_h\n\n{}\n{}\n#endif\n",
        emit_words("kAutoSwitchVn", &vn),
        emit_words("kAutoSwitchEn", &en)
    );
    fs::write(&output, &text)?;
    println!("vn {} en {} bytes {}", vn.len(), en.len(), text.len());
    for w in [
        "treen", "truowfng", "moer", "more", "last", "carm", "dos", "nos",
    ] {
        let val = |v: Option<&i32>| v.map(ToString::to_string).unwrap_or_else(|| "None".into());
        println!("{w} EN {} VNkey {}", val(en.get(w)), val(vn.get(w)));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_keys_and_python_rounding() {
        assert_eq!(vn_key("trường"), Some("truwowngf".into()));
        assert_eq!(vn_key("hoà"), vn_key("hòa"));
        assert_eq!(vn_key("sữa"), Some("suwax".into()));
        assert_eq!(vn_key("a\u{301}\u{300}"), None);
        assert_eq!(score(1., 1e6), 0);
    }
}

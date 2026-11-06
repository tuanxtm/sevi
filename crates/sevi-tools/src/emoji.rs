use crate::{Args, Result};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use unicode_segmentation::UnicodeSegmentation;
const PACKAGES: [(&str, &str); 3] = [
    ("emojibase-data", "18.0.0"),
    ("cldr-annotations-full", "49.0.0"),
    ("cldr-annotations-derived-full", "49.0.0"),
];
fn read(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn fetch(root: &Path) -> Result<PathBuf> {
    let directory = root.join("target/emoji-packages");
    fs::create_dir_all(&directory)?;
    for (name, version) in PACKAGES {
        let output = Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" })
            .args(["pack", &format!("{name}@{version}"), "--silent"])
            .env("npm_config_cache", root.join("target/npm-cache"))
            .current_dir(&directory)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "npm pack {name} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let filename = stdout
            .lines()
            .last()
            .ok_or("npm pack returned no archive")?;
        if Path::new(filename).file_name().and_then(|s| s.to_str()) != Some(filename) {
            return Err("npm pack returned an invalid archive filename".into());
        }
        let dest = directory.join(name);
        fs::create_dir_all(&dest)?;
        let gzip = flate2::read::GzDecoder::new(fs::File::open(directory.join(filename))?);
        tar::Archive::new(gzip).unpack(&dest)?;
    }
    Ok(directory)
}
fn cldr(packages: &Path, lang: &str) -> Result<BTreeMap<String, Value>> {
    let normal: CldrRoot = serde_json::from_slice(&fs::read(packages.join(format!(
        "cldr-annotations-full/package/annotations/{lang}/annotations.json"
    )))?)?;
    let derived: CldrRoot = serde_json::from_slice(&fs::read(packages.join(format!(
        "cldr-annotations-derived-full/package/annotationsDerived/{lang}/annotations.json"
    )))?)?;
    let mut merged = Vec::<(String, Value)>::new();
    for (key, value) in derived
        .annotations
        .annotations
        .0
        .into_iter()
        .chain(normal.annotations.annotations.0)
    {
        if let Some((_, old)) = merged.iter_mut().find(|(k, _)| *k == key) {
            *old = value;
        } else {
            merged.push((key, value));
        }
    }
    // Normalize after merging, matching the original dictionary comprehension.
    Ok(merged
        .into_iter()
        .map(|(key, value)| (norm(&key), value))
        .collect())
}
#[derive(serde::Deserialize)]
struct CldrRoot {
    #[serde(alias = "annotationsDerived")]
    annotations: CldrBody,
}
#[derive(serde::Deserialize)]
struct CldrBody {
    annotations: OrderedAnnotations,
}
struct OrderedAnnotations(Vec<(String, Value)>);
impl<'de> serde::Deserialize<'de> for OrderedAnnotations {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedAnnotations;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a CLDR annotation object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut rows = vec![];
                while let Some(row) = map.next_entry()? {
                    rows.push(row)
                }
                Ok(OrderedAnnotations(rows))
            }
        }
        d.deserialize_map(Visitor)
    }
}
fn norm(s: &str) -> String {
    s.replace('\u{fe0f}', "")
}
fn strings(value: &Value) -> Vec<String> {
    if let Some(s) = value.as_str() {
        vec![s.into()]
    } else {
        value
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
}
pub fn run(a: &Args<'_>) -> Result {
    let packages = if let Some(path) = a.option("--packages") {
        PathBuf::from(path)
    } else {
        fetch(a.root)?
    };
    let data = read(packages.join("emojibase-data/package/en/data.json"))?;
    let github = read(packages.join("emojibase-data/package/en/shortcodes/github.json"))?;
    let en = cldr(&packages, "en")?;
    let vi = cldr(&packages, "vi")?;
    let popular: BTreeMap<_, _> = include_str!("popular.txt")
        .graphemes(true)
        .enumerate()
        .map(|(i, g)| (norm(g), i))
        .collect();
    let mut emoticons = BTreeMap::<String, BTreeSet<String>>::new();
    let all = data.as_array().ok_or("emojibase data must be an array")?;
    for x in all {
        let key = norm(x["emoji"].as_str().ok_or("missing emoji")?);
        for t in strings(&x["emoticon"]) {
            emoticons
                .entry(key.clone())
                .or_default()
                .insert(t.to_lowercase());
        }
    }
    let extra: BTreeMap<String, String> = serde_json::from_str(include_str!("emoticons.json"))?;
    for (t, chars) in extra {
        for g in chars.graphemes(true) {
            emoticons.entry(norm(g)).or_default().insert(t.clone());
        }
    }
    let mut items: Vec<_> = all
        .iter()
        .filter(|x| !x["group"].is_null() && x["group"] != 2)
        .collect();
    items.sort_by_key(|x| x["order"].as_i64().unwrap_or(0));
    let mut out = vec![];
    for x in items {
        let e = x["emoji"].as_str().unwrap();
        let key = norm(e);
        let empty = Value::Null;
        let ea = en.get(&key).unwrap_or(&empty);
        let va = vi.get(&key).unwrap_or(&empty);
        let codes = strings(&github[x["hexcode"].as_str().unwrap_or("")]);
        let mut enk: BTreeSet<_> = strings(&ea["default"])
            .into_iter()
            .chain(strings(&x["tags"]))
            .collect();
        enk.extend(codes.into_iter().map(|c| c.replace('_', " ")));
        let vik: BTreeSet<_> = strings(&va["default"]).into_iter().collect();
        let enname = strings(&ea["tts"])
            .first()
            .cloned()
            .unwrap_or_else(|| x["label"].as_str().unwrap_or("").into());
        let viname = strings(&va["tts"]).first().cloned().unwrap_or_default();
        let skins: Vec<_> = x["skins"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["tone"].is_i64() || s["tone"].is_u64())
            .map(|s| s["emoji"].clone())
            .collect();
        let popularity = popular
            .get(&key)
            .map(|i| 10usize.saturating_sub(i / 16))
            .unwrap_or(0);
        let emoticons = emoticons
            .get(&key)
            .map(|v| v.iter().cloned().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        out.push(json!([
            e,
            x["group"],
            x["version"],
            enname,
            viname,
            enk.into_iter().collect::<Vec<_>>().join(" | "),
            vik.into_iter().collect::<Vec<_>>().join(" | "),
            if skins.len() == 5 {
                json!(skins)
            } else {
                json!(0)
            },
            popularity,
            emoticons
        ]));
    }
    let output = a.output("App/Resources/Emoji.json");
    let text = serde_json::to_vec(&out)?;
    fs::write(&output, &text)?;
    let missing = out.iter().filter(|o| o[4] == "").count();
    println!(
        "{} emoji -> {} ({} KB), {} without a Vietnamese name",
        out.len(),
        output.strip_prefix(a.root).unwrap_or(&output).display(),
        text.len() / 1024,
        missing
    );
    Ok(())
}

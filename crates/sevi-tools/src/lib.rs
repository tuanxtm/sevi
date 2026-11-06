//! Existing support-tool behavior, dispatched by `cargo xtask tool`.
use std::{
    error::Error,
    path::{Path, PathBuf},
};
pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
mod appcast;
mod corpus;
mod dictionary;
mod emoji;
mod experiments;
mod generators;
mod images;
mod log;
mod pacman;
mod quality;
mod release;
mod report;
mod site;
mod spell;
pub use release::check_metadata;
pub struct Args<'a> {
    pub root: &'a Path,
    pub args: &'a [String],
}
impl Args<'_> {
    pub fn option(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .position(|v| v == name)
            .and_then(|i| self.args.get(i + 1))
            .map(String::as_str)
    }
    pub fn path(&self, name: &str, default: impl AsRef<Path>) -> PathBuf {
        self.option(name)
            .map(PathBuf::from)
            .unwrap_or_else(|| default.as_ref().to_owned())
    }
    pub fn data(&self) -> PathBuf {
        self.option("--data")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("SEVI_DATA").map(PathBuf::from))
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Work/sevi-data")
            })
    }
    pub fn output(&self, name: &str) -> PathBuf {
        self.path("--output", self.root.join(name))
    }
}
pub fn run(root: &Path, args: &[String]) -> Result {
    let a = Args {
        root,
        args: args.get(1..).unwrap_or_default(),
    };
    match args.first().map(String::as_str){
        Some("build-dict")=>dictionary::run(&a),
        Some("build-bigrams")=>generators::bigrams(&a),
        Some("build-teencode")=>generators::teencode(&a),
        Some("keepgen")=>generators::keep(&a),
        Some("analyze-log")=>log::run(&a),
        Some("appcast")=>appcast::run(&a),
        Some("build-emoji")=>emoji::run(&a),
        Some("spellfilter")=>spell::run(),
        Some("scorecard")=>quality::run(&a),
        Some("teen-eval")=>experiments::teen(&a),
        Some("margins")=>experiments::margins(&a),
        Some("bulk")=>experiments::bulk(&a),
        Some("sweep")=>experiments::sweep(&a),
        Some("cases")=>experiments::cases(&a),
        Some("pr-report")=>report::run(),
        Some("clean-viblo")=>corpus::clean(&a),
        Some("fetch-viblo")=>corpus::fetch(&a),
        Some("gen-chat")=>corpus::chat(&a),
        Some("site-build")=>site::run(&a),
        Some("make-pacman")=>pacman::run(&a),
        Some("dmg-background")=>images::dmg(&a),
        Some("dl-shots")=>images::shots(&a),
        Some("release")=>release::run(&a),
        Some("deploy-site")=>release::deploy(&a),
        Some("metadata")=>release::metadata(&a),
        Some("compare-driver")=>experiments::compare_driver(&a),
        _=>Err("usage: cargo xtask tool <command> [args]; see Tools/README.md for all migrated commands".into()),
    }
}
pub fn env_f64(name: &str, default: f64) -> Result<f64> {
    Ok(std::env::var(name)
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(default))
}
pub fn counts(path: &Path) -> Result<Vec<(String, f64)>> {
    std::fs::read_to_string(path)?
        .lines()
        .map(|l| {
            let mut p = l.split_whitespace();
            let word = p.next().ok_or("missing count word")?.to_owned();
            let count = p.next().ok_or("missing count")?.parse()?;
            Ok((word, count))
        })
        .collect()
}
pub fn emit_words(name: &str, table: &std::collections::BTreeMap<String, i32>) -> String {
    let keys: Vec<_> = table.keys().collect();
    let mut out = format!("static const char {name}Words[] =\n");
    let mut offsets = vec![];
    let mut offset = 0usize;
    for chunk in keys.chunks(200) {
        out.push_str("    \"");
        for key in chunk {
            offsets.push(offset);
            offset += key.len() + 1;
            out.push_str(key);
            out.push_str("\\0");
        }
        out.push_str("\"\n");
    }
    if !keys.is_empty() {
        out.pop();
    }
    out.push_str(";\n");
    out.push_str(&format!(
        "static const unsigned int {name}Offsets[{}] = {{{}}};\n",
        keys.len(),
        offsets
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    ));
    out.push_str(&format!(
        "static const signed char {name}Scores[{}] = {{{}}};\n",
        keys.len(),
        table
            .values()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    ));
    out.push_str(&format!("static const int {name}Count = {};\n", keys.len()));
    out
}

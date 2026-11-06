use crate::{Args, Result};
use serde::{
    Serialize,
    ser::{SerializeMap, Serializer},
};
use serde_json::Value;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Clone)]
struct Measurements(Vec<(String, Value)>);
impl Serialize for Measurements {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}
impl Measurements {
    fn new() -> Self {
        Self(vec![])
    }
    fn set(&mut self, k: impl Into<String>, v: impl Into<Value>) {
        self.0.push((k.into(), v.into()));
    }
    fn get(&self, k: &str) -> Option<&Value> {
        self.0.iter().find(|(key, _)| key == k).map(|(_, v)| v)
    }
}
pub fn executable(dir: &Path, name: &str) -> PathBuf {
    dir.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    })
}
pub fn output(command: &mut Command) -> Result<String> {
    let result = command.output()?;
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}
fn build_current(root: &Path) -> Result<PathBuf> {
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args([
            "build",
            "--locked",
            "--release",
            "-p",
            "sevi-test-tools",
            "--bins",
        ])
        .status()?;
    if !status.success() {
        return Err("building Rust measurement tools failed".into());
    }
    Ok(std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("release"))
}
fn build_reference(root: &Path, work: &Path, reference: &str) -> Result<PathBuf> {
    if reference.starts_with('-') {
        return Err("invalid comparison reference".into());
    }
    let repository = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !repository.status.success() {
        return Err("reference comparisons require a Git checkout".into());
    }
    let repository = PathBuf::from(String::from_utf8(repository.stdout)?.trim());
    // Archives use repository-relative paths, even when invoked inside Rust/.
    // Retain comparisons against both earlier root workspaces and upstream C++.
    let rust_layout = [
        ("Rust/", "sevi-test-tools"),
        ("Rust/", "vikey-test-tools"),
        ("", "vikey-test-tools"),
    ]
    .into_iter()
    .find(|(prefix, package)| {
        Command::new("git")
            .current_dir(&repository)
            .args([
                "cat-file",
                "-e",
                &format!("{reference}:{prefix}crates/{package}/Cargo.toml"),
            ])
            .output()
            .is_ok_and(|o| o.status.success())
    });
    let paths = match rust_layout {
        Some(("Rust/", _)) => vec!["Rust"],
        Some(_) => vec![
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates",
            "xtask",
        ],
        None => vec!["Engine", "Tools/autoswitch"],
    };
    let archive = Command::new("git")
        .current_dir(&repository)
        .args(["archive", "--format=tar", reference, "--"])
        .args(paths)
        .output()?;
    if !archive.status.success() {
        return Err("cannot archive reference engine".into());
    }
    let sources = work.join("reference");
    fs::create_dir_all(&sources)?;
    tar::Archive::new(Cursor::new(archive.stdout)).unpack(&sources)?;
    if let Some((directory, package)) = rust_layout {
        let target = work.join("reference-target");
        let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(sources.join(directory))
            .args(["build", "--locked", "--release", "-p", package, "--bins"])
            .env("CARGO_TARGET_DIR", &target)
            .status()?;
        if !status.success() {
            return Err("building archived Rust reference failed".into());
        }
        return Ok(target.join("release"));
    }
    let engine = sources.join("Engine");
    let out = work.join("refbin");
    fs::create_dir_all(&out)?;
    for (name, path, extra, testing) in [
        ("sim", "sim/sim.cpp", vec![], true),
        ("rtsim", "sim/rtsim.cpp", vec![], true),
        ("harness", "test/harness.cpp", vec![], false),
        (
            "touchsim",
            "touch/touchsim.cpp",
            vec!["TouchModel.cpp", "Driver.cpp"],
            false,
        ),
        ("bench", "driver/bench.cpp", vec!["Driver.cpp"], false),
    ] {
        let mut command = Command::new(std::env::var_os("CXX").unwrap_or_else(|| "c++".into()));
        command
            .current_dir(root)
            .args(["-std=c++17", "-O2", "-w"])
            .arg(format!("-I{}", engine.display()))
            .arg(format!("-I{}", engine.join("platforms").display()));
        if testing {
            command.arg("-DAS_TESTING");
        }
        command
            .arg("-o")
            .arg(executable(&out, name))
            .arg(sources.join("Tools/autoswitch").join(path));
        for source in
            extra
                .into_iter()
                .chain(["Engine.cpp", "Vietnamese.cpp", "SmartSwitchKey.cpp"])
        {
            command.arg(engine.join(source));
        }
        if !command.status()?.success() {
            return Err(format!("reference {name} build failed").into());
        }
    }
    Ok(out)
}
fn units(root: &Path, work: &Path, corpus: &str, typos: f64) -> Result<PathBuf> {
    let path = work.join(format!("{corpus}_{typos}.units"));
    if !path.exists() {
        let name = if corpus == "chat" {
            "chat_messages.txt"
        } else {
            "synthetic_val.txt"
        };
        let text = fs::read_to_string(root.join("Tools/autoswitch/sim/corpus").join(name))?;
        fs::write(&path, sevi_test_tools::prep::prepare(&[text], typos))?;
    }
    Ok(path)
}
fn number(text: &str, pattern: &str, group: usize) -> Result<f64> {
    regex::Regex::new(pattern)?
        .captures(text)
        .and_then(|c| c.get(group))
        .ok_or_else(|| format!("missing measurement: {pattern}"))?
        .as_str()
        .parse()
        .map_err(Into::into)
}
fn run_bin(dir: &Path, name: &str, args: &[&str]) -> Result<String> {
    output(Command::new(executable(dir, name)).args(args))
}
fn measure(root: &Path, bins: &Path, work: &Path) -> Result<Measurements> {
    let mut result = Measurements::new();
    for (corpus, typos) in [("chat", 0.), ("chat", 0.03), ("synthetic", 0.)] {
        let name = format!("{corpus}{}", if typos > 0. { "+typos" } else { "" });
        let path = units(root, work, corpus, typos)?;
        let lazy = run_bin(
            bins,
            "sim",
            &[path.to_str().ok_or("invalid unit path")?, "--lazy"],
        )?;
        let words = number(&lazy, r"words (\d+)\s+wrong words (\d+)", 1)?;
        let wrong = number(&lazy, r"words (\d+)\s+wrong words (\d+)", 2)?;
        let ideal = run_bin(bins, "sim", &[path.to_str().unwrap()])?;
        let manual = number(&ideal, r"manual switches (\d+)", 1)?;
        result.set(format!("accuracy {name}"), 100. - 100. * wrong / words);
        result.set(format!("manual switches {name}"), 1000. * manual / words);
    }
    let mut kinds = Vec::<(String, u64, u64)>::new();
    let (mut safety, mut stuck) = (0u64, 0u64);
    let pattern = regex::Regex::new(r"  ([a-z-]+) (\d+)/(\d+)")?;
    for (corpus, seeds) in [("chat", 5), ("synthetic", 2)] {
        let path = units(root, work, corpus, 0.)?;
        for seed in 1..=seeds {
            let text = run_bin(
                bins,
                "rtsim",
                &[
                    path.to_str().unwrap(),
                    "--seed",
                    &seed.to_string(),
                    "--show",
                    "0",
                ],
            )?;
            safety += (number(&text, r"guard (\d+)\s+elsewhere (\d+)\s+stuck (\d+)", 1)?
                + number(&text, r"guard (\d+)\s+elsewhere (\d+)\s+stuck (\d+)", 2)?)
                as u64;
            stuck += number(&text, r"guard (\d+)\s+elsewhere (\d+)\s+stuck (\d+)", 3)? as u64;
            for m in pattern.captures_iter(&text) {
                let a = m[2].parse::<u64>()?;
                let b = m[3].parse::<u64>()?;
                if let Some((_, old_a, old_b)) = kinds.iter_mut().find(|(k, _, _)| k == &m[1]) {
                    *old_a += a;
                    *old_b += b;
                } else {
                    kinds.push((m[1].into(), a, b));
                }
            }
        }
    }
    let (mut ops, mut manual) = (0u64, 0u64);
    for (k, a, b) in kinds {
        result.set(
            format!("slip recovery: {k}"),
            if b > 0 {
                100. - 100. * a as f64 / b as f64
            } else {
                100.
            },
        );
        ops += b;
        manual += a;
    }
    result.set(
        "slip recovery (all)",
        100. - 100. * manual as f64 / ops as f64,
    );
    result.set("text safety: bad edits", safety);
    result.set("words even a retype couldn't fix", stuck);
    let path = units(root, work, "chat", 0.)?;
    for (noise, key) in [
        ("0.0", "auto-correct: right words changed (clean)"),
        ("0.30", "auto-correct: words left wrong (sloppy)"),
    ] {
        let text = output(
            Command::new(executable(bins, "touchsim"))
                .arg(&path)
                .arg(noise)
                .env("TOUCH_CORRECT", "1"),
        )?;
        result.set(key, number(&text, r"auto-correct: ([\d.]+)% wrong", 1)?);
    }
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let text = run_bin(
            bins,
            "bench",
            &[path.to_str().unwrap(), "100000", "1000000"],
        )?;
        best = best.min(number(&text, r"avg (\d+) ns", 1)?);
    }
    result.set("speed: ns per key", best);
    let count = rule_counts(root, bins, false)?;
    result.set("locked edge cases passing", count.0);
    let count = rule_counts(root, bins, true)?;
    result.set("reported bugs fixed", format!("{}/{}", count.0, count.1));
    result.set("_reported_fixed", count.0);
    Ok(result)
}
fn rule_counts(root: &Path, bins: &Path, reported: bool) -> Result<(u64, u64)> {
    let file = root.join("Tools/autoswitch/rules").join(if reported {
        "reported.tsv"
    } else {
        "rules.tsv"
    });
    let rows = fs::read_to_string(file)?;
    let mut passed = 0;
    let mut count = 0;
    for line in rows
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        let offset = if reported { 4 } else { 1 };
        if fields.len() < offset + 4 {
            return Err("invalid rule row".into());
        }
        let keys = fields[offset + 1].replace('|', "\n");
        let text = run_bin(bins, "harness", &[fields[offset], &keys])?;
        let (got, state) = text
            .split_once("\nswitches=")
            .ok_or("invalid harness result")?;
        let lang = if state.contains("lang=VN") { "1" } else { "0" };
        if got == fields[offset + 2].replace('|', "\n") && lang == fields[offset + 3] {
            passed += 1
        }
        count += 1;
    }
    Ok((passed, count))
}
fn format(key: &str, value: &Value) -> String {
    if let Some(s) = value.as_str() {
        return s.into();
    }
    let n = value.as_f64().unwrap_or(0.);
    if key.contains("speed") {
        format!("{n:.0}")
    } else if value.is_i64()
        || value.is_u64()
        || key.starts_with("text safety")
        || key.contains("couldn't")
    {
        format!("{}", n as i64)
    } else if key.contains("manual") {
        format!("{n:.2}")
    } else {
        format!("{n:.2}%")
    }
}
pub fn run(a: &Args<'_>) -> Result {
    let work = a.root.join("target/quality").join(format!(
        "{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir_all(&work)?;
    let bins = build_current(a.root)?;
    let now = measure(a.root, &bins, &work)?;
    let reference = a.option("--compare");
    let old = reference
        .map(|reference| -> Result<Measurements> {
            let bins = build_reference(a.root, &work, reference)?;
            measure(a.root, &bins, &work)
        })
        .transpose()?;
    let mut rows = vec![if let Some(reference) = reference {
        format!("| | {reference} | now |\n|---|---:|---:|")
    } else {
        "| | now |\n|---|---:|".into()
    }];
    for (key, value) in &now.0 {
        if key.starts_with('_') {
            continue;
        }
        let Some(old) = &old else {
            rows.push(format!("| {key} | {} |", format(key, value)));
            continue;
        };
        let old_value = old.get(key);
        let mut mark = "";
        if key == "reported bugs fixed" {
            let delta = now
                .get("_reported_fixed")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                - old
                    .get("_reported_fixed")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
            mark = if delta > 0 {
                " ✅"
            } else if delta < 0 {
                " ⚠️"
            } else {
                ""
            };
        } else if let (Some(v), Some(o)) = (value.as_f64(), old_value.and_then(Value::as_f64)) {
            let noise = if key.contains("speed") {
                0.10 * o.abs()
            } else {
                1e-9
            };
            if (v - o).abs() > noise {
                let lower = [
                    "manual switches",
                    "text safety",
                    "couldn't fix",
                    "auto-correct",
                    "speed",
                ]
                .iter()
                .any(|s| key.contains(s));
                mark = if if lower { v < o } else { v > o } {
                    " ✅"
                } else {
                    " ⚠️"
                };
            }
        }
        rows.push(format!(
            "| {key} | {} | {}{mark} |",
            old_value
                .map(|v| format(key, v))
                .unwrap_or_else(|| "–".into()),
            format(key, value)
        ));
    }
    println!(
        "### Sevi quality scorecard\n\n{}\n\nPublic data only: chat corpus (21k words, plain and with 3% typos) and the synthetic val set (87k words). Realistic typing: 5 seeds on chat, 2 on synthetic. ✅ better, ⚠️ worse than the reference.",
        rows.join("\n")
    );
    if let Some(path) = a.option("--json") {
        #[derive(Serialize)]
        struct Report<'a> {
            now: &'a Measurements,
            r#ref: Option<&'a str>,
            ref_values: &'a Option<Measurements>,
        }
        let report = Report {
            now: &now,
            r#ref: reference,
            ref_values: &old,
        };
        let mut file = fs::File::create(path)?;
        let mut serializer = serde_json::Serializer::with_formatter(
            &mut file,
            serde_json::ser::PrettyFormatter::with_indent(b" "),
        );
        report.serialize(&mut serializer)?;
        file.flush()?;
    }
    Ok(())
}

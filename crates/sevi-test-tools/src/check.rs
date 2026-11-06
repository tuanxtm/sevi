use crate::{
    Harness, config_from_env, key_for, prep,
    sim::{self, Simulator},
};
use sevi_engine::*;
use std::{error::Error, fs, path::Path, process::Command};
pub fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
fn rules(file: &str, style: TelexStyle) -> Result<usize, Box<dyn Error>> {
    let text = fs::read_to_string(root().join("Tools/autoswitch/rules").join(file))?;
    let mut config = config_from_env();
    config.telex_style = style;
    let mut h = Harness::new(config);
    let mut total = 0;
    let mut passed = 0;
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let row: Vec<&str> = line.split('\t').collect();
        if row.len() < 5 {
            return Err(format!("invalid rule row: {line}").into());
        }
        total += 1;
        h.reset(if row[1] == "0" {
            Language::English
        } else {
            Language::Vietnamese
        });
        for c in row[2].chars() {
            h.press(if c == '|' { '\n' } else { c })
        }
        if h.text() == row[3].replace('|', "\n")
            && (h.engine.config.language as i32).to_string() == row[4]
        {
            passed += 1
        } else {
            eprintln!(
                "FAIL {}: keys {:?}, got {:?} lang {}, expected {:?} lang {}",
                row[0],
                row[2],
                h.text(),
                h.engine.config.language as i32,
                row[3],
                row[4]
            );
        }
    }
    println!(
        "rules{}: {passed}/{total} pass",
        if file == "rules.tsv" {
            String::new()
        } else {
            format!(" ({file})")
        }
    );
    if total != passed {
        return Err(format!("{} rules failed", total - passed).into());
    }
    Ok(total)
}
fn reported() -> Result<(), Box<dyn Error>> {
    let text = fs::read_to_string(root().join("Tools/autoswitch/rules/reported.tsv"))?;
    let (mut total, mut passed, mut regressions) = (0, 0, 0);
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let row: Vec<&str> = line.split('\t').collect();
        if row.len() < 8 {
            return Err(format!("invalid reported row: {line}").into());
        }
        let mut h = Harness::new(config_from_env());
        h.reset(if row[4] == "0" {
            Language::English
        } else {
            Language::Vietnamese
        });
        for c in row[5].chars() {
            h.press(c)
        }
        total += 1;
        if h.text() == row[6] && (h.engine.config.language as i32).to_string() == row[7] {
            passed += 1
        } else if row[3] == "fixed" {
            regressions += 1;
            eprintln!(
                "FAIL reported bug {}: {:?} -> {:?}, expected {:?}",
                row[0],
                row[5],
                h.text(),
                row[6]
            )
        }
    }
    println!("reported bugs fixed: {passed}/{total}");
    if regressions > 0 {
        return Err("reported regressions returned".into());
    }
    Ok(())
}
pub fn undo() -> Result<(), Box<dyn Error>> {
    for (keys, want) in [
        ("booj source vaay ", "bộ source vây "),
        ("booj source vaay <", "bộ source vaay"),
        ("booj source vaay <j ", "bộ source vậy "),
        ("booj source vaay < ", "bộ source vaay "),
        ("xin |hello|<s", "xin hells"),
        ("heelo!vieejt", "hêloviệt"),
    ] {
        let mut driver = Driver::default();
        driver.reset_context();
        let mut out: Vec<u16> = vec![];
        for c in keys.chars() {
            if c == '|' {
                driver.manual_switch(0.0);
                continue;
            }
            if c == '!' {
                driver
                    .engine
                    .handle_event(Event::Mouse, EventState::MouseDown, 0, 0, false);
                continue;
            }
            let (k, s) = key_for(c).unwrap();
            let e = driver.press(k, s, 0.0);
            out.truncate(out.len().saturating_sub(e.delete_count));
            out.extend(e.text);
        }
        let got = String::from_utf16_lossy(&out);
        if got != want {
            return Err(format!("undo case {keys:?}: {got:?}, expected {want:?}").into());
        }
    }
    println!("undo after fix, switch, click: ok");
    Ok(())
}
fn syllables(update: bool) -> Result<(), Box<dyn Error>> {
    let source =
        fs::read_to_string(root().join("Tools/autoswitch/rules/golden/telex_vn_mode.tsv"))?;
    let mut h = Harness::new(config_from_env());
    let mut n = 0;
    let mut updated = String::new();
    for line in source.lines() {
        let (keys, expected) = line.split_once('\t').ok_or("invalid syllable snapshot")?;
        h.reset(Language::Vietnamese);
        for c in keys.chars().chain(std::iter::once(' ')) {
            h.press(c)
        }
        updated.push_str(&format!("{keys}\t{}\n", h.line()));
        if !update && h.line() != expected {
            return Err(format!("syllable changed {keys}: {} != {expected}", h.line()).into());
        }
        n += 1
    }
    if update {
        fs::write(
            root().join("Tools/autoswitch/rules/golden/telex_vn_mode.tsv"),
            updated,
        )?;
    }
    println!("telex_vn_mode.tsv: unchanged ({n} lines)");
    Ok(())
}
fn golden(update: bool) -> Result<(), Box<dyn Error>> {
    let metric_pattern = regex::Regex::new(r"manual switches (\d+).*wrong words (\d+)")?;
    let as_root = root().join("Tools/autoswitch");
    let temp = root().join("target/verification");
    fs::create_dir_all(&temp)?;
    let mut metrics: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        as_root.join("rules/golden/metrics.json"),
    )?)?;
    let executable = std::env::current_exe()?
        .parent()
        .ok_or("missing executable directory")?
        .to_owned();
    for (corpus, rate, name) in [
        ("chat_messages.txt", 0.0, "chat"),
        ("chat_messages.txt", 0.03, "chat_typos"),
        ("synthetic_val.txt", 0.0, "synthetic"),
    ] {
        let source = fs::read_to_string(as_root.join("sim/corpus").join(corpus))?;
        let prepared = prep::prepare(&[source], rate);
        let units = sim::units(&prepared);
        let path = temp.join(format!("{name}.units"));
        fs::write(&path, &prepared)?;
        let mut lazy = Simulator::new(config_from_env());
        let mut expected = vec![];
        let mut dump = String::new();
        for u in &units {
            expected.extend(lazy.golden(u));
            lazy.type_text(&u.keys);
            lazy.type_text(&u.separators);
            if u.separators.contains('\n') {
                let trim = |text: &[u16]| {
                    String::from_utf16_lossy(if text.last() == Some(&10) {
                        &text[..text.len() - 1]
                    } else {
                        text
                    })
                };
                dump.push_str(&format!("{}\t{}\n", trim(&lazy.screen), trim(&expected)));
                lazy.screen.clear();
                expected.clear();
            }
        }
        let golden_path = as_root.join(format!("rules/golden/{name}_lazy.tsv"));
        if update {
            fs::write(&golden_path, &dump)?;
        }
        let reference = fs::read_to_string(golden_path)?;
        if dump != reference {
            fs::write(temp.join(format!("{name}.actual.tsv")), dump)?;
            return Err(format!(
                "{name} golden changed; actual saved under {}",
                temp.display()
            )
            .into());
        }
        println!("{name}_lazy.tsv: unchanged");
        let output = Command::new(executable.join(if cfg!(windows) { "sim.exe" } else { "sim" }))
            .arg(&path)
            .output()?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        let text = String::from_utf8(output.stdout)?;
        let c = metric_pattern
            .captures(&text)
            .ok_or("missing simulator metrics")?;
        let (manual, wrong) = (c[1].parse::<u64>()?, c[2].parse::<u64>()?);
        if update {
            metrics[name] = serde_json::json!({"manual":manual,"wrong":wrong});
        }
        if manual
            > metrics[name]["manual"]
                .as_u64()
                .ok_or("missing manual baseline")?
            || wrong
                > metrics[name]["wrong"]
                    .as_u64()
                    .ok_or("missing wrong baseline")?
        {
            return Err(format!("metrics worsened: {name}: {text}").into());
        }
        println!("metrics {name}: manual switches {manual}, wrong words {wrong} [ok]");
        let driver = Command::new(executable.join(if cfg!(windows) {
            "drvlazy.exe"
        } else {
            "drvlazy"
        }))
        .arg(&path)
        .env("DRV_OUT", temp.join(format!("{name}.driver.txt")))
        .output()?;
        if !driver.status.success() {
            return Err("driver simulator failed".into());
        }
        let raw = fs::read_to_string(temp.join(format!("{name}.driver.txt")))?;
        let want = reference
            .lines()
            .map(|line| line.split('\t').next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let normalize = |s: &str| s.chars().filter(|c| c.is_alphabetic()).collect::<String>();
        if normalize(&raw) != normalize(&want) {
            return Err(format!("driver != simulator: {name}").into());
        }
        println!("driver == simulator ({name}): ok");
    }
    if update {
        fs::write(
            as_root.join("rules/golden/metrics.json"),
            serde_json::to_string_pretty(&metrics)?,
        )?;
    }
    syllables(update)
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    match args.first().map(String::as_str) {
        Some("rules") => {
            let file = args.get(1).map(String::as_str).unwrap_or("rules.tsv");
            rules(file, config_from_env().telex_style)?;
            Ok(())
        }
        Some("reported") => reported(),
        Some("undo") => undo(),
        Some("golden") => golden(args.get(1).is_some_and(|s| s == "update")),
        Some("check") | None => {
            rules("rules.tsv", TelexStyle::Telex)?;
            rules("telex_simple1.tsv", TelexStyle::Simple1)?;
            rules("telex_simple2.tsv", TelexStyle::Simple2)?;
            reported()?;
            undo()?;
            golden(args.get(1).is_some_and(|s| s == "update"))?;
            Ok(())
        }
        _ => Err("usage: sevi-test-tools [check | rules [file] | reported | undo | golden]".into()),
    }
}

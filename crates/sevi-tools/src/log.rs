use crate::{Args, Result};
use serde_json::Value;
pub fn run(a: &Args<'_>) -> Result {
    let path = a
        .args
        .first()
        .ok_or("analyze-log requires typing-log.jsonl")?;
    let context = a.option("--context").unwrap_or("3").parse::<isize>()?;
    let rows: Vec<Value> = std::fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let mut counts = Vec::<(String, usize)>::new();
    let mut apps = Vec::<(String, usize)>::new();
    let s = |r: &Value, k: &str| r[k].as_str().unwrap_or("").to_owned();
    let place = |r: &Value| {
        let site = s(r, "site");
        if site.is_empty() { s(r, "app") } else { site }
    };
    fn add(v: &mut Vec<(String, usize)>, k: String) {
        if let Some((_, n)) = v.iter_mut().find(|(key, _)| *key == k) {
            *n += 1
        } else {
            v.push((k, 1));
        }
    }
    for r in &rows {
        let e = s(r, "e");
        add(
            &mut counts,
            if e == "auto" {
                format!("auto_{}", s(r, "rule"))
            } else {
                e.clone()
            },
        );
        if ["manual", "undo"].contains(&e.as_str()) {
            add(&mut apps, place(r));
        }
    }
    let count = |k: &str| {
        counts
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    };
    let words = count("word");
    println!(
        "words {words}  manual {}  undo {}  context {}",
        count("manual"),
        count("undo"),
        count("context")
    );
    println!(
        "auto: {{{}}}",
        counts
            .iter()
            .filter(|(k, _)| k.starts_with("auto_"))
            .map(|(k, v)| format!("'{k}': {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if words > 0 {
        println!(
            "manual switches per 100 words: {:.2}",
            100. * count("manual") as f64 / words as f64
        );
    }
    apps.sort_by_key(|row| std::cmp::Reverse(row.1));
    println!(
        "manual/undo by app or site: [{}]",
        apps.iter()
            .take(10)
            .map(|(k, v)| format!("('{k}', {v})"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("\n--- manual switches and undos in context ---");
    let format = |r: &Value| {
        if s(r, "e") == "word" {
            format!("{}({})", s(r, "keys"), s(r, "lang"))
        } else {
            format!("<auto {}>", s(r, "rule"))
        }
    };
    for (i, r) in rows.iter().enumerate() {
        if !["manual", "undo"].contains(&s(r, "e").as_str()) {
            continue;
        }
        let before: Vec<_> = rows[i.saturating_sub(15)..i]
            .iter()
            .filter(|r| ["word", "auto"].contains(&s(r, "e").as_str()))
            .collect();
        let start = if context > 0 {
            before.len().saturating_sub(context as usize)
        } else if context == 0 {
            0
        } else {
            context.unsigned_abs().min(before.len())
        };
        let before = before[start..]
            .iter()
            .map(|r| format(r))
            .collect::<Vec<_>>()
            .join(" ");
        let after = rows[i + 1..(i + 15).min(rows.len())]
            .iter()
            .filter(|r| s(r, "e") == "word")
            .collect::<Vec<_>>();
        let end = if context >= 0 {
            (context as usize).min(after.len())
        } else {
            after.len().saturating_sub(context.unsigned_abs())
        };
        let after = after[..end]
            .iter()
            .map(|r| format(r))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "{} {}: {before}  ==[{} -> {}]==>  {after}",
            s(r, "t").chars().skip(5).take(14).collect::<String>(),
            place(r),
            s(r, "e"),
            s(r, "to")
        );
    }
    Ok(())
}

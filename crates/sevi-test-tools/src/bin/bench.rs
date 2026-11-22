use sevi_engine::Driver;
use std::{env, error::Error, fs, hint::black_box, time::Instant};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let source = fs::read_to_string(
        args.first()
            .ok_or("usage: bench units [max_avg_ns] [max_p999_ns]")?,
    )?;
    let avg_limit: f64 = args
        .get(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(3000.0);
    let p999_limit: f64 = args
        .get(2)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(50000.0);
    let mut keys = vec![];
    for line in source.lines() {
        let text = if let Some(s) = line.strip_prefix("W\t") {
            s.split('\t').next().unwrap_or("")
        } else if line == "N" {
            "\n"
        } else {
            line.strip_prefix("S\t").unwrap_or("")
        };
        for c in text.chars() {
            if (c.is_ascii_alphabetic() || " ,.\n'".contains(c))
                && let Some(k) = sevi_test_tools::key_for(c)
            {
                keys.push((k, c))
            }
        }
    }
    let mut times = Vec::with_capacity(keys.len() * 2);
    let mut groups: [Vec<f64>; 4] = Default::default();
    let pair_guard = env::var_os("SIM_PAIR_GUARD").is_some();
    for round in 0..3 {
        let mut driver = Driver::default();
        driver.engine.set_lookback_pair_guard(pair_guard);
        for &((key, shift), c) in &keys {
            let now = Instant::now();
            black_box(driver.press(key, shift, 0.0));
            let elapsed = now.elapsed().as_nanos() as f64;
            if round > 0 {
                times.push(elapsed);
                let group = match c {
                    ' ' => 1,
                    '\n' => 3,
                    c if c.is_ascii_alphabetic() => 0,
                    _ => 2,
                };
                groups[group].push(elapsed);
            }
        }
    }
    if times.is_empty() {
        return Err("no benchmark keys".into());
    }
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(f64::total_cmp);
    let p99 = times[times.len() * 99 / 100];
    let p999 = times[times.len() * 999 / 1000];
    let max = times.last().unwrap();
    println!(
        "keys {}  avg {avg:.0} ns  p99 {p99:.0} ns  p99.9 {p999:.0} ns  max {max:.0} ns  (budget avg {avg_limit:.0}, p99.9 {p999_limit:.0})",
        times.len()
    );
    for (name, group) in ["letters", "space", "punctuation", "enter"]
        .into_iter()
        .zip(&mut groups)
    {
        if group.is_empty() {
            continue;
        }
        let avg = group.iter().sum::<f64>() / group.len() as f64;
        group.sort_unstable_by(f64::total_cmp);
        let p99 = group[group.len() * 99 / 100];
        let p999 = group[group.len() * 999 / 1000];
        println!(
            "{name:11} {:>7} keys, avg {avg:>5.0} ns, p99 {p99:>5.0} ns, p99.9 {p999:>5.0} ns",
            group.len()
        );
    }
    if avg > avg_limit || p999 > p999_limit {
        std::process::exit(1)
    }
    Ok(())
}

use sevi_engine::{Language, keys::*};
use sevi_test_tools::{Harness, config_from_env};
use std::{
    env,
    error::Error,
    io::{self, BufRead},
    time::Instant,
};
fn lang(s: &str) -> Language {
    if s == "0" || s == "en" {
        Language::English
    } else {
        Language::Vietnamese
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut h = Harness::new(config_from_env());
    match args.first().map(String::as_str) {
        Some("lines") => {
            for line in io::stdin().lock().lines() {
                let line = line?;
                let Some((l, keys)) = line.split_once('\t') else {
                    continue;
                };
                h.reset(lang(l));
                for c in keys.chars() {
                    h.press(if c == '|' { '\n' } else { c })
                }
                println!("{}", h.line());
            }
        }
        Some("frames") => {
            if args.len() < 3 {
                return Err("usage: harness frames <language> <keys>".into());
            }
            h.reset(lang(&args[1]));
            for c in args[2].chars() {
                h.press(c);
                println!("{c}\t{}\t{}", h.text(), h.engine.config.language as i32)
            }
        }
        Some("bench") => {
            let keys = [KEY_T, KEY_R, KEY_U, KEY_O, KEY_W, KEY_F, KEY_N, KEY_G];
            let start = Instant::now();
            for _ in 0..200000 {
                std::hint::black_box(h.engine.check_english_word(&keys, &[]));
            }
            println!(
                "english-word check (8 keys, replay + 2 lookups): {:.2} us/word",
                start.elapsed().as_secs_f64() * 1e6 / 200000.0
            );
        }
        Some(l) if args.len() == 2 => {
            h.reset(lang(l));
            for c in args[1].chars() {
                h.press(c)
            }
            println!(
                "{}\nswitches={} lang={}",
                h.text(),
                h.switches,
                if h.engine.config.language == Language::Vietnamese {
                    "VN"
                } else {
                    "EN"
                }
            );
        }
        _ => {
            return Err(
                "usage: harness <language> <keys> | lines | frames <language> <keys> | bench"
                    .into(),
            );
        }
    }
    Ok(())
}

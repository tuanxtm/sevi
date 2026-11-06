use sevi_engine::{Driver, Language};
use std::{env, error::Error, fs};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let source = fs::read_to_string(args.first().ok_or("usage: drvlazy units [--start en|vi]")?)?;
    let mut config = sevi_test_tools::config_from_env();
    if args.windows(2).any(|p| p == ["--start", "en"]) {
        config.language = Language::English
    }
    let mut driver = Driver::new(config);
    let mut screen: Vec<u16> = vec![];
    let mut n = 0;
    for line in source.lines() {
        let text = if let Some(rest) = line.strip_prefix("W\t") {
            rest.split('\t').next().unwrap_or("")
        } else if line == "N" {
            "\n"
        } else {
            line.strip_prefix("S\t").unwrap_or("")
        };
        for c in text.chars() {
            if let Some((k, s)) = sevi_test_tools::key_for(c) {
                let e = driver.press(k, s, 0.0);
                screen.truncate(screen.len().saturating_sub(e.delete_count));
                screen.extend(e.text);
                n += 1
            }
        }
    }
    if let Ok(path) = env::var("DRV_OUT") {
        fs::write(path, String::from_utf16_lossy(&screen))?
    }
    let mut hash: usize = 1469598103934665603u64 as usize;
    for &c in &screen {
        hash ^= c as usize;
        hash = hash.wrapping_mul(1099511628211u64 as usize)
    }
    println!("keys {n} chars {} hash {hash:x}", screen.len());
    Ok(())
}

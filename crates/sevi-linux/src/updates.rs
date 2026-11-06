//! Existing opt-in Linux release check; no update source is supplied by default.
use std::process::Command;
pub fn configured() -> bool {
    ["SEVI_RELEASE_URL", "SEVI_DOWNLOAD_URL"]
        .iter()
        .all(|key| std::env::var(key).is_ok_and(|v| v.starts_with("https://")))
}
pub fn check() -> Result<Option<String>, String> {
    if !configured() {
        return Err("No update source is configured for Sevi.".into());
    }
    let url = std::env::var("SEVI_RELEASE_URL").unwrap_or_default();
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            "--max-filesize",
            "4194304",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            &url,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Could not check for updates. Check your connection and try again.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let version = value["linux"]["version"]
        .as_str()
        .filter(|v| v.len() < 80)
        .ok_or("The release response did not contain a Linux version.")?;
    Ok(newer(version, env!("CARGO_PKG_VERSION")).then(|| version.to_owned()))
}
fn newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .map(|v| {
                v.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>()
    };
    let (mut a, mut b) = (parse(a), parse(b));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}
pub fn quiet() {
    if let Ok(Some(version)) = check() {
        let output = Command::new("notify-send")
            .args([
                "-a",
                "Sevi",
                "-i",
                "sevi",
                "-A",
                "open=Download",
                &format!("Sevi {version} is available"),
                &format!("Installed version: {}", env!("CARGO_PKG_VERSION")),
            ])
            .output();
        if output.is_ok_and(|o| o.stdout.starts_with(b"open"))
            && let Ok(url) = std::env::var("SEVI_DOWNLOAD_URL")
        {
            let _ = Command::new("xdg-open").arg(url).spawn();
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn compares_release_components() {
        assert!(super::newer("1.0.31", "1.0.30"));
        assert!(!super::newer("1.0.30.0", "1.0.30"));
        assert!(!super::newer("0.99.9", "1.0.30"));
    }
}

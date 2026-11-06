//! Existing desktop release lanes. Local checks never invoke these publishing commands.
use crate::{Args, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};
const REPO: &str = "tonyminibot/Vikey";
const IDENTITY: &str = "Developer ID Application: Typing Technology Company Limited (3H6967F79Y)";
// Reference publishing tools target the original author's infrastructure.
const LEGACY_PUBLISHING: bool = false;
fn execute(c: &mut Command) -> Result {
    let status = c.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("release command failed: {status}").into())
    }
}
fn output(c: &mut Command) -> Result<String> {
    let o = c.output()?;
    if !o.status.success() {
        return Err(format!(
            "release command failed: {}",
            String::from_utf8_lossy(&o.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(o.stdout)?.trim().into())
}
fn command(root: &Path, name: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(name);
    c.current_dir(root);
    c
}
pub fn version(root: &Path) -> Result<(String, u64)> {
    let text = fs::read_to_string(root.join("Cargo.toml"))?;
    let ver = regex::Regex::new(r#"(?m)^version = "([^"]+)""#)?
        .captures(&text)
        .ok_or("missing Cargo version")?[1]
        .to_owned();
    let build = regex::Regex::new(r"(?m)^build = (\d+)")?
        .captures(&text)
        .ok_or("missing Cargo build")?[1]
        .parse()?;
    Ok((ver, build))
}
fn sync(root: &Path, ver: &str, build: u64) -> Result {
    let file = root.join("project.yml");
    let old = fs::read_to_string(&file)?;
    let next = regex::Regex::new(r#"MARKETING_VERSION: "[^"]+""#)?
        .replace(&old, format!("MARKETING_VERSION: \"{ver}\""));
    let next = regex::Regex::new(r#"CURRENT_PROJECT_VERSION: "[^"]+""#)?
        .replace(&next, format!("CURRENT_PROJECT_VERSION: \"{build}\""));
    if next != old {
        fs::write(file, next.as_bytes())?;
    }
    Ok(())
}
pub fn check_metadata(root: &Path) -> Result {
    let (ver, build) = version(root)?;
    let text = fs::read_to_string(root.join("project.yml"))?;
    if !text.contains(&format!("MARKETING_VERSION: \"{ver}\""))
        || !text.contains(&format!("CURRENT_PROJECT_VERSION: \"{build}\""))
    {
        return Err("project.yml metadata differs from Cargo.toml; run cargo xtask tool metadata --sync before xcodegen".into());
    }
    Ok(())
}
pub fn metadata(a: &Args<'_>) -> Result {
    let (v, b) = version(a.root)?;
    if a.args.iter().any(|s| s == "--sync") {
        sync(a.root, &v, b)?;
    } else {
        check_metadata(a.root)?;
    }
    if a.args.iter().any(|s| s == "--version") {
        println!("{v}");
    } else {
        println!("{v} build {b}");
    }
    Ok(())
}
fn bump(root: &Path, ver: &str) -> Result<u64> {
    if !regex::Regex::new(r"^\d+\.\d+\.\d+$")?.is_match(ver) {
        return Err("release version must be major.minor.patch".into());
    }
    let (_, build) = version(root)?;
    let build = build + 1;
    let p = root.join("Cargo.toml");
    let old = fs::read_to_string(&p)?;
    let next = regex::Regex::new(r#"(?m)^version = "[^"]+""#)?
        .replace(&old, format!("version = \"{ver}\""));
    let next = regex::Regex::new(r"(?m)^build = \d+")?.replace(&next, format!("build = {build}"));
    fs::write(p, next.as_bytes())?;
    sync(root, ver, build)?;
    execute(
        command(
            root,
            std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        )
        .args(["update", "--workspace"]),
    )?;
    Ok(build)
}
fn json_read(root: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        root.join("site/release.json"),
    )?)?)
}
fn json_write(root: &Path, v: &Value) -> Result {
    fs::write(
        root.join("site/release.json"),
        format!("{}\n", serde_json::to_string_pretty(v)?),
    )?;
    Ok(())
}
fn sha(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn home() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME required for existing signing credentials")?)
}
fn secret(path: &Path, name: &str) -> Result<String> {
    fs::read_to_string(path)?
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{name}=")))
        .map(str::to_owned)
        .ok_or_else(|| format!("missing {name} in credential file").into())
}
fn fresh(root: &Path, name: &str) -> Result<PathBuf> {
    let path = root.join(name);
    if path.exists() {
        let resolved = path.canonicalize()?;
        if !resolved.starts_with(root.canonicalize()?) {
            return Err("release output resolves outside project".into());
        }
        let backup = root.join("target/release-previous").join(format!(
            "{}-{}",
            name.replace('/', "-"),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(backup.parent().unwrap())?;
        fs::rename(&path, backup)?;
    }
    fs::create_dir_all(&path)?;
    Ok(path)
}
fn notes(root: &Path, ver: &str) -> Result<String> {
    let text = fs::read_to_string(root.join("site/pages/changelog.html"))?;
    let start = format!("<!-- release {ver} -->");
    Ok(text
        .split_once(&start)
        .ok_or("missing release changelog")?
        .1
        .split("<!-- /release -->")
        .next()
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.contains("<h2>") && !l.contains("<!--"))
        .collect::<Vec<_>>()
        .join("\n"))
}
fn sign_update(root: &Path, path: &Path) -> Result<String> {
    output(
        command(root, home()?.join(".tools/sparkle/bin/sign_update"))
            .arg("--ed-key-file")
            .arg(home()?.join(".secrets/sevi_sparkle_ed25519"))
            .arg(path),
    )
}
fn appcast(root: &Path, args: Vec<String>) -> Result {
    let path = root.join("site/appcast.xml");
    let old = fs::read_to_string(&path).unwrap_or_default();
    fs::write(
        path,
        crate::appcast::update(
            &old,
            &args,
            &chrono::Utc::now()
                .format("%a, %d %b %Y %H:%M:%S +0000")
                .to_string(),
        )?,
    )?;
    Ok(())
}
fn download(root: &Path, ver: &str, patterns: &[String], dest: &Path) -> Result {
    let mut c = command(root, "gh");
    c.args(["release", "download", &format!("v{ver}"), "-R", REPO, "-D"])
        .arg(dest)
        .arg("--clobber");
    for p in patterns {
        c.arg("-p").arg(p);
    }
    execute(&mut c)
}
fn urls(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for v in o.values() {
                urls(v, out)
            }
        }
        Value::String(s) if s.starts_with("/downloads/") => out.push(s.clone()),
        _ => {}
    }
}
fn complete_downloads(root: &Path) -> Result {
    let mut files = vec![];
    urls(&json_read(root)?, &mut files);
    let dest = root.join("site/downloads");
    fs::create_dir_all(&dest)?;
    let re = regex::Regex::new(r"(?i)^sevi[-_](\d+(?:\.\d+)+)")?;
    for url in files {
        let name = url.rsplit('/').next().ok_or("invalid download URL")?;
        if !dest.join(name).is_file()
            && let Some(cap) = re.captures(name)
        {
            download(root, &cap[1], &[name.into()], &dest)?;
        }
    }
    Ok(())
}
fn prune(root: &Path, ver: &str, kind: &str) -> Result {
    let dest = root.join("site/downloads");
    let archive = root.join("target/release-previous/downloads");
    for entry in fs::read_dir(dest)? {
        let e = entry?;
        if !e.file_type()?.is_file() {
            continue;
        }
        let n = e.file_name().to_string_lossy().into_owned();
        let candidate = match kind {
            "mac" => n.starts_with("Sevi-") && n.ends_with(".dmg"),
            "windows" => {
                n.starts_with("Sevi-")
                    && (n.ends_with("-Setup.exe") || n.ends_with("-portable.zip"))
            }
            _ => {
                n.starts_with("sevi")
                    && (n.ends_with(".deb") || n.ends_with(".rpm") || n.ends_with(".pkg.tar.zst"))
            }
        };
        let current = match kind {
            "mac" => n == format!("Sevi-{ver}.dmg"),
            "windows" => n.contains(&format!("-{ver}-")),
            _ => n.contains(ver),
        };
        if candidate && !current {
            fs::create_dir_all(&archive)?;
            let target = archive.join(format!(
                "{}-{n}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
            ));
            fs::rename(e.path(), target)?;
        }
    }
    Ok(())
}
fn commit(root: &Path, ver: &str, lane: &str) -> Result {
    let mut c = command(root, "git");
    c.arg("add");
    if lane == "mac" {
        c.args(["Cargo.toml", "Cargo.lock", "project.yml"]);
    }
    c.arg("site/release.json");
    if lane != "linux" {
        c.arg("site/appcast.xml");
    }
    execute(&mut c)?;
    let msg = if lane == "mac" {
        format!("Release {ver}")
    } else {
        format!(
            "Release {ver} ({})",
            if lane == "windows" {
                "Windows"
            } else {
                "Linux"
            }
        )
    };
    execute(command(root, "git").args(["commit", "-q", "-m", &msg]))?;
    execute(command(root, "git").args(["push", "-q", "origin", "main"]))
}
fn wait_assets(root: &Path, ver: &str, lane: &str) -> Result {
    for _ in 0..60 {
        if let Ok(text) = output(command(root, "gh").args([
            "release",
            "view",
            &format!("v{ver}"),
            "-R",
            REPO,
            "--json",
            "assets",
        ])) {
            let v: Value = serde_json::from_str(&text)?;
            let count = v["assets"]
                .as_array()
                .ok_or("missing release assets")?
                .iter()
                .filter_map(|x| x["name"].as_str())
                .filter(|n| {
                    if lane == "windows" {
                        n.ends_with("-Setup.exe")
                    } else {
                        n.ends_with(".deb") || n.ends_with(".rpm") || n.ends_with(".pkg.tar.zst")
                    }
                })
                .count();
            if (lane == "windows" && count == 2) || (lane == "linux" && count >= 6) {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_secs(30));
    }
    Err("release packages did not arrive after 30 minutes".into())
}
pub fn deploy(a: &Args<'_>) -> Result {
    if !LEGACY_PUBLISHING {
        return Err("Legacy website publishing is disabled for Sevi; see RELEASING.md".into());
    }
    complete_downloads(a.root)?;
    crate::site::run(&Args {
        root: a.root,
        args: &[],
    })?;
    let credentials = fs::read_to_string(home()?.join(".secrets/cloudflare_dedicated"))?;
    let mut c = command(&a.root.join("site"), "npx");
    c.args([
        "--yes",
        "wrangler@latest",
        "pages",
        "deploy",
        "--branch",
        &std::env::var("BRANCH").unwrap_or_else(|_| "main".into()),
        "--commit-dirty=true",
    ]);
    for line in credentials
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let (key, value) = line
            .strip_prefix("export ")
            .unwrap_or(line)
            .split_once('=')
            .ok_or("invalid deployment credential assignment")?;
        c.env(key, value.trim_matches(['\'', '"']));
    }
    execute(&mut c)
}
pub fn run(a: &Args<'_>) -> Result {
    if !LEGACY_PUBLISHING {
        return Err("Use the GitHub release workflow for Sevi; see RELEASING.md".into());
    }
    let first = a
        .args
        .first()
        .ok_or("release <version> | --windows <version> | --linux <version>")?;
    let (lane, ver) = if first == "--windows" || first == "--linux" {
        (
            first.trim_start_matches('-'),
            a.args.get(1).ok_or("missing release version")?.as_str(),
        )
    } else {
        ("mac", first.as_str())
    };
    if lane == "mac" {
        mac(a.root, ver)?;
    } else {
        let (current, build) = version(a.root)?;
        if current != ver {
            return Err(format!("Cargo.toml is at {current}, not {ver}").into());
        }
        wait_assets(a.root, ver, lane)?;
        let dist = fresh(a.root, &format!("dist/{lane}"))?;
        let dest = a.root.join("site/downloads");
        fs::create_dir_all(&dest)?;
        let patterns = if lane == "windows" {
            vec![format!("Sevi-{ver}-*")]
        } else {
            vec!["*.deb".into(), "*.rpm".into(), "*.pkg.tar.zst".into()]
        };
        download(a.root, ver, &patterns, &dist)?;
        let mut rel = json_read(a.root)?;
        if lane == "windows" {
            let notes = notes(a.root, ver)?;
            let mut win = json!({"version":ver});
            for arch in ["x64", "arm64"] {
                let name = format!("Sevi-{ver}-{arch}-Setup.exe");
                let file = dist.join(&name);
                let verified = command(a.root, "osslsigncode")
                    .args(["verify"])
                    .arg(&file)
                    .status()?
                    .success();
                if !verified && std::env::var("ALLOW_UNSIGNED").as_deref() != Ok("1") {
                    return Err(format!("{name} is not Authenticode signed").into());
                }
                let sig = sign_update(a.root, &file)?;
                let portable = format!("Sevi-{ver}-{arch}-portable.zip");
                fs::copy(&file, dest.join(&name))?;
                fs::copy(dist.join(&portable), dest.join(&portable))?;
                appcast(
                    a.root,
                    vec![
                        "windows".into(),
                        ver.into(),
                        build.to_string(),
                        arch.into(),
                        format!("https://vikey.app/downloads/{name}"),
                        sig,
                        notes.clone(),
                    ],
                )?;
                win[arch] = json!({"download_url":format!("/downloads/{name}"),"portable_url":format!("/downloads/{portable}"),"sha256":sha(&file)?});
            }
            rel["windows"] = win;
        } else {
            let mut linux = json!({"version":ver});
            let mut files = fs::read_dir(&dist)?.collect::<std::io::Result<Vec<_>>>()?;
            files.sort_by_key(|e| e.file_name());
            for e in files {
                let n = e.file_name().to_string_lossy().into_owned();
                if !e.file_type()?.is_file() {
                    continue;
                }
                let kind = if n.ends_with(".deb") {
                    "deb"
                } else if n.ends_with(".rpm") {
                    "rpm"
                } else if n.ends_with(".pkg.tar.zst") {
                    "pacman"
                } else {
                    continue;
                };
                let arch = if n.contains("arm64") || n.contains("aarch64") {
                    "arm64"
                } else {
                    "x64"
                };
                fs::copy(e.path(), dest.join(&n))?;
                linux[format!("{kind}_{arch}")] =
                    json!({"download_url":format!("/downloads/{n}"),"sha256":sha(&e.path())?});
            }
            rel["linux"] = linux;
        }
        json_write(a.root, &rel)?;
        complete_downloads(a.root)?;
    }
    prune(a.root, ver, lane)?;
    commit(a.root, ver, lane)?;
    if lane == "mac" {
        let notes = regex::Regex::new("<[^>]*>")?
            .replace_all(&notes(a.root, ver)?.replace("<li>", "- "), "")
            .into_owned();
        let path = a.root.join("dist/release-notes.txt");
        fs::write(&path, notes)?;
        execute(
            command(a.root, "gh")
                .args(["release", "create", &format!("v{ver}")])
                .arg(a.root.join(format!("dist/Sevi-{ver}.dmg")))
                .args([
                    "-R",
                    REPO,
                    "--title",
                    &format!("Sevi {ver}"),
                    "--notes-file",
                ])
                .arg(path),
        )?;
    }
    deploy(&Args {
        root: a.root,
        args: &[],
    })?;
    println!("==> done: https://vikey.app");
    Ok(())
}
fn mac(root: &Path, ver: &str) -> Result {
    if !cfg!(target_os = "macos") {
        return Err("the macOS release lane requires macOS signing/build tools".into());
    }
    for file in ["site/pages/changelog.html", "site/pages_en/changelog.html"] {
        if !fs::read_to_string(root.join(file))?.contains(&format!("<!-- release {ver} -->")) {
            return Err(format!("missing <!-- release {ver} --> in {file}").into());
        }
    }
    let build = bump(root, ver)?;
    execute(command(root, "xcodegen").arg("generate"))?;
    execute(
        command(
            root,
            std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()),
        )
        .args(["xtask", "check"]),
    )?;
    let dd = fresh(root, "build/dd")?;
    let dist = fresh(root, "dist")?;
    execute(
        command(root, "xcodebuild")
            .args([
                "-project",
                "Sevi.xcodeproj",
                "-scheme",
                "Sevi",
                "-configuration",
                "Release",
                "-derivedDataPath",
            ])
            .arg(&dd)
            .args([
                "ARCHS=arm64 x86_64",
                "ONLY_ACTIVE_ARCH=NO",
                "CODE_SIGN_IDENTITY=-",
                "CODE_SIGNING_REQUIRED=NO",
                "build",
            ]),
    )?;
    let app = dist.join("Sevi.app");
    execute(
        command(root, "ditto")
            .arg(dd.join("Build/Products/Release/Sevi.app"))
            .arg(&app),
    )?;
    let built = output(
        command(root, "defaults")
            .arg("read")
            .arg(app.join("Contents/Info.plist"))
            .arg("CFBundleShortVersionString"),
    )?;
    if built != ver {
        return Err("built version mismatch".into());
    }
    let archs = output(
        command(root, "lipo")
            .arg("-archs")
            .arg(app.join("Contents/MacOS/Sevi")),
    )?;
    if !archs.contains("arm64") || !archs.contains("x86_64") {
        return Err("built application is not universal".into());
    }
    let kc = home()?.join("Library/Keychains/s10w-signing.keychain-db");
    let pass = secret(&home()?.join(".secrets/s10w_signing"), "KC_PASS")?;
    execute(
        command(root, "security")
            .args(["unlock-keychain", "-p", &pass])
            .arg(kc),
    )?;
    let framework = app.join("Contents/Frameworks/Sparkle.framework");
    let sparkle = framework.join("Versions/B");
    for f in [
        "XPCServices/Installer.xpc",
        "XPCServices/Downloader.xpc",
        "Autoupdate",
        "Updater.app",
    ] {
        let p = sparkle.join(f);
        if p.exists() {
            execute(
                command(root, "codesign")
                    .args([
                        "--force",
                        "--options",
                        "runtime",
                        "--timestamp",
                        "--preserve-metadata=entitlements",
                        "-s",
                        IDENTITY,
                    ])
                    .arg(p),
            )?;
        }
    }
    execute(
        command(root, "codesign")
            .args([
                "--force",
                "--options",
                "runtime",
                "--timestamp",
                "-s",
                IDENTITY,
            ])
            .arg(framework),
    )?;
    execute(
        command(root, "codesign")
            .args([
                "--force",
                "--options",
                "runtime",
                "--timestamp",
                "--entitlements",
                "App/Sevi.entitlements",
                "-s",
                IDENTITY,
            ])
            .arg(&app),
    )?;
    execute(
        command(root, "codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&app),
    )?;
    let zip = dist.join("Sevi.zip");
    execute(
        command(root, "ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(&app)
            .arg(&zip),
    )?;
    let notarize = |p: &Path| -> Result {
        execute(
            command(root, "xcrun")
                .args(["notarytool", "submit"])
                .arg(p)
                .arg("--key")
                .arg(home()?.join(".secrets/AuthKey_3QNAF6H8AT.p8"))
                .args([
                    "--key-id",
                    "3QNAF6H8AT",
                    "--issuer",
                    "6587ae6d-4945-468e-8879-d58b7a6d33ad",
                    "--wait",
                ]),
        )
    };
    notarize(&zip)?;
    execute(command(root, "xcrun").args(["stapler", "staple"]).arg(&app))?;
    let bg = dist.join("dmg_bg.png");
    crate::images::dmg(&Args {
        root,
        args: &[bg.to_string_lossy().into_owned()],
    })?;
    let dmg = dist.join(format!("Sevi-{ver}.dmg"));
    let mut c = command(root, "create-dmg");
    c.args(["--volname", "Sevi"]);
    let icon = app.join("Contents/Resources/AppIcon.icns");
    if icon.is_file() {
        c.arg("--volicon").arg(icon);
    }
    c.arg("--background")
        .arg(bg)
        .args([
            "--window-pos",
            "200",
            "120",
            "--window-size",
            "660",
            "420",
            "--icon-size",
            "128",
            "--icon",
            "Sevi.app",
            "170",
            "190",
            "--app-drop-link",
            "490",
            "190",
            "--hide-extension",
            "Sevi.app",
            "--no-internet-enable",
            "--codesign",
            IDENTITY,
        ])
        .arg(&dmg)
        .arg(&app);
    execute(&mut c)?;
    notarize(&dmg)?;
    execute(command(root, "xcrun").args(["stapler", "staple"]).arg(&dmg))?;
    execute(
        command(root, "spctl")
            .args([
                "-a",
                "-vv",
                "-t",
                "open",
                "--context",
                "context:primary-signature",
            ])
            .arg(&dmg),
    )?;
    let dest = root.join("site/downloads");
    fs::create_dir_all(&dest)?;
    fs::copy(&dmg, dest.join(dmg.file_name().unwrap()))?;
    let mut rel = json_read(root)?;
    rel["version"] = ver.into();
    rel["download_url"] = format!("/downloads/Sevi-{ver}.dmg").into();
    rel["sha256"] = sha(&dmg)?.into();
    json_write(root, &rel)?;
    appcast(
        root,
        vec![
            "mac".into(),
            ver.into(),
            build.to_string(),
            format!("https://vikey.app/downloads/Vikey-{ver}.dmg"),
            sign_update(root, &dmg)?,
            notes(root, ver)?,
        ],
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_publishing_is_disabled_before_accessing_credentials_or_services() {
        let args = Args {
            root: Path::new("."),
            args: &[],
        };
        assert!(
            run(&args)
                .unwrap_err()
                .to_string()
                .contains("GitHub release")
        );
        assert!(deploy(&args).unwrap_err().to_string().contains("disabled"));
    }
    #[test]
    fn nested_download_urls_preserve_platforms() {
        let mut out = vec![];
        urls(
            &json!({"download_url":"/downloads/Sevi-1.0.0.dmg","windows":{"x64":{"download_url":"/downloads/Sevi-1.0.0-x64-Setup.exe"}},"ignored":"https://example.com"}),
            &mut out,
        );
        assert_eq!(out.len(), 2);
    }
    #[test]
    fn metadata_matches_workspace() {
        check_metadata(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .as_path(),
        )
        .unwrap();
    }
}

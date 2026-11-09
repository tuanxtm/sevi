use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn run(cmd: &mut Command) -> Result {
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("{cmd:?} failed: {status}").into());
    }
    Ok(())
}
fn main() -> Result {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("tool") => sevi_tools::run(&root(), &args[1..]),
        Some("check-data") => sevi_tools::check_data(&root()),
        Some("lock-data") => sevi_tools::lock_data(&root()),
        Some("check-sources") => sevi_tools::check_sources(&root()),
        Some("build-macos") => build_macos(),
        Some("check") => {
            let update = match args.get(1).map(String::as_str) {
                None | Some("check") => false,
                Some("update") => true,
                _ => return Err("usage: cargo xtask check [check|update]".into()),
            };
            check(update)
        }
        _ => Err("usage: cargo xtask check [check|update] | check-data | lock-data | check-sources | tool <command> | build-macos".into()),
    }
}

fn build_macos() -> Result {
    sevi_tools::check_metadata(&root())?;
    let arches = env::var("ARCHS").map_err(|_| "Xcode must provide ARCHS")?;
    let output = PathBuf::from(
        env::var("BUILT_PRODUCTS_DIR").map_err(|_| "Xcode must provide BUILT_PRODUCTS_DIR")?,
    );
    let mut libraries = Vec::new();
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let debug = env::var("CONFIGURATION").is_ok_and(|v| v == "Debug");
    for arch in arches.split_whitespace() {
        let target = match arch {
            "arm64" => "aarch64-apple-darwin",
            "x86_64" => "x86_64-apple-darwin",
            _ => return Err(format!("unsupported Mac architecture: {arch}").into()),
        };
        let mut command = Command::new(&cargo);
        command
            .args([
                "build",
                "--locked",
                "--package",
                "sevi-macos",
                "--lib",
                "--target",
                target,
            ])
            .arg("--manifest-path")
            .arg(root().join("Cargo.toml"))
            .env("MACOSX_DEPLOYMENT_TARGET", "13.0");
        if !debug {
            command.arg("--release");
        }
        run(&mut command)?;
        let target_dir = env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root().join("target"));
        libraries.push(
            target_dir
                .join(target)
                .join(if debug { "debug" } else { "release" })
                .join("libsevi_macos.a"),
        );
    }
    if libraries.is_empty() {
        return Err("ARCHS contains no architectures".into());
    }
    fs::create_dir_all(&output)?;
    run(Command::new("lipo")
        .arg("-create")
        .args(libraries)
        .arg("-output")
        .arg(output.join("libsevi_macos.a")))
}

fn check(update: bool) -> Result {
    sevi_tools::check_data(&root())?;
    sevi_tools::check_sources(&root())?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(Command::new(&cargo).current_dir(root()).args([
        "test",
        "--locked",
        "--package",
        "sevi-engine",
        "--package",
        "sevi-runtime",
        "--package",
        "sevi-ui",
        "--package",
        "sevi-ui-protocol",
        "--package",
        "sevi-windows",
        "--package",
        "sevi-macos",
        "--package",
        "sevi-tools",
        "--package",
        "sevi-test-tools",
        "--package",
        "xtask",
        "--release",
        "--all-features",
    ]))?;
    run(Command::new(&cargo).current_dir(root()).args([
        "build",
        "--locked",
        "--release",
        "--package",
        "sevi-test-tools",
        "--bins",
    ]))?;
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target"));
    let bins = target_dir.join("release");
    let program = |name: &str| {
        bins.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        })
    };
    let mut command = Command::new(program("sevi-test-tools"));
    command.arg("check");
    if update {
        command.arg("update");
    }
    run(&mut command)?;
    let units = root().join("target/verification/chat.units");
    let mut perf_ok = false;
    for _ in 0..3 {
        if Command::new(program("bench"))
            .arg(&units)
            .args(["1000", "20000"])
            .status()?
            .success()
        {
            perf_ok = true;
            break;
        }
    }
    if !perf_ok {
        return Err("performance budget exceeded".into());
    }
    let baseline: serde_json::Value = serde_json::from_str(&fs::read_to_string(
        root().join("Tools/autoswitch/rules/golden/rtsim.json"),
    )?)?;
    let mut totals = std::collections::BTreeMap::<String, u64>::new();
    for (corpus, seeds) in [("chat", vec![1, 2, 3]), ("synthetic", vec![1])] {
        for seed in seeds {
            let output = Command::new(program("rtsim"))
                .arg(root().join(format!("target/verification/{corpus}.units")))
                .args(["--seed", &seed.to_string(), "--show", "3"])
                .output()?;
            let stdout = String::from_utf8(output.stdout)?;
            print!("{stdout}");
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            // rtsim exits 1 for known stuck cases too. The existing check locks
            // their count separately; only guard/elsewhere violations must be zero.
            let mut safety_metrics = 0;
            for pair in stdout.split_whitespace().collect::<Vec<_>>().windows(2) {
                if pair[0] == "guard" || pair[0] == "elsewhere" {
                    safety_metrics += 1;
                    if pair[1].parse::<u64>()? != 0 {
                        return Err("realistic typing text-safety violation".into());
                    }
                }
                if pair[0] == "stuck" {
                    *totals.entry("stuck".into()).or_default() += pair[1].parse::<u64>()?;
                }
                if let Some((count, _)) = pair[1].split_once('/')
                    && let Ok(count) = count.parse::<u64>()
                {
                    *totals.entry(pair[0].to_owned()).or_default() += count
                }
            }
            if safety_metrics != 2 || !matches!(output.status.code(), Some(0 | 1)) {
                return Err("realistic typing simulator failed".into());
            }
        }
    }
    if update {
        fs::write(
            root().join("Tools/autoswitch/rules/golden/rtsim.json"),
            serde_json::to_string_pretty(&totals)?,
        )?;
    }
    for (name, count) in &totals {
        if let Some(old) = baseline[name].as_u64()
            && !update
            && *count as f64 > old as f64 * 1.05 + 3.0
        {
            return Err(format!("realistic typing regression: {name}: {old} -> {count}").into());
        }
    }
    for noise in ["0.0", "0.30"] {
        let output = Command::new(program("touchsim"))
            .arg(&units)
            .arg(noise)
            .env("TOUCH_CORRECT", "1")
            .output()?;
        let text = String::from_utf8(output.stdout)?;
        print!("{text}");
        if !output.status.success() {
            return Err("touch evaluation failed".into());
        }
        let number = |prefix: &str, suffix: &str| -> Result<f64> {
            Ok(text
                .split_once(prefix)
                .ok_or("missing touch metric")?
                .1
                .split_once(suffix)
                .ok_or("missing touch metric terminator")?
                .0
                .trim()
                .parse()?)
        };
        let corrected = number("auto-correct:", "%")?;
        if noise == "0.0" {
            if corrected > 0.10 {
                return Err("auto-correct changed clean typing".into());
            }
        } else {
            let tail = text
                .split_once("wrong, model ")
                .ok_or("missing model word error")?
                .1;
            let model: f64 = tail.split('%').next().unwrap().parse()?;
            let near_tail = text
                .split_once(" | words ")
                .ok_or("missing nearest word error")?
                .1;
            let nearest: f64 = near_tail
                .split_once("nearest ")
                .ok_or("missing nearest word error")?
                .1
                .split('%')
                .next()
                .unwrap()
                .parse()?;
            let ns = number("pick:", "ns")?;
            if model > 15.0 || model >= nearest * 0.6 || ns >= 20000.0 || corrected >= model * 0.75
            {
                return Err("touch/auto-correct budget exceeded".into());
            }
        }
    }
    println!("ALL RUST CHECKS PASSED");
    Ok(())
}

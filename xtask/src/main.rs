use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
mod import_data;

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
fn copy_tree(from: &Path, to: &Path) -> Result {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("build" | "dist" | "__pycache__")
            ) {
                copy_tree(&path, &to.join(entry.file_name()))?;
            }
        } else if entry.file_type()?.is_file() {
            fs::copy(&path, to.join(entry.file_name()))?;
        } else {
            return Err(format!("refusing to copy symlink {}", path.display()).into());
        }
    }
    Ok(())
}
fn legacy_build() -> Result<PathBuf> {
    let root = root();
    let dest = root.join("target/legacy");
    for dir in ["Engine", "Tools/autoswitch"] {
        copy_tree(&root.join(dir), &dest.join(dir))?;
    }
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()?;
    let paths = Command::new("git")
        .args([
            "ls-files",
            "Engine",
            "Tools/autoswitch",
            "App",
            "Linux",
            "Windows",
        ])
        .current_dir(&root)
        .output()?;
    let mut hashes = String::from_utf8(revision.stdout)?;
    for path in String::from_utf8(paths.stdout)?.lines() {
        if root.join(path).is_file() {
            let output = Command::new("git")
                .args(["hash-object", "--", path])
                .current_dir(&root)
                .output()?;
            hashes.push_str(&format!(
                "{} {path}\n",
                String::from_utf8(output.stdout)?.trim()
            ));
        }
    }
    fs::write(dest.join("baseline.txt"), hashes)?;
    let engine = dest.join("Engine");
    let tools = dest.join("Tools/autoswitch");
    let cc = env::var("CXX").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "clang++"
        } else {
            "g++"
        }
        .into()
    });
    for (name, file, driver, touch, testing) in [
        ("test/harness", "test/harness.cpp", false, false, false),
        ("sim/sim", "sim/sim.cpp", false, false, true),
        ("sim/rtsim", "sim/rtsim.cpp", false, false, true),
        ("driver/drvlazy", "driver/drvlazy.cpp", true, false, false),
        ("driver/bench", "driver/bench.cpp", true, false, false),
        (
            "driver/undo_test",
            "driver/undo_test.cpp",
            true,
            false,
            false,
        ),
        ("touch/touchsim", "touch/touchsim.cpp", true, true, false),
        ("touch/typoeval", "touch/typoeval.cpp", false, true, false),
        ("touch/normeval", "touch/normeval.cpp", false, true, false),
    ] {
        println!("reference: {name}");
        let mut cmd = Command::new(&cc);
        cmd.args(["-std=c++17", "-O2", "-w", "-I"])
            .arg(&engine)
            .arg(tools.join(file));
        for file in ["Engine.cpp", "Vietnamese.cpp", "SmartSwitchKey.cpp"] {
            cmd.arg(engine.join(file));
        }
        if driver {
            cmd.arg(engine.join("Driver.cpp"));
        }
        if touch {
            cmd.arg(engine.join("TouchModel.cpp"));
        }
        if testing {
            cmd.arg("-DAS_TESTING");
        }
        run(cmd.arg("-o").arg(tools.join(name)))?;
    }
    Ok(dest)
}
fn legacy_check() -> Result {
    let dest = legacy_build()?;
    let tools = dest.join("Tools/autoswitch");
    let tmp = dest.join("tmp");
    fs::create_dir_all(&tmp)?;
    let mut failed = Vec::new();
    for (script, arg, style) in [
        ("rules/run_rules.py", None, "0"),
        ("rules/run_rules.py", Some("telex_simple1.tsv"), "1"),
        ("rules/run_rules.py", Some("telex_simple2.tsv"), "2"),
        ("rules/run_reported.py", None, "0"),
        ("rules/golden.py", Some("check"), "0"),
    ] {
        let mut cmd = Command::new("python3");
        cmd.arg(script);
        if let Some(arg) = arg {
            cmd.arg(arg);
        }
        if !cmd
            .current_dir(&tools)
            .env("SEVI_TELEX_STYLE", style)
            .env_remove("SEVI_AUTO_OFF")
            .env_remove("SEVI_TEENCODE")
            .env("TMPDIR", &tmp)
            .status()?
            .success()
        {
            failed.push(script);
        }
    }
    run(&mut Command::new(tools.join("driver/undo_test")))?;
    if !failed.is_empty() {
        return Err(format!("reference checks failed: {failed:?}").into());
    }
    println!(
        "Reference checks passed. Sources and outputs are isolated in {}",
        dest.display()
    );
    Ok(())
}
fn main() -> Result {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("tool") => {
            sevi_tools::run(&root(), &args[1..])?;
            if args.get(1).is_some_and(|s| {
                matches!(
                    s.as_str(),
                    "build-dict" | "build-bigrams" | "build-teencode" | "keepgen"
                )
            }) && !args.iter().any(|s| s == "--output")
            {
                import_data::run(false)?;
            }
            Ok(())
        }
        Some("import-legacy-data") => import_data::run(args.get(1).is_some_and(|s| s == "--check")),
        Some("build-macos") => build_macos(),
        Some("check") => {
            let update = match args.get(1).map(String::as_str) {
                None | Some("check") => false,
                Some("update") => true,
                _ => return Err("usage: cargo xtask check [check|update]".into()),
            };
            check(update)
        }
        Some("baseline") => {
            if args.get(1).map(String::as_str) == Some("check") {
                legacy_check()
            } else {
                legacy_build().map(|_| ())
            }
        }
        Some("legacy-run") => {
            let binary = args.get(1).ok_or("legacy-run requires a binary")?;
            if Path::new(binary).components().any(|p| {
                matches!(
                    p,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                return Err("invalid binary path".into());
            }
            run(
                Command::new(root().join("target/legacy/Tools/autoswitch").join(binary))
                    .args(&args[2..])
                    .stdin(Stdio::inherit()),
            )
        }
        _ => Err("usage: cargo xtask baseline [check] | legacy-run <binary> [args]".into()),
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
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(Command::new(&cargo).current_dir(root()).args([
        "test",
        "--locked",
        "--package",
        "sevi-engine",
        "--package",
        "sevi-runtime",
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

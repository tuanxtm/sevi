use std::{env, fs, path::PathBuf};
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap();
    for source in [
        "Windows/App/Sevi.rc",
        "Windows/App/Sevi.ico",
        "Windows/App/tray-v.ico",
        "Windows/App/tray-e.ico",
        "Windows/App/Sevi.manifest",
        "Windows/App/resource.h",
        "Windows/App/Sounds/SwitchToVietnamese.wav",
        "Windows/App/Sounds/SwitchToEnglish.wav",
        "App/Resources/Emoji.json",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }
    // Resources belong to the target executable, including LLVM cross-builds.
    if !env::var("TARGET").unwrap_or_default().contains("windows") {
        return;
    }
    if env::var("TARGET").unwrap_or_default().ends_with("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    // Rust embeds these bytes through include_str!/include_bytes!. Keep the
    // compatibility RC unchanged, but omit its unused second copies in Rust EXEs.
    let source = fs::read_to_string(root.join("Windows/App/Sevi.rc"))
        .unwrap()
        .lines()
        .filter(|line| {
            !matches!(
                line.split_whitespace().next(),
                Some("IDR_SOUND_VI" | "IDR_SOUND_EN" | "IDR_EMOJI_JSON")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let files = [
        ("resource.h", "Windows/App/resource.h"),
        ("Sevi.ico", "Windows/App/Sevi.ico"),
        ("tray-v.ico", "Windows/App/tray-v.ico"),
        ("tray-e.ico", "Windows/App/tray-e.ico"),
        ("Sevi.manifest", "Windows/App/Sevi.manifest"),
        (
            "Sounds/SwitchToVietnamese.wav",
            "Windows/App/Sounds/SwitchToVietnamese.wav",
        ),
        (
            "Sounds/SwitchToEnglish.wav",
            "Windows/App/Sounds/SwitchToEnglish.wav",
        ),
        ("../../App/Resources/Emoji.json", "App/Resources/Emoji.json"),
    ];
    let mut source = source;
    for (from, to) in files {
        source = source.replace(
            &format!("\"{from}\""),
            &format!(
                "\"{}\"",
                root.join(to).display().to_string().replace('\\', "/")
            ),
        );
    }
    source = source
        .replace(
            "FILEVERSION 1,0,14,0",
            &format!("FILEVERSION {},0", version.replace('.', ",")),
        )
        .replace(
            "PRODUCTVERSION 1,0,14,0",
            &format!("PRODUCTVERSION {},0", version.replace('.', ",")),
        );
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("Sevi.rc");
    fs::write(
        &output,
        format!("#pragma code_page(65001)\n#define SEVI_VERSION \"{version}\"\n{source}"),
    )
    .unwrap();
    embed_resource::compile(output, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}

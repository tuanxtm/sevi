# Sevi Rust desktop port

Vietnamese typing for macOS, Windows and Linux that switches between Vietnamese and English for you.

This repository is the complete Rust workspace. Run Cargo commands from its root.
In the original upstream checkout, the same workspace is inside `Rust/`.
Builds use the workspace's own resources, fixtures and reference code.

| Folder/files | Purpose |
| --- | --- |
| `crates/` | Rust engine, shared runtime, native desktop apps and support tools |
| `xtask/`, `.cargo/`, `Cargo.*`, `rust-toolchain.toml` | Cargo workspace and development commands |
| `App/`, `Linux/`, `Windows/` | Desktop resources, packaging, native fixtures and reference hosts |
| `Engine/`, `Tools/autoswitch/` | Copied reference engine/data and regression corpora |
| `cmake/`, `project.yml` | CMake integration and macOS project generation |
| `site/`, `site-shots/`, `Tools/` | Inputs for the existing Rust support/release tools |
| `target/` | Ignored generated builds, checks and test executables |

The original desktop sources are retained only for reference/compatibility;
the default desktop implementations are the Rust crates. iOS, Android and their
exclusive tools are excluded. GitHub workflows are in `.github/workflows/`.

Sevi is a Rust port of [Vikey](https://github.com/tonyminibot/Vikey), originally made by [Tony Dinh](https://tonydinh.com) (Đinh Quang Trung, [@trungdq88](https://github.com/trungdq88)). Free and open source under GPL-3.0; based on [OpenKey](https://github.com/tuyenvm/OpenKey) by Tuyen Mai.

Type Telex as usual. When you type an English word in Vietnamese mode, or a Vietnamese word in English mode, Sevi fixes the word and switches the language. No more pressing the switch key every few words.

## Features

- **Telex** with modern or classic tone placement (hoà / hòa), spell checking, and keeping your keys as typed when a word isn't Vietnamese.
- **Auto switch:**
  - `more` → stays English;
  - `treen` → `trên`;
  - `truowfng`, `truongwf`, `truwownfg` → `trường`;
  - `this makes sense` comes out right in Vietnamese mode, because ambiguous words are fixed once the context is clear.
- **Programming-aware:**
  - `var`, `const`, `git push`, `obj.dir` and `my_var` are left alone.
- **Undo:** press Delete right after an automatic fix to undo it.
- **Remembers the language** for each app, and for each website in Chrome, Safari, Arc, Brave, Edge, Vivaldi and Opera.
- **Switch hotkey:** a key with modifiers, or modifiers alone (⌃⇧), with an optional beep.
- **Works with Dvorak and other layouts:** Sevi steps aside automatically for other input languages (Japanese, Chinese…).

## App identity

The application is named **Sevi** on Windows, macOS and Linux. Cargo packages
use the `sevi-` prefix. Windows builds `sevi.exe`; Linux builds
`ibus-engine-sevi` and `sevi-settings`; macOS bundles `Sevi.app`.

Sevi uses its own settings and log locations: the Sevi registry/AppData entries
on Windows, sevi configuration/data directories on Linux, and Sevi preferences
and Application Support on macOS. Existing Vikey settings are preserved and
are not automatically migrated. On Linux, add **Vietnamese (Sevi)** to your input
sources; on macOS, grant Sevi Accessibility permission. Exit the old IME before
starting Sevi.

Upstream attribution and historical design decisions remain in the project
documentation. The app uses Sevi branding and the supplied Sevi icons.
Windows x64 releases and signed automatic updates are published through
[GitHub Releases](https://github.com/tuanxtm/sevi/releases). See
[RELEASING.md](RELEASING.md) for Git pushes, version tags, signing-key backups,
the initial manual installation and platform-specific update support.

## Build

All desktop windows use the shared `sevi-ui` crate (egui/eframe). Settings fit on
one page, use English labels and checkboxes, follow the system light/dark theme,
and scale in logical pixels. Help, About and credits share that window. Closing
the last window exits the renderer helper; the native IME keeps running. The
parent owns preferences and insertion and communicates through private anonymous
pipes. Settings and the separate emoji popup each use an on-demand helper, so
closing emoji preserves an already-open settings window. Windows retains its nonactivating picker; macOS renders the same picker
inside a nonactivating NSPanel. Linux retains IBus commit-on-focus behavior.

Preview the shared UI without starting an IME:

```sh
cargo run --locked --release -p sevi-ui --bin sevi-ui-preview
```

Windows and Linux use egui's smaller Glow/OpenGL renderer; macOS keeps Metal with
a minimal WGPU backend for its nonactivating NSPanel. The UI/graphics dependencies
are optimized for size; the input engine and hooks keep speed optimization.
Linux production UI needs Pango/Cairo for system emoji glyphs and an OpenGL driver.
GTK/libadwaita are used only by the retained native test fixtures,
enabled with `--features native-tests`; the production settings UI uses egui.
See [UI validation](UI_VALIDATION.md) for checks and native acceptance steps.

Windows x64:

```sh
cargo build --locked --release -p sevi-windows --bins --target x86_64-pc-windows-msvc
```

The project Cargo configuration selects the static MSVC runtime, so the Windows
executable does not need the Visual C++ Redistributable. CMake uses the same configuration.

Linux desktop/packaging:

```sh
cmake -S Linux -B target/linux-build -DCMAKE_BUILD_TYPE=Release
cmake --build target/linux-build
```

macOS:

Requires Rust 1.95.0, Xcode and [XcodeGen](https://github.com/yonaskolb/XcodeGen). Xcode builds the Rust desktop host and bundles the existing resources and Sparkle updater. Windows and Linux also build Rust hosts through their existing CMake entrypoints. Native acceptance is tracked in [Rust migration status](RUST_MIGRATION.md). Existing support commands are documented in [Tools](Tools/README.md).

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask tool metadata --sync
xcodegen generate
xcodebuild -project Sevi.xcodeproj -scheme Sevi -configuration Release build
```

Sevi opens settings directly without a first-launch welcome wizard. On macOS, use the Accessibility permission button in settings to allow reading and rewriting keystrokes. The typing guide remains available from the menu.

## Engine tests

Every typing rule is locked down by tests: rule-tagged cases, golden snapshots of typing a mixed Vietnamese/English corpus, and every Vietnamese syllable.

```sh
cargo xtask check
```

See [DECISIONS.md](DECISIONS.md) for every rule and why it exists.

## Contributing

Pull requests are welcome, AI-written code included. Show the problem with a screenshot or video, describe the
before and after, and add test cases. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License and credits

Sevi is based on [OpenKey](https://github.com/tuyenvm/OpenKey) by Tuyen Mai, and is distributed under the GNU General Public License v3.0. See [LICENSE](LICENSE).

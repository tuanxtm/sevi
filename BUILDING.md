# Building Sevi

This repository contains the complete Rust workspace. Run the commands below
from its root, with the Rust toolchain pinned in `rust-toolchain.toml` (1.95.0).
In the original upstream checkout, the same workspace is inside `Rust/`.
Builds use the workspace's own resources, fixtures and reference code.

## Workspace

| Folder/files | Purpose |
| --- | --- |
| `crates/` | Rust engine, shared runtime, native desktop apps and support tools |
| `xtask/`, `.cargo/`, `Cargo.*`, `rust-toolchain.toml` | Cargo workspace and development commands |
| `App/`, `Linux/`, `Windows/` | Desktop resources, packaging, native fixtures and reference hosts |
| `Engine/`, `Tools/autoswitch/` | Reference engine/data and regression corpora |
| `cmake/`, `project.yml` | CMake integration and macOS project generation |
| `site/`, `site-shots/`, `Tools/` | Inputs for support tools and retained publishing references |
| `target/` | Ignored generated builds, checks and test executables |

The original desktop sources are retained for reference and compatibility;
the default desktop implementations are the Rust crates. Mobile apps are excluded.
GitHub workflows live in `.github/workflows/`. Legacy upstream publishing commands
are disabled; use [RELEASING.md](RELEASING.md) for Sevi releases and updates.

## Shared UI

Preview settings without starting an input method:

```sh
cargo run --locked --release -p sevi-ui --bin sevi-ui-preview
```

All desktop windows use `sevi-ui` (egui/eframe). Settings, Help, About and Credits
share one window. Settings and the separate emoji popup each use an on-demand
helper; closing the last window exits that helper while the native IME keeps
running. The parent owns preferences and text insertion and communicates through
private anonymous pipes.

Windows and Linux use Glow/OpenGL. macOS uses Metal through a minimal WGPU backend
for its nonactivating NSPanel; Windows keeps its nonactivating emoji picker and
Linux keeps IBus commit-on-focus behavior. UI/graphics dependencies are optimized
for size; the engine and hooks retain speed optimization.
See [UI validation](UI_VALIDATION.md) for native acceptance checks.

## Windows x64

Use Windows with the MSVC C++ build tools and Windows SDK installed:

```sh
cargo build --locked --release -p sevi-windows --bins --target x86_64-pc-windows-msvc
```

The output is `target/x86_64-pc-windows-msvc/release/sevi.exe`.
The project Cargo configuration selects the static MSVC runtime, so the executable
does not need the Visual C++ Redistributable. CMake uses the same configuration.

For the installer, signing and GitHub workflow, see [RELEASING.md](RELEASING.md).

## Linux

The production host needs IBus; the shared UI needs Pango/Cairo for system emoji
glyphs and an OpenGL driver. On Ubuntu, the CI build dependencies are:

```sh
sudo apt-get install cmake pkg-config libibus-1.0-dev libpango1.0-dev libcairo2-dev libgstreamer1.0-dev libegl1-mesa-dev libxkbcommon-dev libwayland-dev libx11-dev libssl-dev
cmake -S Linux -B target/linux-build -DCMAKE_BUILD_TYPE=Release
cmake --build target/linux-build
```

GTK/libadwaita are used only by retained native test fixtures, enabled with
`--features native-tests`. The production settings UI uses egui.
After installing and registering the IBus engine, add **Vietnamese (Sevi)**
to your input sources.

## macOS

Requires Xcode and [XcodeGen](https://github.com/yonaskolb/XcodeGen):

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo xtask tool metadata --sync
xcodegen generate
xcodebuild -project Sevi.xcodeproj -scheme Sevi -configuration Release build
```

Xcode builds the Rust host and bundles the resources and Sparkle updater.
Sparkle updates remain disabled until Mac release signing and distribution are
configured. Grant Sevi Accessibility permission using the button in settings.

## Settings and app identity

Windows builds `sevi.exe`; Linux builds `ibus-engine-sevi` and `sevi-settings`;
macOS bundles `Sevi.app`. Cargo packages use the `sevi-` prefix.

Sevi uses its own registry/AppData entries on Windows, configuration/data
directories on Linux, and preferences and Application Support on macOS.
Existing Vikey settings are preserved and are not automatically migrated.
Exit the old IME before starting Sevi. Settings open directly without a welcome wizard.

## Tests and further reading

Run the typing-rule checks, corpus snapshots and performance checks:

```sh
cargo xtask check
```

- [Contributing](CONTRIBUTING.md): checks for each part of the project.
- [Decisions](DECISIONS.md): typing rules and their rationale.
- [Migration status](RUST_MIGRATION.md): port progress and native acceptance.
- [Tools](Tools/README.md): support commands.
- [Releasing](RELEASING.md): Git pushes, version tags, signing and updates.

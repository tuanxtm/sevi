# Windows implementation and acceptance

`crates/sevi-windows` implements keyboard/mouse hooks, foreground and browser
identification, checked SendInput, tray, preferences and updates in Rust.
`sevi-engine` and `sevi-runtime` own typing and shared desktop behavior;
`sevi-ui` supplies the shared settings and emoji windows.
The superseded C++ host and TweetNaCl implementation are retired.

`Windows/` retains icons, sounds, manifest/version resources, installer inputs
and native test TSVs. CMake invokes Cargo and preserves installer output paths;
there are no alternative C++ builds. Windows SDK/linker/resource tools are still
required by native Rust builds.

Build: `cargo build --locked --release -p sevi-windows --bins`.
Run portable regressions with `cargo xtask check`.
For live typing, exit the old tray process, start the rebuilt app, then run
`sevi-windows-livetest.exe --control=edit` and `--control=richedit` on Windows.

Cross-compilation does not establish native application compatibility. Real
Windows live typing, browser/UI interactions and installed update flows still
need acceptance. See [compatibility checks](Tests/COMPATIBILITY.md),
[migration status](../RUST_MIGRATION.md) and [build guide](../BUILDING.md).

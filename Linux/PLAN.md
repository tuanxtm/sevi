# Linux implementation and acceptance

`crates/sevi-linux` implements the IBus host in Rust. `sevi-engine` supplies
Telex and language switching; `sevi-runtime` owns shared desktop behavior;
`sevi-ui` supplies settings and emoji windows. The superseded C++ host is retired.

`Linux/` contains IBus/desktop registration, sounds, packaging, test TSVs and the
CMake packaging entrypoint. CMake invokes Cargo without detecting C/C++ compilers.

Run `cargo xtask check` for portable regressions. Build the production host with
`cargo build --locked --release -p sevi-linux --features settings-ui --bins`.
The optional Rust live runner uses GTK/libadwaita as native test libraries:
`cargo run --locked -p sevi-linux --features native-tests --bin sevi-linux-livetest`.
It needs an isolated graphical session and a running, registered Sevi IBus engine.

Native acceptance still includes pure Wayland/client combinations, unsupported
surrounding-text clients, package installation and update flows. Per-app identity
is limited by compositor support; Linux per-site memory remains excluded.
See [migration status](../RUST_MIGRATION.md) and [build guide](../BUILDING.md).

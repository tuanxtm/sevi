# Rust conversion status

Application behavior, the engine, all three native desktop hosts, shared UI and
engine/support tools are implemented in Rust. The parallel C/C++/Objective-C++
and Swift implementations, transitional engine C ABI and alternative build paths
have been retired. Native acceptance remains a separate, unfinished task.
The engine is now a normal Rust library and unconditionally forbids unsafe code;
native operating-system bindings remain in the desktop crates.

## Source ownership

- `crates/sevi-engine`: Telex, spelling, language switching, driver and touch model.
- `crates/sevi-runtime`: shared session, preferences, privacy and emoji behavior.
- `crates/sevi-linux`, `sevi-windows`, `sevi-macos`: native desktop integration.
- `crates/sevi-ui`, `sevi-ui-protocol`: shared egui UI and host communication.
- `crates/sevi-test-tools`, `sevi-tools`, `xtask`: regression runners, generators,
  corpus tools, scorecard and development commands.

`App/`, `Linux/` and `Windows/` retain resources, manifests, packaging and native
acceptance fixtures. CMake supplies packaging and invokes Cargo, without C/C++
compiler detection. Xcode bundles the Rust macOS archive and resources; its tiny
assembly entry forwards to Rust. Platform SDKs, system libraries, Sparkle,
linkers and resource compilers remain native dependencies.

Shell/PowerShell command wrappers, the Python GitHub release automation and font
asset generation, and the HTML/CSS/JavaScript website are separate tooling/assets.
They do not provide a second implementation of the input method.

## Engine data

The compact Rust word blobs and Rust spelling/key tables are authoritative.
Generators write these formats directly; there are no C headers or legacy import
steps. Dictionary scores, offsets, word order and regression expectations are
preserved by this cleanup. Original attribution remains in
[ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).

`cargo xtask check-data` verifies the SHA-256 manifest. After intentional data
changes, review them, run `cargo fmt --all`, then `cargo xtask lock-data`.
`cargo xtask check-sources` rejects superseded native implementation sources.
Both checks run as part of `cargo xtask check`.

## Regression validation

```sh
cargo xtask check
cargo clippy --locked -p sevi-engine -p sevi-runtime -p sevi-test-tools -p sevi-tools -p xtask --all-targets --all-features -- -D warnings
```

The existing tagged rules, reported bugs, decision coverage, golden snapshots,
syllable cases, driver/simulator parity, realistic-edit safety, touch/correction
and performance limits remain the behavioral contract. Goldens change only with
an explicit `cargo xtask check update` after reviewing intentional changes.
The known synthetic stuck cases retain their existing accepted limits.

The scorecard can compare archived Rust-era Git revisions. Pre-Rust comparisons
are rejected rather than invoking a C++ compiler. Historical upstream integration
and previously recorded checks remain in [UPSTREAM_PORTS.md](UPSTREAM_PORTS.md).

## Native acceptance still required

The Windows x64 cleanup test build (version 1.0.40, SHA-256
`ace1b429277c261ef4fc4d254180fdc3a11d094e8d7d2083b8ae5e41ac9b286b`)
was tested by the user without reported problems before preparing 1.0.50.
This confirms that test session, rather than the entire application matrix.

- Windows: live typing in real applications, browser integration, UI interactions,
  installer and signed update behavior; see [acceptance](Windows/Tests/COMPATIBILITY.md).
- macOS: bundle linking and execution, permissions, live typing, browser integration,
  system spell/rendering behavior and update flows.
- Linux: remaining client/compositor combinations and installed package/update flows.

Portable tests and cross-built executables cannot establish these results.
Retiring the reference sources does not mark them complete. See
[UI validation](UI_VALIDATION.md) and [building](BUILDING.md).

## Workspace and recovery

In this original checkout the self-contained workspace remains under `Rust/`;
standalone Sevi exports place its contents at repository root. The duplicate
upstream root sources have also been retired. Existing unrelated work and build
artifacts are preserved.

Before cleanup, edited files and retired sources were preserved locally under
`target/rust-cleanup/preserved/` at the original checkout root. That ignored
recovery area is outside the standalone export and is not a build input.
No source is deleted from that recovery area by the conversion.

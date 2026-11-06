# Rust migration status

This is a staged, behavior-preserving migration. The engine, shared desktop session, all three desktop host implementations and active desktop support tools are implemented in Rust. **Native acceptance and final retirement of reference implementations are unfinished.** Linux builds, packages and passes its native live suite here. Windows release executables now link through the project-local LLVM/MSVC cross-build toolchain. Windows/macOS live typing and UI acceptance, and macOS bundle linking, remain outstanding.

## Implemented

- Cargo workspace pinned to Rust 1.95.0, edition 2024, with `Cargo.lock`.
- Owned Rust Telex engine, spelling/orthography, auto-switch dictionaries and context, doubled-key handling, navigation, restoration and teencode; portable driver, immediate Backspace undo, pause timing, language-memory encoding and touch/correction model.
- Shared desktop preferences, hotkeys, app/site memory, exclusions, privacy guards, typing logs/statistics, multilingual emoji search and usage ranking.
- Native Rust Linux IBus adapter, X11 app identification, pre-edit/direct output, property menu, commands, settings monitoring, switch sounds and log/update scheduling. Production settings and emoji use the shared egui UI; GTK/libadwaita remain only in native test/reference fixtures.
- Native Rust Windows keyboard/mouse/focus hooks, checked batched `SendInput`, registry preferences/migration, independent UI Automation site/password workers, tray/menu, login setting, sounds and signed updater. The shared UI retains a nonactivating HWND for emoji and system DirectWrite color glyphs. The Rust live runner implements the existing Windows case file.
- Native Rust macOS event tap, re-enabling watchdog, layout/input-source tracking, secure-input logging guard, asynchronous AppleScript site lookup, UserDefaults migration, menu bar, login service and Sparkle binding. Shared egui windows replace the former AppKit widgets; the shared emoji picker renders inside a real nonactivating NSPanel. Spotlight selection replacement and Sublime/browser output workarounds are retained. Former native widget/debug files remain as references.
- Shared English UI: one compact settings page, checkboxes, custom title bar, system light/dark theme, embedded Inter/Geist Mono, and common help/about/credits. Its private helper exits after the final window closes; the IME keeps running. See [UI validation](UI_VALIDATION.md). Earlier UI/DPI notes below describe the previous native implementation.
- Linux/Windows CMake select Rust hosts by default. Xcode builds the Rust host archive for each requested architecture and bundles existing assets, entitlements and Sparkle. Its assembly entry only forwards to Rust; application behavior is in Rust. Original implementations remain available as reference sources.
- Exact transfer of 8,143 Vietnamese entries, 41,225 English entries, 160,386 bigrams and 13,490 protected words. The English score for `docs` now includes upstream D58; all other entries retain their original values. The Rust importer checks generated data without a C++ compiler.
- Rust rule harness, Python-compatible seeded corpus preparation, lazy/ideal and realistic-editing simulators, driver corpus runner, benchmark, English typo/normalization evaluators, and unified checks.
- Rust support commands cover dictionary/bigram/teencode/protected-word generation, typing-log analysis, appcast/emoji generation, macOS spelling, corpus fetch/cleaning/chat generation, case/bulk/margin/teencode/sweep evaluations, driver comparison, the quality scorecard, CI reporting, desktop image composition, site generation, Arch packaging and desktop release/deploy orchestration. See [command catalog](Tools/README.md). Default dictionary-generator outputs also refresh Rust engine data through the importer. Existing sources remain as migration references.
- Original simulator experiment controls and compile-time auto-switch thresholds are preserved. Sweep builds use an isolated target directory and do not replace normal engine binaries.
- Cargo workspace metadata is authoritative for version/build. CMake and Windows packaging read Cargo; `metadata --sync` refreshes Xcode's copy, checked before a native Mac build. Release scripts and Arch packaging are thin Rust dispatchers. CI calls the Rust scorecard/report, retaining default-branch-only code execution for the report.
- CI runs common Rust checks across five desktop runners and native adapter lint on Windows/macOS. Existing Linux/Windows build lanes remain; absent mobile projects are excluded/skipped.

## Workspace location

The self-contained port is under `Rust/`. Run all commands below from that folder.
Its `App/`, `Engine/`, `Linux/`, `Windows/`, `Tools/`, `site/` and `site-shots/`
contain independent copies of desktop resources, packaging, regression fixtures
and reference sources. Cargo and native builds use these copies, not the original
directories at the repository root. Mobile projects/tools and prebuilt reference
test binaries were not copied. The original GPL license is included here.

GitHub workflows remain in the repository's required `.github/workflows/`
location and run inside `Rust/`. Existing root desktop check/CMake/Xcode
entrypoints delegate to the port for compatibility. New build outputs live under
`Rust/target/`; historical `target/` artifact paths recorded below refer to the
old repository-root output directory, which is preserved.

Relocation validation passes: full common checks, byte-for-byte asset/data and
embedded-path audits, frozen-data import, strict Windows/support-tool Clippy,
Linux desktop release build, Apple Silicon static archive, Windows x64 release
and PE/resource checks. The root CMake compatibility entrypoints configure, and
the C++ compatibility benchmark links against the relocated Rust engine. The
scorecard comparison with pre-move `f73d01e` matches every non-timing measurement.
Native Windows/macOS application execution remains unverified here.

Current x64 test executable: `Rust/target/windows-test/Sevi-x64.exe` relative to
the repository root (5,466,624 bytes; SHA-256
`89fb084b53380b8a04f71070112a545b7721d75aff9e9fac00d8d9fff185446f`).
The previous executable remains in the old root `target/windows-test/` folder.

## Build and check

```sh
cargo xtask check
cargo xtask import-legacy-data --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The workspace includes a Linux-only native crate. On macOS/Windows, use explicit portable packages for common lint (`sevi-engine`, `sevi-runtime`, `sevi-test-tools`, `sevi-tools`, `xtask`) plus the current OS adapter. `cargo xtask check` selects portable packages explicitly, including the portable Windows/macOS policy tests.

`check` covers rules, regressions, undo, unchanged golden snapshots/metrics, 11,092 syllable cases, driver/simulator parity, performance, realistic editing and touch/correction thresholds. The four synthetic stuck cases remain the recorded baseline; text-safety violations must remain zero. Existing shell/PowerShell check entrypoints dispatch to Rust. Golden files change only with an explicit `update` argument.

The isolated C++ reference is available through `cargo xtask baseline check`; sources, binaries, hashes and results stay under `target/legacy`. The starting revision is `490820bc70c48c38b864cb1b63f6de61f9a44ab3`.

Native build commands and SDK requirements remain. Windows/Linux can select original hosts with `-DSEVI_RUST_DESKTOP=OFF`. macOS needs Rust libraries for its requested architectures:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
xcodegen generate
xcodebuild -project Sevi.xcodeproj -scheme Sevi -configuration Release build
```

`Cargo.toml` supplies version 1.0.30/build 32. After editing it, run `cargo xtask tool metadata --sync` before `xcodegen generate`. The Rust release lane bumps both Cargo metadata and Xcode's generated copy.

## Validation performed

- Original C++ rules, reported regressions, golden snapshots, metrics and undo passed in isolation. Rust rules, snapshots, corpus metrics, syllable lock, driver parity, realistic-edit counts and touch/correction thresholds passed. The original C++ harness/simulator linked to the Rust engine passed the original Python golden suite unchanged.
- Shared session replay passes all 40 Linux cases and all 26 Windows fixtures on both Windows and macOS policies, plus pause expiry, Windows secure-focus/other-IME checks, macOS exclusions/hotkeys and distinct rewrite/undo output policies. New persistence tests cover deferred writes, retry, clear, stats snapshots and exact log-tail boundaries.
- Linux native sources pass strict Clippy. Settings, five onboarding steps, log and emoji windows rendered in isolated Xvfb tests; screenshots are in `target/native-ui/`. Visual findings concerning label wrapping and log-window sizing were corrected.
- Linux release rebuilt through CMake and produced `target/rust-linux-package/sevi_1.0.30_amd64.deb` and `sevi-1.0.30-1-x86_64.pkg.tar.zst`. The Arch repack preserves every Debian payload byte, mode and root ownership; its manifest hashes verify. The Debian architecture is explicit, including on hosts without `dpkg`. No system installation was performed; RPM packaging is unverified because `rpmbuild` is unavailable here.
- Native GTK/IBus typing passes **40/40** upstream cases against the rebuilt release engine, including both restored-word Home cases. The adapter commits restoration and preserves the original navigation event so GTK4 widget bindings run. The runner uses a real focus change to commit pre-edit text before checking `Entry::text()`. Latest passing log: `target/native-session/run.ewx6IV/live.log`.
- Windows release application and live runner link through LLVM/MSVC SDK cross-builds; x64 passes strict Clippy. Only x64 is rebuilt, as requested. The refreshed test build includes the DPI and insertion-safety fixes: `target/windows-test/Sevi-x64.exe` (5,466,624 bytes; SHA-256 `15097dca2fd87be618a710d6cedc14ea0c3ac6c96739b9e34e25fddeacac86ec`). PE inspection verifies architecture, GUI subsystem, icon/version/sound/emoji resources, manifest and system-only DLL dependencies with a statically linked C runtime. Previous executables are preserved under `target/windows-test/previous/950c93e/`, `target/windows-test/previous/c030d0e/` and `target/windows-test/previous/34eee7c/`. ARM64 build evidence applies to the initial port only. The executable is unsigned and has not been run on Windows here.
- macOS passes strict Clippy for Intel and Apple Silicon, and the current Apple Silicon static archive builds (`target/aarch64-apple-darwin/release/libsevi_macos.a`). Actual macOS bundle linking and native execution remain unverified.
- First support-tool parity fixtures pass byte-for-byte comparisons with isolated Python copies for dictionary blending/overrides, UTF-8 bigram offsets/thresholds, teencode forms/scores and protected-word filtering/order. Log output matches for normal, zero and negative context lengths. Local parity harness: `target/tool-parity/test_generators.py`.
- The normalization evaluator matched the C++ reference on an eight-row local fixture; its full private dataset is unavailable in this project.
- Appcast updates, emoji generation and pending/full static-site fixtures match original output bytes. Corpus cleaning matches original bytes; corpus HTTP clients pass local mock-server checks without private credentials or paid API calls.
- Bulk and margin output, all eight threshold-sweep configurations, simulator early-switch/direction/no-lookback/restored-word controls, and externally loaded bigram/trigram/capital-frequency controls match C++/Python references. The quality scorecard's non-timing measurements match C++; text-safety violations stay zero. Local comparison artifacts are under `target/tool-parity/` and `target/native-session/`.
- Scorecard comparisons build archived Rust tools for Rust-era references and archived C++ tools for earlier references. The Rust-reference build path passes an isolated archive fixture without creating a commit or changing Git refs.
- The DMG background matches original pixels and DPI. Desktop, scene and macOS-desktop composition fixtures have identical decoded pixels, including rounded masks, shadows and alpha-composite rounding. Frozen mask/blur and CI-log parsing tests run in the Rust suite.

## Remaining accepted work

1. Finish native acceptance: macOS bundle linking; Windows/macOS preferences/UI interactions, browser integration, permissions, real live typing, macOS spell filtering/offscreen rendering and updates; remaining Linux client types and package/update flows. Review adapters against original behavior where actual native testing reveals differences.
2. Validate installed packages and updates on actual operating systems and architectures, including RPM/Arch installation. Signing, notarization, publication and deployment are not exercised by local migration checks.
3. Retire compatibility adapters and superseded reference implementations only after native acceptance passes. Their presence does not make them the default desktop implementation.

Website runtime HTML/CSS/browser JS and Cloudflare handlers stay outside scope. Deleted mobile projects and tools whose only consumers are those projects (including prediction packing and Apple Intelligence proofreading eval) stay excluded. Pre-existing deletions are preserved.

## Resumed checkpoint — 2026-11-02 UTC

Work resumed after the requested rest checkpoint. Local implementation and tooling parity work is complete at this checkpoint; native acceptance limits are listed above. No test servers are left running. The Rust migration is saved on the local `rust-port` branch, based on upstream commit `490820bc70c48c38b864cb1b63f6de61f9a44ab3`. Pre-existing unrelated deletions and the crash dump remain outside the migration commit. Generated executables, SDK caches and validation artifacts remain under ignored `target/`. Nothing has been pushed or published.

The user reports that normal Windows x64 testing found no problems. This provides an initial runtime check; the broader native acceptance items above remain open.

Desktop bug fixes have now been reviewed through upstream `0e272a71839db51f387f371570c35e207c4de1b3`. See [UPSTREAM_PORTS.md](UPSTREAM_PORTS.md) for the applied/already-satisfied fixes, exclusions and next-review baseline. iOS/Android remain excluded. The updated common checks pass 157 rules and 20 reported regressions; the quality scorecard matches that upstream revision's non-timing measurements, with zero text-safety violations. Version/build remain 1.0.30/32 until a Rust release is deliberately cut.

Use `RUSTUP_TOOLCHAIN=stable` locally (installed Rust 1.95.0) and project-local `CARGO_HOME=/home/tuantm/projects/sevi/target/cargo-home`. Native test scratch/output stays under `target/native-session`; its isolated launcher mounts only system dependencies and this project, and uses disposable XDG/D-Bus/IBus paths. Do not run the old broad-root-mount launcher or touch the normal desktop configuration. GTK smoke tests are ignored by default and require an isolated graphical session:

```sh
cargo test -p sevi-linux --lib native_views -- --ignored --nocapture --test-threads=1
```

Private data sources referenced by the original tools are outside this session's project boundary. Their files were not read; portable generator validation uses project-owned fixtures. No signing, publishing, deployment, installer execution or system update/install has been performed.

## Windows display scaling — 2026-11-02 UTC

The manifest already selected PerMonitorV2, but the Rust host still treated most layouts and fonts as physical pixels. UI geometry now uses 96-DPI logical units and converts at the window's actual DPI. Settings, onboarding, history, About/credits, tooltips, icons and custom painting scale together. Native fonts are cached by role and DPI. Logical control positions are retained across monitor changes, preserving entered text, recorder state and placement without accumulating resize rounding errors. History columns/resizing, credits scrolling/link hit testing and the emoji picker's destination-monitor DPI are included.

Top-level windows follow Windows' recommended `WM_DPICHANGED` rectangle and use `AdjustWindowRectExForDpi` for client sizing ([Microsoft DPI guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)). The layout supports arbitrary Windows DPI values, including 100%, 125%, 150%, 175%, 200% and custom scales; it is not fixed to the reported 200% setting.

Eight portable Windows policy tests pass, including new scale/font-size and hit-target regressions at fractional/custom DPI values. Windows x64 strict Clippy, release linking, formatting and PE/resource inspection pass. Logs are under `target/native-session/dpi-*`. Visual Windows acceptance remains: exit the previous tray process, launch the refreshed executable, inspect every window at the desired scale, then move windows between monitors/change Windows scale while an onboarding field contains text. Confirm sizing, sharp text, click alignment and preserved input. This Linux session cannot perform that native visual check.

## Windows insertion safety — 2026-11-03 UTC

The host now checks the number of events accepted by `SendInput`. Complete batches
retain the existing insertion behavior. A zero-event failure forwards the original
physical key. A partial failure suppresses that key and never retries replacement
text or Backspace; at most one unmatched synthetic key-up is attempted, and its
result is checked without retrying. Failed output discards transient word,
correction, undo and navigation state while preserving language/configuration.
Transformations remain bypassed in that foreground window until the actual
foreground changes. Language hotkeys remain available; clicks, word breaks,
settings reloads and site changes do not lift the failure guard. Emoji output uses
the same checked path. No dependencies, app-specific workarounds or privilege
bypasses were added.

Five portable insertion tests and two session recovery tests pass, alongside all
existing common checks: 157 rules, 20 reported regressions, unchanged goldens,
11,092 syllable cases, zero text-safety violations and the recorded four synthetic
stuck cases. The engine/driver benchmark measured 570 ns average and 6,558 ns
p99.9 per key on Linux; this excludes Windows delivery. Windows x64 strict Clippy
(including the native foreground-guard test), release linking, formatting and PE
inspection pass. Logs: `target/native-session/insertion-*.log`.

The packaged `target/windows-test/Sevi-x64-livetest.exe` checks its own input
delivery and supports both Edit and RichEdit, with seven new selection/navigation
fixtures embedded by default. Native execution remains pending, including Word,
Excel, browsers/autocomplete, Windows Terminal, Remote Desktop and elevated
targets. See [Windows acceptance instructions](Windows/Tests/COMPATIBILITY.md).
`SendInput` success only confirms queue insertion; a partial edit may still need
manual correction. Exit the old tray process before testing the refreshed EXE.

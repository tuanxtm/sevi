# Rust support tools

Run commands from the `Rust/` workspace root with `cargo xtask tool <command> [arguments]`.
Engine, corpus and dictionary tools are implemented in Rust. Superseded Python,
Swift and C++ implementations have been retired. Shell/PowerShell wrappers and
the standalone GitHub release automation remain. Mobile projects are excluded.

| Command | Existing task |
|---|---|
| `build-dict` | Blend word frequencies, spelling acceptance and overrides |
| `build-forms <SCOWL-directory>` | Expand the supplied en_US Hunspell dictionary with `unmunch`, keeping only missing forms of listed English base words |
| `build-bigrams` | Generate scored word pairs |
| `build-teencode` | Generate teencode forms and frequencies |
| `keepgen <word-file>` | Generate protected-word data |
| `spellfilter` | Filter standard input with the macOS English spell checker |
| `build-emoji` | Generate the existing emoji index from emojibase/CLDR |
| `analyze-log [arguments]` | Summarize the existing typing log |
| `cases` | Run `Tools/autoswitch/test/cases.tsv` |
| `bulk`, `margins`, `sweep` | Existing dictionary accuracy and threshold experiments |
| `teen-eval` | Evaluate the existing teencode corpus |
| `compare-driver <units> [en\|vi]` | Compare driver and lazy-simulator text |
| `scorecard [--compare REF] [--json PATH]` | Measure public corpora and compare a Rust-era Git revision; pre-Rust revisions are rejected |
| `clean-viblo`, `fetch-viblo`, `gen-chat` | Existing corpus cleaning, fetch and chat generation tasks |
| `dl-shots desktop\|scene\|macdesk ...` | Compose existing desktop download-page pictures |
| `dmg-background <output.png>` | Generate the existing 2× DMG background |
| `site-build` | Build the existing static website into `site/dist` |
| `make-pacman <package.deb> <output-directory>` | Repack the Debian payload for Arch Linux |
| `appcast mac\|windows ...` | Update one platform/architecture in the existing feed |
| `metadata [--sync] [--version]` | Check/sync Xcode's copy of Cargo release metadata |
| `release <version>` | Existing macOS build/sign/notarize/publish lane |
| `release --windows <version>`, `release --linux <version>` | Existing desktop package publication lanes |
| `deploy-site` | Restore referenced downloads, build and deploy the existing site |
| `pr-report` | Existing CI report, using `REPO`, `SHA`, `GH_TOKEN`; `DRY=1` prints it |

The engine command-line programs are Cargo binaries in `sevi-test-tools`:
`harness`, `prep`, `sim`, `rtsim`, `drvlazy`, `bench`, `touchsim`, `typoeval`,
and `normeval`. For example:

```sh
mkdir -p target
cargo run --release -p sevi-test-tools --bin prep -- Tools/autoswitch/sim/corpus/chat_messages.txt > target/chat.units
cargo run --release -p sevi-test-tools --bin sim -- target/chat.units --lazy
```

`cargo xtask check` also audits historical decisions against tagged rules and the
Rust references in `Tools/autoswitch/rules/decisions.tsv`.
`cargo run -p sevi-test-tools --bin sevi-test-tools -- decisions` runs that audit alone. Explicit `open` entries report native-acceptance
gaps; `none` entries record removed, rejected or mobile-only behavior.

Dictionary/corpus inputs default to the workspace's `data/` directory. Use
`--data` or `SEVI_DATA` and `--source` to select explicitly supplied inputs.
Dictionary generators write directly into `crates/sevi-engine/src/data/`;
`--output DIRECTORY` writes isolated outputs for review. No C headers or importer
are involved. The dictionary commands produce `vn.*`/`en.*`, `en-forms.*`,
`bigram.*`, `teencode.rs` and `keep.words`, respectively. After reviewing deliberate
data changes, run `cargo fmt --all` and `cargo xtask lock-data`. `check-data` verifies
the committed SHA-256 manifest without requiring original private corpora. The
original frequency knobs keep their names. The engine's original `AS_*`
compile-time experiments use environment variables supplied to Cargo; `sweep`
builds its variants under `target/experiment-sweep`.

The simulator retains `SIM_EARLY`, `SIM_EARLY_DIR`, `SIM_NO_LOOKBACK`,
`SIM_NO_RESTORED_PREV`, `SIM_BIGRAM`/`SIM_BIGRAM_W`/`SIM_BIGRAM_MIN`,
`SIM_TRIGRAM`/`SIM_TRIGRAM_W`/`SIM_TRIGRAM_MIN`, `SIM_CAP`/`SIM_CAP_MIN` and
`SIM_MANUAL_LOG`. Normal desktop builds keep the existing defaults.

Release/deploy commands retain the existing signing services, credential files
and external tools. They publish when explicitly invoked; migration validation
uses fixtures and does not call them. Superseded generated site/release output
is moved under `target/` before replacement. No native installer is executed
by the portable checks.

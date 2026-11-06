# Auto switch by typing: word lists and tests

The Rust engine in `crates/sevi-engine` preserves the existing Telex and
auto-switch behavior. Frequency data and existing regression/golden fixtures
remain in this directory. C++, Python and Swift sources are retained as
migration references.

Run from the `Rust/` workspace root:

```sh
cargo xtask tool build-dict
cargo xtask tool cases
cargo xtask tool bulk
cargo xtask tool scorecard
cargo xtask check
```

On macOS, `cargo xtask tool spellfilter` filters words on standard input using
the existing English spelling policy. The generators refresh Rust engine data
when writing their default output. Use `--source`, `--data` and `--output`
for isolated fixtures.

- `vi_50k.txt`, `en_50k.txt`: FrequencyWords by Hermit Dave (OpenSubtitles 2018), CC BY-SA 4.0.
- `en_spell_ok.txt`: lowercase or capitalized words accepted by the macOS checker.
- `en_extra.txt`: chat shorthand and developer words always treated as English.
- `rules/rules.tsv`: decided rules, tagged with their cases.
- `rules/golden/`: unchanged corpus/syllable snapshots and locked metrics.

`cargo xtask check update` accepts changed goldens only after reviewing their
diffs. Existing compile-time thresholds can be supplied to Cargo, for example
`AS_VN_FLOOR=30 cargo xtask check`; `cargo xtask tool sweep` isolates its
variants in a separate target directory.

See [all support commands](../README.md) and [migration status](../../RUST_MIGRATION.md).

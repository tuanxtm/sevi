# Engine data

The committed Rust tables and compact blobs are authoritative; no C headers or
legacy importer are needed to build, regenerate or verify them.

| Files | Representation | Frozen entries |
| --- | --- | --- |
| `vn.*` | NUL-separated ASCII keys, little-endian u32 offsets, signed i8 scores | 8,143 |
| `en.*` | Same representation | 41,225 |
| `en-forms.*` | NUL-separated ASCII forms, little-endian u32 offsets, no frequency scores | 18,197 |
| `bigram.*` | NUL-separated UTF-8 keys, little-endian u32 offsets, unsigned logarithmic count bytes | 160,386 |
| `keep.words` | Sorted ASCII protected words, one per line | 13,490 |
| `teencode.rs` | Rust constants for teencode forms and their scores | Pinned forms/scores |

Word blobs retain the extra final NUL from the original data representation.
Missing English forms score -8 only after the main English lookup and Vietnamese
recognition fail. Existing frequency scores are unchanged.

Spelling/character tables are in `../tables.rs`; canonical key codes are in
`../keys.rs`. `checksums.json` records SHA-256 hashes for all frozen data and
these Rust tables. `cargo xtask check-data` verifies them without private corpora.

The Rust commands `build-dict`, `build-forms`, `build-bigrams`, `keepgen` and
`build-teencode` write these formats directly. Run them through `cargo xtask tool`.
`--output DIRECTORY` writes reviewable fixture data separately. See
[Tools](../../../../Tools/README.md) for inputs and command details.
After deliberate changes, review the data, run `cargo fmt --all`, then explicitly
accept its checksums with `cargo xtask lock-data`. Normal checks never rewrite
or accept the manifest.

Attribution is retained in [ACKNOWLEDGEMENTS.md](../../../../ACKNOWLEDGEMENTS.md):
FrequencyWords/OpenSubtitles (CC BY-SA 4.0); FineWeb, HPLT, MADLAD, Reddit and
Vietnamese forum word-pair statistics; SCOWL/English Speller Database en_US forms.
The original OpenKey/Vikey credits and license notices remain in the project.

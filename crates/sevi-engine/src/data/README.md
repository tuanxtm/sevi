# Frozen engine data

These files contain the existing Sevi/OpenKey tables transferred to Rust without regeneration, frequency changes, or new source data.

| Files | Representation | Entries |
| --- | --- | --- |
| `vn.*` | NUL-separated ASCII keys, little-endian `u32` offsets, signed `i8` scores | 8,143 |
| `en.*` | Same representation | 41,225 |
| `bigram.*` | NUL-separated UTF-8 keys, little-endian `u32` offsets, unsigned logarithmic count bytes | 160,386 |
| `keep.words` | Sorted ASCII words, one per line | 13,490 |

The word blobs retain the C string literals' final terminator, including their extra trailing NUL. Table lookups do not require native byte order or runtime file access.

The character, spelling, and teencode tables are in `../tables.rs`. Canonical key codes are in `../keys.rs`.

Verify all transferred bytes and table values against the reference sources:

```sh
cargo xtask import-legacy-data --check
```

`cargo xtask import-legacy-data` explicitly imports the reference tables again. It does not fetch dictionaries or retrain them. Run `cargo fmt --all` after an import. Normal builds use only the committed Rust data.

Original attribution remains in the reference headers and [ACKNOWLEDGEMENTS.md](../../../../ACKNOWLEDGEMENTS.md). Word frequencies derive from FrequencyWords/OpenSubtitles (CC BY-SA 4.0); word-pair sources include FineWeb, HPLT, MADLAD, Reddit, and the original forum crawl. Preserve these notices when redistributing or regenerating the data.

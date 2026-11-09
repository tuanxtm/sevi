//! Native Rust engine-data generation and integrity checks.
use crate::{Args, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const DATA_FILES: &[&str] = &[
    "keys.rs",
    "tables.rs",
    "data/teencode.rs",
    "data/vn.words",
    "data/vn.offsets",
    "data/vn.scores",
    "data/en.words",
    "data/en.offsets",
    "data/en.scores",
    "data/en-forms.words",
    "data/en-forms.offsets",
    "data/bigram.words",
    "data/bigram.offsets",
    "data/bigram.scores",
    "data/keep.words",
];

pub(crate) fn directory(args: &Args<'_>) -> PathBuf {
    args.output("crates/sevi-engine/src/data")
}

fn encode_words<'a>(words: impl IntoIterator<Item = &'a str>) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut blob = Vec::new();
    let mut offsets = Vec::new();
    let mut previous = None;
    for word in words {
        if word.is_empty() || word.contains('\0') || previous.is_some_and(|p: &str| p >= word) {
            return Err(
                "engine word lists must be nonempty keys in strict byte order, without NULs".into(),
            );
        }
        offsets.extend(u32::try_from(blob.len())?.to_le_bytes());
        blob.extend(word.as_bytes());
        blob.push(0);
        previous = Some(word);
    }
    // Preserve the frozen blobs' final extra terminator, including empty lists.
    blob.push(0);
    Ok((blob, offsets))
}

pub(crate) fn write_words<'a>(
    directory: &Path,
    prefix: &str,
    words: impl IntoIterator<Item = &'a str>,
) -> Result {
    let (blob, offsets) = encode_words(words)?;
    fs::create_dir_all(directory)?;
    fs::write(directory.join(format!("{prefix}.words")), blob)?;
    fs::write(directory.join(format!("{prefix}.offsets")), offsets)?;
    Ok(())
}

pub(crate) fn write_dictionary<'a>(
    directory: &Path,
    prefix: &str,
    rows: impl IntoIterator<Item = (&'a str, u8)>,
) -> Result {
    let rows: Vec<_> = rows.into_iter().collect();
    write_words(directory, prefix, rows.iter().map(|(word, _)| *word))?;
    fs::write(
        directory.join(format!("{prefix}.scores")),
        rows.iter().map(|(_, score)| *score).collect::<Vec<_>>(),
    )?;
    Ok(())
}

fn checksums(root: &Path) -> Result<BTreeMap<String, String>> {
    let source = root.join("crates/sevi-engine/src").canonicalize()?;
    DATA_FILES
        .iter()
        .map(|name| {
            let path = source.join(name).canonicalize()?;
            if !path.starts_with(&source) {
                return Err("engine data leaves the source directory".into());
            }
            Ok((
                (*name).to_owned(),
                format!("{:x}", Sha256::digest(fs::read(path)?)),
            ))
        })
        .collect()
}

/// Lock deliberate data changes only when explicitly requested.
pub fn lock_data(root: &Path) -> Result {
    let path = root.join("crates/sevi-engine/src/data/checksums.json");
    fs::write(
        path,
        serde_json::to_string_pretty(&checksums(root)?)? + "\n",
    )?;
    Ok(())
}

pub fn check_data(root: &Path) -> Result {
    let expected: BTreeMap<String, String> = serde_json::from_slice(&fs::read(
        root.join("crates/sevi-engine/src/data/checksums.json"),
    )?)?;
    let actual = checksums(root)?;
    if expected != actual {
        let changed: Vec<_> = DATA_FILES
            .iter()
            .filter(|name| expected.get(**name) != actual.get(**name))
            .collect();
        return Err(format!("engine data checksum mismatch: {changed:?}; review data changes before cargo xtask lock-data").into());
    }
    println!("verified {} Rust engine data files", actual.len());
    Ok(())
}

/// Keep superseded native implementations out of active source and build inputs.
pub fn check_sources(root: &Path) -> Result {
    fn visit(path: &Path) -> Result {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(
                    format!("source audit refuses symlink {}", entry.path().display()).into(),
                );
            }
            if kind.is_dir() {
                if !matches!(
                    entry.file_name().to_str(),
                    Some("target" | "build" | "dist" | "__pycache__" | ".git")
                ) {
                    visit(&entry.path())?;
                }
            } else if entry
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    matches!(
                        s,
                        "c" | "cc"
                            | "cpp"
                            | "cxx"
                            | "h"
                            | "hh"
                            | "hpp"
                            | "hxx"
                            | "m"
                            | "mm"
                            | "swift"
                            | "inc"
                    )
                })
            {
                return Err(format!(
                    "superseded native source in Rust workspace: {}",
                    entry.path().display()
                )
                .into());
            }
        }
        Ok(())
    }
    for name in ["App", "Linux", "Windows", "Tools", "crates", "xtask"] {
        visit(&root.join(name))?;
    }
    if root.join("Engine").exists() || root.join("cmake/RustEngine.cmake").exists() {
        return Err("legacy engine build inputs must not be restored".into());
    }
    println!("verified Rust-only application and tool sources");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_words_preserve_utf8_offsets_and_the_frozen_terminator() {
        let (words, offsets) = encode_words(["a\tbus", "a\tbên", "z"]).unwrap();
        assert_eq!(words, b"a\tbus\0a\tb\xc3\xaan\0z\0\0");
        assert_eq!(
            offsets,
            [0u32, 6, 13]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(encode_words([]).unwrap(), (vec![0], vec![]));
        assert!(encode_words(["b", "a"]).is_err());
        assert!(encode_words(["a", "a"]).is_err());
        assert!(encode_words(["a\0b"]).is_err());
        assert!(encode_words([""]).is_err());
    }
    #[test]
    fn retired_headers_are_unneeded_and_frozen_rust_data_is_intact() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        check_data(root).unwrap();
        check_sources(root).unwrap();
    }
    #[test]
    fn native_writer_reproduces_every_frozen_dictionary_byte() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let output = root.join("target/data-writer-tests").join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for (prefix, dictionary) in [
            ("vn", &sevi_engine::data::VN),
            ("en", &sevi_engine::data::EN),
            ("bigram", &sevi_engine::data::BIGRAM),
        ] {
            write_dictionary(
                &output,
                prefix,
                (0..dictionary.len()).map(|index| {
                    (
                        std::str::from_utf8(dictionary.word(index)).unwrap(),
                        dictionary.scores[index],
                    )
                }),
            )
            .unwrap();
            for (extension, expected) in [
                ("words", dictionary.words),
                ("offsets", dictionary.offsets),
                ("scores", dictionary.scores),
            ] {
                assert_eq!(
                    fs::read(output.join(format!("{prefix}.{extension}"))).unwrap(),
                    expected,
                    "{prefix}.{extension}"
                );
            }
        }
        let forms = &sevi_engine::data::EN_FORMS;
        write_words(
            &output,
            "en-forms",
            (0..forms.len()).map(|index| std::str::from_utf8(forms.word(index)).unwrap()),
        )
        .unwrap();
        assert_eq!(
            fs::read(output.join("en-forms.words")).unwrap(),
            forms.words
        );
        assert_eq!(
            fs::read(output.join("en-forms.offsets")).unwrap(),
            forms.offsets
        );
    }
}

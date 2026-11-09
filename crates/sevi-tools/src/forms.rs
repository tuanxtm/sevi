//! Rebuild the supplemental SCOWL en_US list with Hunspell's own affix expansion.
use crate::{Args, Result, data_files};
use sevi_engine::data::EN;
use std::{
    collections::BTreeSet,
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn select_forms(
    expanded: &str,
    listed: impl Fn(&str) -> bool,
) -> Result<(BTreeSet<String>, usize)> {
    let mut forms = BTreeSet::new();
    let mut group: Vec<&str> = Vec::new();
    let mut groups = 0;
    for line in expanded.lines().filter(|line| !line.is_empty()) {
        if line
            .strip_prefix("qqsep")
            .is_some_and(|tail| !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()))
        {
            if let Some(base) = group.first()
                && listed(base)
            {
                forms.extend(
                    group
                        .iter()
                        .filter(|word| {
                            word.len() >= 2
                                && word.bytes().all(|b| b.is_ascii_lowercase())
                                && !listed(word)
                        })
                        .map(|word| (*word).to_owned()),
                );
            }
            group.clear();
            groups += 1;
        } else {
            group.push(line);
        }
    }
    if !group.is_empty() {
        return Err("incomplete Hunspell expansion: final separator missing".into());
    }
    Ok((forms, groups))
}
pub fn run(args: &Args<'_>) -> Result {
    let source = args
        .args
        .first()
        .filter(|value| !value.starts_with('-'))
        .ok_or(
            "usage: build-forms <directory-with-en_US.dic-and-en_US.aff> [--output DIRECTORY]",
        )?;
    let source = std::path::Path::new(source).canonicalize()?;
    let dictionary = fs::read_to_string(source.join("en_US.dic"))?;
    let entries: Vec<_> = dictionary
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return Err("empty en_US dictionary".into());
    }
    let scratch = args.root.join("target/english-forms").join(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(&scratch)?;
    let mut separated = format!("{}\n", entries.len() * 2);
    for (index, entry) in entries.iter().enumerate() {
        separated.push_str(&format!("{entry}\nqqsep{index}\n"));
    }
    let input = scratch.join("separated.dic");
    fs::write(&input, separated)?;
    let expanded = Command::new("unmunch")
        .arg(&input)
        .arg(source.join("en_US.aff"))
        .output()?;
    if !expanded.status.success() {
        return Err(format!(
            "Hunspell unmunch failed: {}",
            String::from_utf8_lossy(&expanded.stderr)
        )
        .into());
    }
    let (forms, groups) = select_forms(&String::from_utf8(expanded.stdout)?, |word| {
        EN.index(word.as_bytes()).is_ok()
    })?;
    if groups != entries.len() || forms.is_empty() {
        return Err("Hunspell expansion did not produce a complete, nonempty form list".into());
    }
    let output = data_files::directory(args);
    data_files::write_words(&output, "en-forms", forms.iter().map(String::as_str))?;
    println!(
        "{} SCOWL forms from listed base words -> {}",
        forms.len(),
        output.display()
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn affix_expansion_requires_a_listed_base_and_preserves_real_forms_only() {
        let (forms, groups) = select_forms("reset\nresets\nresetting\nReset\nreset's\nqqsep0\nunknown\nunknowns\nqqsep1\nquest\nquests\nqqsep2\n", |word| ["reset", "quest", "resetting"].contains(&word)).unwrap();
        assert_eq!(groups, 3);
        assert_eq!(forms, BTreeSet::from(["quests".into(), "resets".into()]));
        assert!(select_forms("reset\nresets\n", |_| true).is_err());
    }
}

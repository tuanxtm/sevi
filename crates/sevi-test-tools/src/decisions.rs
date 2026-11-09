//! Audit the historical decisions against current Rust tests and explicit scope exceptions.
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Component, Path},
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub tested: usize,
    pub open: usize,
    pub excluded: usize,
}
pub fn audit(root: &Path) -> Result<Coverage, Box<dyn Error>> {
    let root = root.canonicalize()?;
    let document = fs::read_to_string(root.join("DECISIONS.md"))?;
    let headings = Regex::new(r"(?m)^## (\d+)\. (.+)$")?;
    let mut decisions = BTreeMap::new();
    for heading in headings.captures_iter(&document) {
        let id: usize = heading[1].parse()?;
        if decisions.insert(id, heading[2].to_owned()).is_some() {
            return Err(format!("duplicate decision D{id}").into());
        }
    }
    if decisions.is_empty() {
        return Err("no numbered decisions found".into());
    }
    let rules = root.join("Tools/autoswitch/rules");
    let tags = Regex::new(r"(?:^|\t)D(\d+)")?;
    let mut tagged = BTreeSet::new();
    for entry in fs::read_dir(&rules)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "tsv")
            && entry.file_name() != "decisions.tsv"
        {
            let path = path.canonicalize()?;
            if !path.starts_with(&root) {
                return Err("rule path leaves the workspace".into());
            }
            for line in fs::read_to_string(path)?
                .lines()
                .filter(|line| !line.starts_with('#'))
            {
                for tag in tags.captures_iter(line) {
                    tagged.insert(tag[1].parse::<usize>()?);
                }
            }
        }
    }
    for id in &tagged {
        if !decisions.contains_key(id) {
            return Err(format!("rule names undocumented decision D{id}").into());
        }
    }
    let mut manifest = BTreeMap::new();
    for line in fs::read_to_string(rules.join("decisions.tsv"))?
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
    {
        let fields: Vec<_> = line.splitn(3, '\t').collect();
        if fields.len() != 3 || fields[2].trim().is_empty() {
            return Err(format!("invalid decision coverage row: {line}").into());
        }
        let id: usize = fields[0]
            .strip_prefix('D')
            .ok_or("decision must start with D")?
            .parse()?;
        if !decisions.contains_key(&id) || manifest.contains_key(&id) {
            return Err(format!("unknown or duplicate manifest decision D{id}").into());
        }
        match fields[1] {
            "test" => {
                for reference in fields[2].split(" ; ") {
                    let (path, needle) = reference
                        .split_once("::")
                        .ok_or("test reference requires path::named-test")?;
                    let relative = Path::new(path);
                    if relative
                        .components()
                        .any(|part| !matches!(part, Component::Normal(_)))
                        || needle.trim().is_empty()
                    {
                        return Err(format!("invalid test reference for D{id}: {reference}").into());
                    }
                    let path = root.join(relative).canonicalize()?;
                    if !path.starts_with(&root) || !path.is_file() {
                        return Err(
                            format!("test reference leaves the workspace: {reference}").into()
                        );
                    }
                    if !fs::read_to_string(path)?.contains(needle) {
                        return Err(format!("D{id}: test not found: {reference}").into());
                    }
                }
            }
            "none" | "open" => (),
            kind => return Err(format!("D{id}: unknown coverage kind {kind}").into()),
        }
        manifest.insert(id, fields[1].to_owned());
    }
    let mut coverage = Coverage::default();
    for (id, title) in decisions {
        match manifest.get(&id).map(String::as_str) {
            Some("none") => coverage.excluded += 1,
            Some("open") => {
                coverage.open += 1;
                println!("  open D{id}: {title}");
            }
            Some("test") => coverage.tested += 1,
            _ if tagged.contains(&id) => coverage.tested += 1,
            _ => {
                return Err(
                    format!("D{id} {title:?}: no tagged case or explicit coverage row").into(),
                );
            }
        }
    }
    println!(
        "decisions with tests: {}/{} (open {}, no desktop behavior {})",
        coverage.tested,
        coverage.tested + coverage.open,
        coverage.open,
        coverage.excluded
    );
    Ok(coverage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coverage_detects_missing_decisions_stale_tests_and_explicit_mobile_exclusions() {
        let scratch = crate::check::root()
            .join("target/decision-coverage-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let rules = scratch.join("Tools/autoswitch/rules");
        fs::create_dir_all(&rules).unwrap();
        fs::create_dir_all(scratch.join("tests")).unwrap();
        fs::write(
            scratch.join("DECISIONS.md"),
            "## 1. Desktop behavior\n## 59. Phone punctuation\n",
        )
        .unwrap();
        fs::write(rules.join("rules.tsv"), "D1\t1\tkeys\twant\t0\n").unwrap();
        fs::write(
            rules.join("decisions.tsv"),
            "D59\tnone\tMobile keyboard excluded from Sevi\n",
        )
        .unwrap();
        assert_eq!(
            audit(&scratch).unwrap(),
            Coverage {
                tested: 1,
                open: 0,
                excluded: 1
            }
        );
        fs::write(rules.join("rules.tsv"), "").unwrap();
        assert!(audit(&scratch).is_err());
        fs::write(
            scratch.join("tests/session.rs"),
            "fn desktop_regression() {}\n",
        )
        .unwrap();
        fs::write(rules.join("decisions.tsv"), "D1\ttest\ttests/session.rs::fn desktop_regression()\nD59\tnone\tMobile keyboard excluded from Sevi\n").unwrap();
        assert_eq!(audit(&scratch).unwrap().tested, 1);
        fs::write(scratch.join("tests/session.rs"), "fn renamed_test() {}\n").unwrap();
        assert!(audit(&scratch).is_err());
        fs::write(
            rules.join("decisions.tsv"),
            "D1\topen\tRequires native acceptance\nD59\tnone\tMobile keyboard excluded from Sevi\n",
        )
        .unwrap();
        assert_eq!(
            audit(&scratch).unwrap(),
            Coverage {
                tested: 0,
                open: 1,
                excluded: 1
            }
        );
    }
}

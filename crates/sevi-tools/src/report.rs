//! The original PR report: trusted API metadata, numeric-only untrusted log parsing.
use crate::Result;
use reqwest::{
    Method,
    blocking::Client,
    header::{ACCEPT, LOCATION, USER_AGENT},
    redirect::Policy,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};
const MARK: &str = "<!-- sevi-pr-report -->";
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    label: &'static str,
    value: f64,
    unit: String,
    decimals: usize,
}
fn number(text: &str, pattern: &str, group: usize) -> Option<f64> {
    regex::Regex::new(pattern)
        .ok()?
        .captures(text)?
        .get(group)?
        .as_str()
        .parse()
        .ok()
}
pub fn metrics(logs: &BTreeMap<String, String>) -> Vec<Metric> {
    let c = logs.get("check-log").map(String::as_str).unwrap_or("");
    let i = logs.get("ios-log").map(String::as_str).unwrap_or("");
    let mut out = vec![];
    let mut add = |label, value: Option<f64>, unit: String, decimals| {
        if let Some(value) = value {
            out.push(Metric {
                label,
                value,
                unit,
                decimals,
            })
        }
    };
    for (label, pat) in [
        ("Rule cases passing", r"(?m)^rules: (\d+)/(\d+) pass"),
        (
            "Reported bugs fixed (rules/reported.tsv)",
            r"(?m)^reported bugs fixed: (\d+)/(\d+)",
        ),
    ] {
        if let Some(total) = number(c, pat, 2).filter(|n| *n != 0.) {
            add(label, number(c, pat, 1), format!("/{}", total as u64), 0)
        }
    }
    for (label, pattern) in [
        (
            "Manual switches, chat corpus",
            r"(?m)^metrics chat: manual switches (?:\d+ -> )?(\d+)",
        ),
        (
            "Wrong words, chat corpus",
            r"(?m)^metrics chat: .*wrong words (?:\d+ -> )?(\d+)",
        ),
        (
            "Manual switches, chat with typos",
            r"(?m)^metrics chat_typos: manual switches (?:\d+ -> )?(\d+)",
        ),
        (
            "Wrong words, chat with typos",
            r"(?m)^metrics chat_typos: .*wrong words (?:\d+ -> )?(\d+)",
        ),
        (
            "Manual switches, synthetic corpus",
            r"(?m)^metrics synthetic: manual switches (?:\d+ -> )?(\d+)",
        ),
        (
            "Wrong words, synthetic corpus",
            r"(?m)^metrics synthetic: .*wrong words (?:\d+ -> )?(\d+)",
        ),
    ] {
        add(label, number(c, pattern, 1), String::new(), 0)
    }
    if let Ok(pattern) = regex::Regex::new(
        r"(?m)^(?:FAIL )?realistic typing:? \w* ?\([^:]*: (\d+) broken; needed fixing by hand: ([^)]*)\)",
    ) && let Some(row) = pattern.captures(c)
    {
        add(
            "Realistic typing, text broken",
            row[1].parse().ok(),
            String::new(),
            0,
        );
        let count = regex::Regex::new(r"(\d+)/\d+")
            .unwrap()
            .captures_iter(&row[2])
            .filter_map(|c| c[1].parse::<f64>().ok())
            .sum();
        add(
            "Realistic typing, fixes needed by hand",
            Some(count),
            String::new(),
            0,
        );
    } else {
        let safety = regex::Regex::new(r"(?m)^realistic typing: \d+ messages, \d+ words \| guard (\d+)  elsewhere (\d+)  stuck \d+").unwrap();
        let rows = safety.captures_iter(c).collect::<Vec<_>>();
        if !rows.is_empty() {
            let broken = rows
                .iter()
                .flat_map(|r| [&r[1], &r[2]])
                .filter_map(|s| s.parse::<f64>().ok())
                .sum();
            add(
                "Realistic typing, text broken",
                Some(broken),
                String::new(),
                0,
            );
            let slips =
                regex::Regex::new(r"(?m)^  slips that needed fixing by hand:([^\n]*)").unwrap();
            let counts = regex::Regex::new(r"(\d+)/\d+").unwrap();
            let fixes = slips
                .captures_iter(c)
                .map(|r| {
                    counts
                        .captures_iter(&r[1])
                        .filter_map(|s| s[1].parse::<f64>().ok())
                        .sum::<f64>()
                })
                .sum();
            add(
                "Realistic typing, fixes needed by hand",
                Some(fixes),
                String::new(),
                0,
            );
        }
    }
    for (label, pat) in [
        (
            "Fat-finger, wrong words",
            r"(?m)^fat-finger: \w+ \(wrong words [\d.]+% -> ([\d.]+)%",
        ),
        (
            "Auto-correct, clean typing wrong",
            r"(?m)^auto-correct: \w+ \(clean typing ([\d.]+)% wrong",
        ),
        (
            "Auto-correct, sloppy typing wrong",
            r"(?m)^auto-correct: .*sloppy [\d.]+% -> ([\d.]+)%",
        ),
    ] {
        add(label, number(c, pat, 1), "%".into(), 2)
    }
    if !c.contains("fat-finger:") {
        let p=regex::Regex::new(r"(?m)^noise ([\d.]+).*\| words \d+: nearest [\d.]+% wrong, model ([\d.]+)% wrong[^\n]*\npick: [^\n]*\nauto-correct: ([\d.]+)%").unwrap();
        let mut clean = None;
        let mut sloppy = None;
        for row in p.captures_iter(c) {
            if row[1].parse::<f64>().unwrap_or_default() == 0. {
                clean = row[3].parse().ok();
            } else {
                sloppy = Some((row[2].parse::<f64>().ok(), row[3].parse::<f64>().ok()));
            }
        }
        if let Some((model, corrected)) = sloppy {
            add("Fat-finger, wrong words", model, "%".into(), 2);
            add("Auto-correct, clean typing wrong", clean, "%".into(), 2);
            add(
                "Auto-correct, sloppy typing wrong",
                corrected,
                "%".into(),
                2,
            );
        }
    }
    let tests = regex::Regex::new(
        r"Executed (\d+) tests?, with (?:\d+ tests? skipped and )?(\d+) failures?",
    )
    .unwrap()
    .captures_iter(i)
    .filter_map(|c| Some((c[1].parse::<u64>().ok()?, c[2].parse::<u64>().ok()?)))
    .max();
    if let Some((total, fail)) = tests {
        add(
            "iOS tests failing",
            Some(fail as f64),
            format!("/{total}"),
            0,
        )
    }
    out
}
fn format(value: f64, d: usize) -> String {
    if d == 0 {
        format!("{}", value as i64)
    } else {
        format!("{value:.d$}")
    }
}
fn render(
    sha: &str,
    base_sha: Option<&str>,
    current: &BTreeMap<String, String>,
    base: &BTreeMap<String, String>,
) -> String {
    let current = metrics(current);
    let base: BTreeMap<_, _> = metrics(base).into_iter().map(|m| (m.label, m)).collect();
    let mut lines = vec![
        MARK.to_owned(),
        format!(
            "**Sevi engine metrics** · `{}` vs main{}",
            sha.chars().take(7).collect::<String>(),
            base_sha
                .map(|s| format!(" `{}`", s.chars().take(7).collect::<String>()))
                .unwrap_or_default()
        ),
        String::new(),
        "| Metric | This PR | main | Change |".into(),
        "|---|--:|--:|--:|".into(),
    ];
    for m in &current {
        let value = format(m.value, m.decimals);
        if let Some(b) = base.get(m.label) {
            let equal = m.value == b.value && m.unit == b.unit;
            let change = if equal {
                "=".into()
            } else if m.unit != b.unit {
                "**changed**".into()
            } else {
                format!(
                    "**{}{}**",
                    if m.value > b.value { "+" } else { "" },
                    format(m.value - b.value, m.decimals)
                )
            };
            let value = if equal {
                format!("{value}{}", m.unit)
            } else {
                format!("**{value}{}**", m.unit)
            };
            lines.push(format!(
                "| {} | {value} | {}{} | {change} |",
                m.label,
                format(b.value, m.decimals),
                b.unit
            ));
        } else {
            lines.push(format!("| {} | {value}{} | | |", m.label, m.unit));
        }
    }
    if current.is_empty() {
        lines.push(
            "| No metrics in this run's log (the engine checks may have failed early) | | | |"
                .into(),
        )
    }
    lines.join("\n")
}
struct Api {
    repo: String,
    token: String,
    client: Client,
    download_client: Client,
}
impl Api {
    fn request(&self, path: &str, method: Method, body: Option<Value>) -> Result<Value> {
        let url = if path.starts_with("http") {
            path.to_owned()
        } else {
            format!("https://api.github.com{path}")
        };
        let mut req = self
            .client
            .request(method, url)
            .bearer_auth(&self.token)
            .header(ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header(USER_AGENT, "Sevi PR report");
        if let Some(body) = body {
            req = req.json(&body)
        }
        let bytes = req.send()?.error_for_status()?.bytes()?;
        Ok(if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)?
        })
    }
    fn get(&self, path: &str) -> Result<Value> {
        self.request(path, Method::GET, None)
    }
    fn download(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .download_client
            .get(url)
            .bearer_auth(&self.token)
            .header(ACCEPT, "application/vnd.github+json")
            .header(USER_AGENT, "Sevi PR report")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()?;
        if [301, 302, 303, 307, 308].contains(&response.status().as_u16()) {
            let next = response
                .headers()
                .get(LOCATION)
                .ok_or("artifact redirect has no Location")?
                .to_str()?;
            return Ok(Client::new()
                .get(next)
                .send()?
                .error_for_status()?
                .bytes()?
                .to_vec());
        }
        Ok(response.error_for_status()?.bytes()?.to_vec())
    }
    fn logs(&self, run: &Value) -> Result<BTreeMap<String, String>> {
        let mut logs = BTreeMap::new();
        let data = self.get(&format!(
            "/repos/{}/actions/runs/{}/artifacts",
            self.repo, run["id"]
        ))?;
        for artifact in data["artifacts"].as_array().ok_or("missing artifacts")? {
            let name = artifact["name"].as_str().unwrap_or("");
            if ["check-log", "ios-log"].contains(&name) && artifact["expired"] == false {
                let bytes = self.download(
                    artifact["archive_download_url"]
                        .as_str()
                        .ok_or("missing artifact URL")?,
                )?;
                let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
                if zip.is_empty() {
                    return Err("empty log artifact".into());
                }
                let entry = zip.by_index(0)?;
                let mut bytes = vec![];
                entry.take(1_600_000).read_to_end(&mut bytes)?;
                logs.insert(
                    name.into(),
                    String::from_utf8_lossy(&bytes)
                        .chars()
                        .take(400_000)
                        .collect(),
                );
            }
        }
        Ok(logs)
    }
}
pub fn run() -> Result {
    let repo = std::env::var("REPO")?;
    let sha = std::env::var("SHA")?;
    let api = Api {
        repo: repo.clone(),
        token: std::env::var("GH_TOKEN")?,
        client: Client::new(),
        download_client: Client::builder().redirect(Policy::none()).build()?,
    };
    let mut prs = vec![];
    for page in 1..=3 {
        let list = api.get(&format!(
            "/repos/{repo}/pulls?state=open&per_page=100&page={page}"
        ))?;
        prs.extend(list.as_array().ok_or("invalid PR list")?.clone());
    }
    let Some(pr) = prs.iter().find(|p| p["head"]["sha"] == sha) else {
        println!("no open PR at {sha}");
        return Ok(());
    };
    let mut runs = vec![];
    for event in ["pull_request", "pull_request_review"] {
        let list = api.get(&format!(
            "/repos/{repo}/actions/runs?head_sha={sha}&event={event}&per_page=30"
        ))?;
        runs.extend(
            list["workflow_runs"]
                .as_array()
                .ok_or("missing workflow runs")?
                .iter()
                .filter(|r| {
                    r["name"] == "Engine rules"
                        && r["conclusion"] != "action_required"
                        && r["conclusion"] != "skipped"
                })
                .cloned(),
        );
    }
    let run = runs
        .iter()
        .max_by_key(|r| r["run_number"].as_u64().unwrap_or(0));
    let Some(run) =
        run.filter(|r| r["status"] == "completed" && r["conclusion"] != "action_required")
    else {
        println!("engine run not done");
        return Ok(());
    };
    let mains = api.get(&format!(
        "/repos/{repo}/actions/runs?branch=main&event=push&status=completed&per_page=20"
    ))?;
    let (mut base, mut base_sha) = (BTreeMap::new(), None);
    for run in mains["workflow_runs"]
        .as_array()
        .ok_or("missing main runs")?
        .iter()
        .filter(|r| r["name"] == "Engine rules")
    {
        base = api.logs(run)?;
        if !base.is_empty() {
            base_sha = run["head_sha"].as_str().map(str::to_owned);
            break;
        }
    }
    let text = render(&sha, base_sha.as_deref(), &api.logs(run)?, &base);
    if std::env::var_os("DRY").is_some_and(|s| !s.is_empty()) {
        println!("{text}");
        return Ok(());
    }
    let number = pr["number"].as_u64().ok_or("missing PR number")?;
    let mut mine = None;
    for page in 1..=3 {
        let comments = api.get(&format!(
            "/repos/{repo}/issues/{number}/comments?per_page=100&page={page}"
        ))?;
        for c in comments.as_array().ok_or("invalid comment list")? {
            if mine.is_none()
                && c["user"]["login"] == "github-actions[bot]"
                && c["body"].as_str().unwrap_or("").contains(MARK)
            {
                mine = c["id"].as_u64();
            }
        }
    }
    if let Some(id) = mine {
        api.request(
            &format!("/repos/{repo}/issues/comments/{id}"),
            Method::PATCH,
            Some(json!({"body":text})),
        )?;
        println!("updated # {number}");
    } else {
        api.request(
            &format!("/repos/{repo}/issues/{number}/comments"),
            Method::POST,
            Some(json!({"body":text})),
        )?;
        println!("posted # {number}");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_logs_only_contribute_expected_numbers() {
        let mut logs = BTreeMap::new();
        logs.insert("check-log".into(),"rules: 154/154 pass\nmetrics chat: manual switches 2 -> 1, wrong words 3 -> 2\n<script>bad</script>\nfat-finger: OK (wrong words 2.00% -> 1.25%\n".into());
        let table = render("abcdef123", None, &logs, &BTreeMap::new());
        assert!(table.contains("| Rule cases passing | 154/154 |"));
        assert!(table.contains("| Manual switches, chat corpus | 1 |"));
        assert!(table.contains("| Fat-finger, wrong words | 1.25% |"));
        assert!(!table.contains("<script>"));
    }
    #[test]
    fn rust_check_logs_keep_the_same_report_metrics() {
        let text = "rules: 154/154 pass\nreported bugs fixed: 19/27\nmetrics chat: manual switches 2, wrong words 3 [ok]\nmetrics chat_typos: manual switches 4, wrong words 5 [ok]\nmetrics synthetic: manual switches 6, wrong words 7 [ok]\nrealistic typing: 10 messages, 20 words | guard 0  elsewhere 0  stuck 1\n  slips that needed fixing by hand:  delete 2/3  click 1/4\nnoise 0.00 weight 1 sigma 1 sure 1 | keys 100: nearest 0% model 0% | words 10: nearest 0% wrong, model 0% wrong\npick: 100 ns per touch on average\nauto-correct: 0.03% wrong words after correction\nnoise 0.30 weight 1 sigma 1 sure 1 | keys 100: nearest 20% model 10% | words 10: nearest 20% wrong, model 10% wrong\npick: 100 ns per touch on average\nauto-correct: 5.74% wrong words after correction\n";
        let result = metrics(&BTreeMap::from([("check-log".into(), text.into())]));
        assert_eq!(result.len(), 13);
        assert_eq!(
            result
                .iter()
                .find(|m| m.label == "Realistic typing, fixes needed by hand")
                .unwrap()
                .value,
            3.
        );
        assert_eq!(
            result
                .iter()
                .find(|m| m.label == "Auto-correct, sloppy typing wrong")
                .unwrap()
                .value,
            5.74
        );
    }
}

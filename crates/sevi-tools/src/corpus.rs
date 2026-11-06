use crate::{Args, Result};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};
use unicode_normalization::UnicodeNormalization;
fn source(a: &Args<'_>) -> std::path::PathBuf {
    a.path("--source", a.root.join("Tools/autoswitch/sim/corpus"))
}
pub fn clean(a: &Args<'_>) -> Result {
    let source = source(a);
    let mut files = fs::read_dir(source.join("viblo"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|p| p.extension().is_some_and(|s| s == "md") && p.is_file());
    files.sort();
    let replacements = [
        (r"(?s)```.*?```", " "),
        (r"`[^`]*`", " "),
        (r"!\[[^\]]*\]\([^)]*\)", " "),
        (r"\[([^\]]*)\]\([^)]*\)", "$1"),
        (r"https?://\S+", " "),
        (r"<[^>]+>", " "),
    ]
    .into_iter()
    .map(|(p, r)| Ok((regex::Regex::new(p)?, r)))
    .collect::<Result<Vec<_>>>()?;
    let prefix = regex::Regex::new(r"^[#*\-+\d.)\s]+")?;
    let marks = regex::Regex::new(r"[*_~]")?;
    let vi = regex::Regex::new(
        r"(?i)[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]",
    )?;
    let allowed = regex::Regex::new(r"[^\p{L}\p{N}_\s,.;:?!'()/-]")?;
    let spaces = regex::Regex::new(r"\s+")?;
    let mut out = vec![];
    for path in files {
        let mut text = fs::read_to_string(path)?;
        for (p, r) in &replacements {
            text = p.replace_all(&text, *r).into_owned();
        }
        for line in text.split('\n').map(str::trim) {
            if line.is_empty() || line.starts_with(['|', '>']) {
                continue;
            }
            let line = prefix.replace(line, "");
            let line = marks.replace_all(&line, "").nfc().collect::<String>();
            if !vi.is_match(&line) {
                continue;
            }
            let line = allowed.replace_all(&line, " ");
            let line = spaces.replace_all(&line, " ").trim().to_owned();
            let mut start = 0;
            let mut previous = '\0';
            for (i, c) in line
                .char_indices()
                .chain(std::iter::once((line.len(), '\0')))
            {
                if c == '\0' || (c == ' ' && ".?!".contains(previous)) {
                    let sentence = &line[start..i];
                    let count = sentence.split_whitespace().count();
                    if (4..=60).contains(&count) {
                        out.push(sentence.to_owned());
                    }
                    start = i + usize::from(c == ' ');
                }
                previous = c;
            }
        }
    }
    let words = out
        .iter()
        .map(|s| s.split_whitespace().count())
        .sum::<usize>();
    fs::write(
        a.path("--output", source.join("viblo_sentences.txt")),
        out.join("\n"),
    )?;
    println!("{} sentences {words} words", out.len());
    Ok(())
}
fn get(client: &Client, url: &str) -> Result<Value> {
    Ok(client
        .get(url)
        .header("Accept", "application/json")
        .header("User-Agent", "Mozilla/5.0 (research; Sevi typing test)")
        .send()?
        .error_for_status()?
        .json()?)
}
pub fn fetch(a: &Args<'_>) -> Result {
    let source = source(a);
    let base = a
        .option("--endpoint")
        .unwrap_or("https://viblo.asia")
        .trim_end_matches('/');
    let client = Client::builder().timeout(Duration::from_secs(20)).build()?;
    let mut slugs = vec![];
    for page in 1..=15 {
        let data = get(
            &client,
            &format!("{base}/api/posts/newest?limit=20&page={page}"),
        )?;
        for post in data["data"]
            .as_array()
            .ok_or("Viblo response has no data")?
        {
            slugs.push(
                post["slug"]
                    .as_str()
                    .ok_or("missing Viblo slug")?
                    .to_owned(),
            );
        }
        std::thread::sleep(Duration::from_millis(800));
    }
    fs::create_dir_all(source.join("viblo"))?;
    let mut n = 0;
    for slug in &slugs {
        if Path::new(slug).file_name().and_then(|s| s.to_str()) != Some(slug) {
            return Err("invalid Viblo filename".into());
        }
        let path = source.join("viblo").join(format!("{slug}.md"));
        if path.exists() {
            continue;
        }
        match get(&client, &format!("{base}/api/p/{slug}")) {
            Ok(data) => {
                let title = data["title"].as_str().unwrap_or("");
                let contents = data["contents"].as_str().unwrap_or("");
                fs::write(path, format!("{title}\n\n{contents}"))?;
                n += 1;
            }
            Err(e) => println!("err {slug} {e}"),
        }
        std::thread::sleep(Duration::from_millis(800));
    }
    println!("fetched {n} of {}", slugs.len());
    Ok(())
}
pub fn chat(a: &Args<'_>) -> Result {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let key = if let Some(path) = a.option("--key-file") {
        fs::read_to_string(path)?
    } else if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        key
    } else {
        fs::read_to_string(
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".secrets/anthropic_api_key"),
        )?
    };
    let key = key.trim().rsplit('=').next().unwrap_or("");
    let endpoint = a
        .option("--endpoint")
        .unwrap_or("https://api.anthropic.com/v1/messages");
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;
    let next = AtomicUsize::new(0);
    let results = Mutex::new(
        (0..44)
            .map(|_| None)
            .collect::<Vec<Option<std::result::Result<String, String>>>>(),
    );
    std::thread::scope(|scope| {
        for _ in 0..6 {
            let client = &client;
            let next = &next;
            let results = &results;
            scope.spawn(move||loop{let i=next.fetch_add(1,Ordering::Relaxed);if i>=44{break}let prompt=format!("Write 45 realistic chat messages (Slack/Messenger/Zalo style) about: {}. Written by Vietnamese people the way they actually type: Vietnamese with full diacritics, naturally mixing in English words, tech terms and sometimes whole English phrases or sentences. About 1 in 8 messages fully in English. Casual, varied length (3 to 40 words). No emoji, no names prefix, no numbering, no quotes. One message per line, nothing else.",TOPICS[i%TOPICS.len()]);let result=(||->Result<String>{let data:Value=client.post(endpoint).header("x-api-key",key).header("anthropic-version","2023-06-01").json(&json!({"model":"claude-haiku-4-5","max_tokens":4000,"messages":[{"role":"user","content":prompt}]})).send()?.error_for_status()?.json()?;Ok(data["content"][0]["text"].as_str().ok_or("missing generated chat text")?.to_owned())})();results.lock().unwrap()[i]=Some(result.map_err(|e|e.to_string()));});
        }
    });
    let mut lines = vec![];
    for result in results.into_inner().unwrap() {
        let text = result
            .ok_or("missing generation result")?
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        lines.extend(
            text.lines()
                .map(str::trim)
                .filter(|l| l.split_whitespace().count() >= 3)
                .map(str::to_owned),
        );
    }
    let words = lines
        .iter()
        .map(|l| l.split_whitespace().count())
        .sum::<usize>();
    fs::write(
        a.path("--output", source(a).join("chat_messages.txt")),
        lines.join("\n"),
    )?;
    println!("{} messages {words} words", lines.len());
    Ok(())
}
const TOPICS: [&str; 22] = [
    "dev team standup, bugs, deploy, PR review",
    "startup product meeting, KPI, roadmap",
    "friends planning a trip, booking hotel",
    "office HR, leave request, meeting room",
    "ecommerce shop owner talking to customers",
    "marketing team, ads campaign, content",
    "gaming friends, rank, match",
    "family group chat",
    "university students, deadline, assignment",
    "designers, Figma, feedback",
    "crypto and stock investors chatting",
    "customer support for a SaaS app",
    "data team, dashboard, SQL query",
    "freelancer talking to a client",
    "couple chatting casually",
    "devops, server down, incident",
    "mobile app team, TestFlight, App Store review",
    "AI tools, ChatGPT, prompt",
    "food delivery and restaurants",
    "football fans",
    "remote work, Zoom, Slack",
    "real estate agent and buyer",
];

use crate::{Args, Result};
const HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<rss version=\"2.0\" xmlns:sparkle=\"http://www.andymatuschak.org/xml-namespaces/sparkle\" xmlns:sevi=\"https://vikey.app/xmlns\">\n<channel>\n<title>Sevi</title>\n<link>https://vikey.app</link>\n";
pub fn update(old: &str, args: &[String], date: &str) -> Result<String> {
    let kind = args.first().ok_or("appcast requires platform")?;
    let mac = kind == "mac";
    let needed = if mac { 6 } else { 7 };
    if args.len() < needed {
        return Err("appcast: missing version/build/architecture/url/signature/notes".into());
    }
    let ver = &args[1];
    let build = &args[2];
    let (arch, start) = if mac { ("", 3) } else { (args[3].as_str(), 4) };
    let url = &args[start];
    let sig = &args[start + 1];
    let notes = &args[start + 2];
    let extra = if mac {
        "<sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>\n"
    } else {
        ""
    };
    let title = if mac {
        format!("Sevi {ver}")
    } else {
        format!("Sevi {ver} for Windows ({arch})")
    };
    let enclosure = if mac {
        format!("<enclosure url=\"{url}\" type=\"application/octet-stream\" {sig}/>")
    } else {
        format!(
            "<enclosure url=\"{url}\" sparkle:os=\"windows\" sevi:arch=\"{arch}\" type=\"application/octet-stream\" {sig}/>"
        )
    };
    let item = format!(
        "<item>\n<title>{title}</title>\n<pubDate>{date}</pubDate>\n<sparkle:version>{build}</sparkle:version>\n<sparkle:shortVersionString>{ver}</sparkle:shortVersionString>\n{extra}<description><![CDATA[{notes}]]></description>\n{enclosure}\n</item>\n"
    );
    let re = regex::Regex::new(r"(?s)<item>.*?</item>\n?")?;
    let keep: Vec<_> = re
        .find_iter(old)
        .map(|m| m.as_str())
        .filter(|s| {
            if mac {
                s.contains("sparkle:os=\"windows\"")
            } else {
                !(s.contains("sparkle:os=\"windows\"")
                    && s.contains(&format!("sevi:arch=\"{arch}\"")))
            }
        })
        .collect();
    let mut out = HEAD.to_owned();
    for i in keep
        .iter()
        .filter(|i| !i.contains("sparkle:os=\"windows\""))
    {
        out.push_str(i)
    }
    if mac {
        out.push_str(&item)
    }
    for i in keep.iter().filter(|i| i.contains("sparkle:os=\"windows\"")) {
        out.push_str(i)
    }
    if !mac {
        out.push_str(&item)
    }
    out.push_str("</channel>\n</rss>\n");
    Ok(out)
}
pub fn run(a: &Args<'_>) -> Result {
    let output = a.output("site/appcast.xml");
    let args: Vec<_> = a
        .args
        .iter()
        .take_while(|s| s.as_str() != "--output")
        .cloned()
        .collect();
    let old = std::fs::read_to_string(&output).unwrap_or_default();
    let out = update(
        &old,
        &args,
        &chrono::Utc::now()
            .format("%a, %d %b %Y %H:%M:%S +0000")
            .to_string(),
    )?;
    std::fs::write(output, out)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_one_architecture_preserving_other_items() {
        let mac = vec!["mac", "1.0", "1", "url", "length=\"1\"", "<p>Hi</p>"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let first = update("", &mac, "DATE").unwrap();
        let mut windows = vec![
            "windows",
            "1.0",
            "1",
            "x64",
            "url.exe",
            "length=\"2\"",
            "Hi",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let second = update(&first, &windows, "DATE").unwrap();
        windows[3] = "arm64".into();
        let third = update(&second, &windows, "DATE").unwrap();
        assert_eq!(third.matches("<item>").count(), 3);
        windows[1] = "1.1".into();
        let fourth = update(&third, &windows, "NEWDATE").unwrap();
        assert_eq!(fourth.matches("<item>").count(), 3);
        assert_eq!(fourth.matches("for Windows (arm64)").count(), 1);
        assert!(fourth.contains("Sevi 1.0</title>"));
    }
}

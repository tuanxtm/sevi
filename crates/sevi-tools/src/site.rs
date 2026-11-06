//! Static site generator. Layout strings and page content are the existing site's.
use crate::{Args, Result};
use chrono::Datelike;
use serde::Serialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, time::UNIX_EPOCH};
const SITE: &str = "https://vikey.app";
const GITHUB: &str = "https://github.com/tonyminibot/Vikey";
const PLATFORMS: [&str; 5] = ["macos", "windows", "linux", "ios", "android"];
#[derive(Serialize)]
struct Link {
    url: String,
}
#[derive(Serialize)]
struct Windows {
    url: String,
    arm: Option<String>,
}
#[derive(Serialize)]
struct Linux {
    url: String,
    rpm: Option<String>,
    pacman: Option<String>,
    deb_arm: Option<String>,
    rpm_arm: Option<String>,
    pacman_arm: Option<String>,
}
#[derive(Serialize)]
struct Downloads {
    macos: Option<Link>,
    windows: Option<Windows>,
    linux: Option<Linux>,
    ios: Option<Link>,
    android: Option<Link>,
}
fn url(v: &Value, key: &str) -> Option<String> {
    v[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}
fn downloads(rel: &Value) -> Downloads {
    let win = &rel["windows"];
    let lin = &rel["linux"];
    Downloads {
        macos: url(rel, "download_url").map(|url| Link { url }),
        windows: url(&win["x64"], "download_url").map(|url| Windows {
            url,
            arm: url_value(&win["arm64"]),
        }),
        linux: url(&lin["deb_x64"], "download_url").map(|url| Linux {
            url,
            rpm: url_value(&lin["rpm_x64"]),
            pacman: url_value(&lin["pacman_x64"]),
            deb_arm: url_value(&lin["deb_arm64"]),
            rpm_arm: url_value(&lin["rpm_arm64"]),
            pacman_arm: url_value(&lin["pacman_arm64"]),
        }),
        ios: url(&rel["ios"], "app_store_url").map(|url| Link { url }),
        android: url(&rel["android"], "download_url").map(|url| Link { url }),
    }
}
fn url_value(v: &Value) -> Option<String> {
    v["download_url"].as_str().map(str::to_owned)
}
fn template(source: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = source.to_owned();
    for (k, v) in values {
        out = out.replace(&format!("@@{k}@@"), v);
    }
    out
}
fn context(lang: &str, pre: &str, data: &Value) -> BTreeMap<String, String> {
    let mut v = BTreeMap::from([
        ("pre".into(), pre.into()),
        ("GITHUB".into(), GITHUB.into()),
        ("YEAR".into(), chrono::Local::now().year().to_string()),
    ]);
    for (k, value) in data["T"][lang].as_object().unwrap() {
        v.insert(format!("t.{k}"), value.as_str().unwrap().into());
    }
    v
}
fn nav(lang: &str, pre: &str, other: &str, data: &Value) -> String {
    let mut values = context(lang, pre, data);
    let t = &data["T"][lang];
    for i in 1..=2 {
        let flag = if lang == "vi" {
            include_str!("site/flag-gb.svg").replace("@@i@@", &i.to_string())
        } else {
            include_str!("site/flag-vn.svg").to_owned()
        };
        values.insert(
            format!("sw{i}"),
            format!(
                "<a class=\"lang\" href=\"{other}\" hreflang=\"{}\" title=\"{}\">{flag}{}</a>",
                if lang == "vi" { "en" } else { "vi" },
                t["switch_title"].as_str().unwrap(),
                t["switch"].as_str().unwrap()
            ),
        );
    }
    template(include_str!("site/nav.html"), &values)
}
fn os_icon(platform: &str, class: &str) -> String {
    format!(
        "<svg class=\"{class}\" aria-hidden=\"true\"><use href=\"/assets/os.svg#{platform}\"/></svg>"
    )
}
fn platform_row(lang: &str, pre: &str, dl: &Value, data: &Value) -> String {
    let mut out = "<div class=\"plats\">".to_owned();
    for p in PLATFORMS {
        let pending = dl[p].is_null();
        out.push_str(&format!(
            "<a href=\"{pre}/download#{p}\" title=\"{}{}\"{}>{}<span>{}</span></a>",
            data["PNAME"][p].as_str().unwrap(),
            if pending {
                format!(" · {}", data["PTXT"][lang]["soon"].as_str().unwrap())
            } else {
                String::new()
            },
            if pending { " class=pending" } else { "" },
            os_icon(p, "ic"),
            data["PNAME"][p].as_str().unwrap()
        ));
    }
    out.push_str("</div>");
    out
}
fn platform_cards(lang: &str, dl: &Value, data: &Value) -> String {
    let t = &data["PTXT"][lang];
    let s = |key: &str| t[key].as_str().unwrap_or("");
    let mut out = "<div class=\"pgrid\">".to_owned();
    for p in PLATFORMS {
        let d = &dl[p];
        let pending = d.is_null();
        let mut btn;
        let badge;
        if !pending {
            let u = d["url"].as_str().unwrap_or("");
            let store = p == "ios";
            btn = format!(
                "<a class=\"btn dl\" href=\"{u}\"{}>{}</a>",
                if store { "" } else { " download" },
                s(if store { "store" } else { "dl" })
            );
            if p == "linux" {
                btn = format!(
                    "<div class=\"two three\"><a class=\"btn dl\" href=\"{u}\" download>.deb</a>"
                );
                if let Some(rpm) = d["rpm"].as_str().filter(|s| !s.is_empty()) {
                    btn.push_str(&format!(
                        "<a class=\"btn dl\" href=\"{rpm}\" download>.rpm</a>"
                    ));
                }
                if let Some(pac) = d["pacman"].as_str().filter(|s| !s.is_empty()) {
                    btn.push_str(&format!("<a class=\"btn dl\" href=\"{pac}\" download title=\"Arch Linux package\">Arch</a>"));
                }
                btn.push_str("</div>");
                let arms = [
                    ("deb_arm", ".deb"),
                    ("rpm_arm", ".rpm"),
                    ("pacman_arm", "Arch"),
                ]
                .into_iter()
                .filter_map(|(k, n)| {
                    d[k].as_str()
                        .filter(|s| !s.is_empty())
                        .map(|url| format!("<a href=\"{url}\" download>{n}</a>"))
                })
                .collect::<Vec<_>>();
                if !arms.is_empty() {
                    btn.push_str(&format!(
                        "<span class=\"alt\">ARM64: {}</span>",
                        arms.join(" · ")
                    ));
                }
            }
            if p == "windows"
                && let Some(arm) = d["arm"].as_str().filter(|s| !s.is_empty())
            {
                btn.push_str(&format!(
                    "<a class=\"alt\" href=\"{arm}\" download>{}</a>",
                    s("arm")
                ));
            }
            if p == "android" {
                btn = format!(
                    "<div class=\"two\"><a class=\"btn dl\" href=\"{u}\" download>APK</a><a class=\"btn ghost soon\" href=\"#android\" data-notify=\"play\" title=\"{}\">{}<small>{}</small></a></div>",
                    s("notify"),
                    s("play"),
                    s("soon")
                );
            }
            badge = String::new();
            if !["windows", "linux"].contains(&p)
                || (url(d, "arm").is_none() && url(d, "deb_arm").is_none())
            {
                btn.push_str("<span class=\"alt none\" aria-hidden=\"true\">.</span>");
            }
        } else {
            btn = format!(
                "<a class=\"btn ghost dl\" href=\"#{p}\" data-notify=\"{p}\">{}</a><span class=\"alt none\" aria-hidden=\"true\">.</span>",
                s("notify")
            );
            badge = format!("<span class=\"badge\">{}</span>", s("soon"));
        }
        let name = data["PNAME"][p].as_str().unwrap();
        let shot = format!(
            "<img class=\"pshot zoom\" src=\"/assets/shots/dl-{p}.webp\" width=\"1200\" height=\"900\" alt=\"Sevi {} {name}\" loading=\"lazy\">",
            if lang == "vi" { "trên" } else { "on" }
        );
        out.push_str(&format!("<section class=\"pcard{}\" id=\"{p}\" data-os=\"{p}\">{shot}{badge}{}<h2>{name}</h2><p>{}</p>{btn}</section>",if pending{" pending"}else{""},os_icon(p,"picon"),data["PREQ"][lang][p].as_str().unwrap()));
    }
    out.push_str("</div>");
    out
}
fn copy_tree(source: &Path, dest: &Path) -> Result {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing source symlink {}", source.display()).into());
    }
    if metadata.is_dir() {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        fs::copy(source, dest)?;
    }
    Ok(())
}
fn mtime(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)?
        .as_secs())
}
fn ascii_json(value: &impl Serialize) -> Result<String> {
    let text = serde_json::to_string(value)?;
    let mut out = String::new();
    for c in text.chars() {
        if c >= '\u{7f}' {
            let mut buf = [0; 2];
            for unit in c.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        } else {
            out.push(c)
        }
    }
    Ok(out)
}
pub fn run(a: &Args<'_>) -> Result {
    let source = a.path("--source", a.root.join("site")).canonicalize()?;
    let output = a.path("--output", source.join("dist"));
    if output == source || source.starts_with(&output) || output == a.root {
        return Err("site output must be a generated directory".into());
    }
    if fs::symlink_metadata(&output).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("refusing site output symlink".into());
    }
    let stage = a.root.join("target/site-build").join(format!(
        "{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir_all(&stage)?;
    copy_tree(&source.join("assets"), &stage.join("assets"))?;
    if source.join("downloads").is_dir() {
        copy_tree(&source.join("downloads"), &stage.join("downloads"))?;
    }
    fs::copy(source.join("style.css"), stage.join("style.css"))?;
    let rel = match fs::read(source.join("release.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.into()),
    };
    if rel.as_object().is_some_and(|m| !m.is_empty()) {
        fs::copy(source.join("release.json"), stage.join("release.json"))?;
    }
    if source.join("appcast.xml").is_file() {
        fs::copy(source.join("appcast.xml"), stage.join("appcast.xml"))?;
    }
    let data: Value = serde_json::from_str(include_str!("site/translations.json"))?;
    let dl = downloads(&rel);
    let dl_value = serde_json::to_value(&dl)?;
    let dl_json = ascii_json(&dl)?;
    let ver = mtime(&source.join("style.css"))?.to_string();
    let meta_re = regex::Regex::new(r"<!--\s*(\w+):\s*(.*?)\s*-->")?;
    let body_re = regex::Regex::new(r"(?s)^(<!--.*?-->\s*)+")?;
    let cache = regex::Regex::new(r#"(/assets/[\w./-]+\.(?:png|json|svg|jpg|js))""#)?;
    let mut site_pages = vec![];
    for (lang, dirname, pre) in [("vi", "pages", ""), ("en", "pages_en", "/en")] {
        let dir = source.join(dirname);
        let mut files = fs::read_dir(&dir)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        files.sort();
        for name in files {
            let name = name.to_string_lossy();
            let Some(slug) = name.strip_suffix(".html") else {
                continue;
            };
            if lang == "vi" && slug != "404" {
                site_pages.push(slug.to_owned());
            }
            let src = fs::read_to_string(dir.join(name.as_ref()))?;
            let head = src.split('\n').take(6).collect::<Vec<_>>().join("\n");
            let meta: BTreeMap<_, _> = meta_re
                .captures_iter(&head)
                .map(|c| (c[1].to_owned(), c[2].to_owned()))
                .collect();
            let get =
                |key: &str, default: &str| meta.get(key).cloned().unwrap_or_else(|| default.into());
            let body = body_re.replace(&src, "");
            let rel_path = if slug == "index" {
                "/".into()
            } else {
                format!("/{slug}")
            };
            let path = format!("{pre}{rel_path}");
            let en_path = format!("/en{rel_path}");
            let canonical = if !get("noindex", "").is_empty() {
                "<meta name=\"robots\" content=\"noindex\">".into()
            } else {
                format!("<link rel=\"canonical\" href=\"{SITE}{path}\">")
            };
            let alternates = format!(
                "<link rel=\"alternate\" hreflang=\"vi\" href=\"{SITE}{rel_path}\"><link rel=\"alternate\" hreflang=\"en\" href=\"{SITE}{en_path}\"><link rel=\"alternate\" hreflang=\"x-default\" href=\"{SITE}{rel_path}\">"
            );
            let one = get("layout", "") == "one";
            let mut page = include_str!("site/head.html").to_owned();
            for (k, v) in [
                ("lang", lang.to_owned()),
                ("title", get("title", "Sevi")),
                ("description", get("description", "")),
                ("site", SITE.into()),
                ("path", path),
                ("ver", ver.clone()),
                ("canonical", canonical),
                ("alternates", alternates),
                (
                    "bodyclass",
                    if one {
                        " class=\"one-page\"".into()
                    } else {
                        String::new()
                    },
                ),
            ] {
                page = page.replace(&format!("{{{k}}}"), &v);
            }
            page.push_str(&nav(
                lang,
                pre,
                if lang == "vi" { &en_path } else { &rel_path },
                &data,
            ));
            let body = body
                .replace(
                    "<!-- PLATFORMS -->",
                    &platform_cards(lang, &dl_value, &data),
                )
                .replace(
                    "<!-- PLATFORM_ROW -->",
                    &platform_row(lang, pre, &dl_value, &data),
                );
            page.push_str(&body);
            if !one {
                page.push_str(&template(
                    include_str!("site/footer.html"),
                    &context(lang, pre, &data),
                ));
            }
            page.push_str(&format!("<script>window.VK_DL={dl_json}</script><script src=\"/assets/platforms.js\" defer></script></body></html>\n"));
            for (platform, pattern) in [
                ("windows", r"(?s)<!-- windows -->.*?<!-- /windows -->"),
                ("linux", r"(?s)<!-- linux -->.*?<!-- /linux -->"),
            ] {
                if rel[platform].is_null()
                    || rel[platform].as_object().is_some_and(|m| m.is_empty())
                {
                    page = regex::Regex::new(pattern)?
                        .replace_all(&page, "")
                        .into_owned();
                }
            }
            let mut subst = vec![
                ("VERSION", &rel["version"]),
                ("LIN_VERSION", &rel["linux"]["version"]),
                ("DOWNLOAD_URL", &rel["download_url"]),
                ("SHA256", &rel["sha256"]),
                ("WIN_VERSION", &rel["windows"]["version"]),
            ];
            for (key, arch, field) in [
                ("WIN_X64_URL", "x64", "download_url"),
                ("WIN_ARM64_URL", "arm64", "download_url"),
                ("WIN_X64_ZIP", "x64", "portable_url"),
                ("WIN_ARM64_ZIP", "arm64", "portable_url"),
                ("WIN_X64_SHA", "x64", "sha256"),
                ("WIN_ARM64_SHA", "arm64", "sha256"),
            ] {
                subst.push((key, &rel["windows"][arch][field]));
            }
            for (key, arch) in [
                ("LIN_DEB_X64", "deb_x64"),
                ("LIN_DEB_ARM64", "deb_arm64"),
                ("LIN_RPM_X64", "rpm_x64"),
                ("LIN_RPM_ARM64", "rpm_arm64"),
            ] {
                subst.push((key, &rel["linux"][arch]["download_url"]));
            }
            subst.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
            for (key, value) in subst {
                if let Some(v) = value.as_str().filter(|s| !s.is_empty()) {
                    page = page.replace(key, v);
                }
            }
            page = cache
                .replace_all(&page, |m: &regex::Captures<'_>| {
                    let path = source.join(m[1].trim_start_matches('/'));
                    if let Ok(canon) = path.canonicalize()
                        && canon.starts_with(&source)
                        && let Ok(time) = mtime(&canon)
                    {
                        format!("{}?v={time}\"", &m[1])
                    } else {
                        m[0].to_owned()
                    }
                })
                .into_owned();
            let outdir = if pre.is_empty() {
                stage.clone()
            } else {
                stage.join("en")
            };
            fs::create_dir_all(&outdir)?;
            fs::write(outdir.join(name.as_ref()), page)?;
        }
    }
    fs::write(
        stage.join("_redirects"),
        "/features /#features 301\n/en/features /en/#features 301\n",
    )?;
    fs::write(
        stage.join("_headers"),
        "/*\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n/downloads/*\n  Content-Disposition: attachment\n",
    )?;
    fs::write(
        stage.join("robots.txt"),
        format!("User-agent: *\nAllow: /\nSitemap: {SITE}/sitemap.xml\n"),
    )?;
    site_pages.sort();
    let mut urls = String::new();
    for pre in ["", "/en"] {
        for p in &site_pages {
            urls.push_str(&format!(
                "<url><loc>{SITE}{pre}{}</loc></url>",
                if p == "index" {
                    "/".into()
                } else {
                    format!("/{p}")
                }
            ));
        }
    }
    fs::write(
        stage.join("sitemap.xml"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{urls}</urlset>\n"
        ),
    )?;
    let entries = fs::read_dir(&stage)?.count();
    if output.exists() {
        let archive = a.root.join("target/site-previous").join(format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        fs::create_dir_all(archive.parent().unwrap())?;
        fs::rename(&output, archive)?;
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(stage, &output)?;
    println!("built {entries} entries into site/dist");
    Ok(())
}

//! Existing Sparkle Windows enclosure format and Ed25519 file signatures.
use ed25519_dalek::{Signature, VerifyingKey};
use sevi_runtime::prefs::decode_binary;
use std::cmp::Ordering;
pub const PUBLIC_KEY: &str = include_str!("../../../updates/public-key.txt");
pub const DEFAULT_URL: &str =
    "https://github.com/tuanxtm/sevi/releases/latest/download/appcast.xml";

pub fn update_url(prefs: &dyn sevi_runtime::prefs::Preferences) -> String {
    let configured = prefs.string("AppcastURL");
    if configured.trim().is_empty() {
        DEFAULT_URL.into()
    } else {
        configured
    }
}
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub signature: String,
    pub notes: String,
    pub length: u64,
}
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| {
        s.split('.')
            .map(|part| {
                part.bytes().filter(u8::is_ascii_digit).fold(0u64, |n, c| {
                    n.saturating_mul(10).saturating_add((c - b'0') as u64)
                })
            })
            .collect::<Vec<_>>()
    };
    let (mut a, mut b) = (parse(a), parse(b));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a.cmp(&b)
}
fn attribute(tag: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    tag.split_once(&key)
        .and_then(|(_, s)| s.split_once('"'))
        .map(|(s, _)| s.to_owned())
        .unwrap_or_default()
}
fn element<'a>(xml: &'a str, name: &str) -> &'a str {
    let start = format!("<{name}>");
    let end = format!("</{name}>");
    xml.split_once(&start)
        .and_then(|(_, s)| s.split_once(&end))
        .map(|(s, _)| s)
        .unwrap_or("")
}
fn notes(html: &str) -> String {
    let html = html
        .split_once("<![CDATA[")
        .map(|(_, s)| s.split_once("]]>").map(|(s, _)| s).unwrap_or(s))
        .unwrap_or(html);
    let mut out = String::new();
    let mut inside = false;
    for (i, c) in html.char_indices() {
        if c == '<' {
            inside = true;
            if html[i..].starts_with("<li>") {
                out.push_str("\n• ")
            }
        } else if c == '>' {
            inside = false
        } else if !inside && c != '\n' && c != '\r' {
            out.push(c)
        }
    }
    out.trim().to_owned()
}
pub fn newest(xml: &str, architecture: &str) -> Option<Release> {
    let mut best: Option<Release> = None;
    for part in xml.split("<item>").skip(1) {
        let Some((item, _)) = part.split_once("</item>") else {
            continue;
        };
        for part in item.split("<enclosure").skip(1) {
            let Some((tag, _)) = part.split_once('>') else {
                continue;
            };
            if attribute(tag, "sparkle:os") != "windows"
                || attribute(tag, "sevi:arch") != architecture
            {
                continue;
            }
            let version = element(item, "sparkle:shortVersionString");
            let version = if version.is_empty() {
                element(item, "sparkle:version")
            } else {
                version
            };
            let release = Release {
                version: version.into(),
                url: attribute(tag, "url"),
                signature: attribute(tag, "sparkle:edSignature"),
                length: attribute(tag, "length").parse().unwrap_or(0),
                notes: notes(element(item, "description")),
            };
            if best
                .as_ref()
                .is_none_or(|old| compare_versions(&release.version, &old.version).is_gt())
            {
                best = Some(release)
            }
        }
    }
    best
}
fn verify_key(file: &[u8], signature: &[u8], key: &[u8]) -> bool {
    let Ok(key) = <&[u8; 32]>::try_from(key) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(key) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(signature) else {
        return false;
    };
    key.verify_strict(file, &signature).is_ok()
}
pub fn verify(file: &[u8], signature: &str) -> bool {
    verify_key(file, &decode_binary(signature), &decode_binary(PUBLIC_KEY))
}
#[cfg(test)]
mod tests {
    use super::*;
    use sevi_runtime::prefs::{MemoryPreferences, Preferences};
    #[test]
    fn sevi_feed_is_the_default_and_explicit_overrides_are_preserved() {
        let mut prefs = MemoryPreferences::default();
        assert_eq!(update_url(&prefs), DEFAULT_URL);
        prefs.set("AppcastURL", "  ".into()).unwrap();
        assert_eq!(update_url(&prefs), DEFAULT_URL);
        prefs
            .set("AppcastURL", "https://example.com/custom.xml".into())
            .unwrap();
        assert_eq!(update_url(&prefs), "https://example.com/custom.xml");
    }
    #[test]
    fn feed_keeps_newest_matching_architecture() {
        let xml = r#"<rss><item><sparkle:shortVersionString>1.0.9</sparkle:shortVersionString><enclosure sparkle:os="windows" sevi:arch="x64" url="old"/></item><item><sparkle:shortVersionString>1.0.14</sparkle:shortVersionString><description><![CDATA[<ul><li>Gõ tốt hơn</li></ul>]]></description><enclosure sparkle:os="windows" sevi:arch="arm64" url="arm"/><enclosure sparkle:os="windows" sevi:arch="x64" url="new" length="123" sparkle:edSignature="abc"/></item><item><sparkle:version>99</sparkle:version><enclosure sparkle:os="macos" sevi:arch="x64" url="mac"/></item></rss>"#;
        let release = newest(xml, "x64").unwrap();
        assert_eq!(
            (
                release.version.as_str(),
                release.url.as_str(),
                release.length
            ),
            ("1.0.14", "new", 123)
        );
        assert_eq!(release.notes, "• Gõ tốt hơn");
        assert_eq!(newest(xml, "arm64").unwrap().url, "arm");
        assert_eq!(compare_versions("1.0.14", "1.0.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Equal);
    }
    #[test]
    fn signatures_are_checked_over_the_entire_file() {
        fn hex(s: &str) -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect()
        }
        let pk = hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let sig = hex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        );
        assert!(verify_key(b"", &sig, &pk));
        assert!(!verify_key(b"changed", &sig, &pk));
        assert!(!verify_key(b"", &sig[..63], &pk));
        assert_eq!(decode_binary(PUBLIC_KEY).len(), 32);
    }
}

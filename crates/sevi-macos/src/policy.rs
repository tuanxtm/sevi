//! Constants and decisions from the original macOS host.
pub const EXCLUDED: &[&str] = &[
    "com.microsoft.rdc.macos",
    "com.microsoft.rdc.mac",
    "com.apple.ScreenSharing",
    "com.apple.RemoteDesktop",
    "com.teamviewer.TeamViewer",
    "com.anydesk.anydesk",
    "com.carriez.RustDesk",
    "tv.parsec.www",
    "com.realvnc.vncviewer",
    "com.citrix.receiver.nomas",
    "com.vmware.horizon",
    "com.p5sys.jump.mac.viewer",
    "com.moonlight-stream.Moonlight",
];
pub const QWERTY: &[&str] = &[
    "com.apple.keylayout.US",
    "com.apple.keylayout.ABC",
    "com.apple.keylayout.USInternational-PC",
    "com.apple.keylayout.British",
    "com.apple.keylayout.British-PC",
    "com.apple.keylayout.Australian",
    "com.apple.keylayout.Canadian",
    "com.apple.keylayout.Irish",
    "com.apple.keylayout.USExtended",
];
pub fn browser_script(bundle: &str) -> Option<String> {
    let tab = match bundle {
        "com.google.Chrome"
        | "com.google.Chrome.beta"
        | "com.google.Chrome.canary"
        | "com.brave.Browser"
        | "com.microsoft.edgemac"
        | "company.thebrowser.Browser"
        | "com.vivaldi.Vivaldi"
        | "org.chromium.Chromium"
        | "com.operasoftware.Opera" => "active tab",
        "com.apple.Safari" | "com.apple.SafariTechnologyPreview" => "current tab",
        _ => return None,
    };
    Some(format!(
        "tell application id \"{bundle}\" to if (count of windows) > 0 then return URL of {tab} of front window"
    ))
}
pub fn delete_grapheme(text: &mut String) {
    use unicode_segmentation::UnicodeSegmentation;
    if let Some((start, _)) = text.grapheme_indices(true).next_back() {
        text.truncate(start);
    }
}
pub fn layout_key(character: &str, fallback: u32) -> u32 {
    let lower = character.to_lowercase();
    let mut chars = lower.chars();
    let Some(c) = chars.next() else {
        return fallback;
    };
    if chars.next().is_some() {
        return fallback;
    }
    if c == ' ' || c == '\0' {
        return fallback;
    }
    // The original layout bridge maps the logical key to a canonical QWERTY key.
    for key in 0..=50 {
        for caps in [0, sevi_engine::keys::CAPS_MASK] {
            if sevi_engine::data::key_character(key | caps) == c as u32 {
                return key;
            }
        }
    }
    fallback
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rewrite {
    Prefix(char),
    Selection,
}
pub fn spotlight_window(owner: &str, layer: i32, status_level: i32) -> bool {
    owner == "Spotlight" && layer != status_level
}
pub fn rewrite(app: &str, spotlight_visible: bool) -> Rewrite {
    if spotlight_visible || app == "com.apple.Spotlight" {
        Rewrite::Selection
    } else if ["com.sublimetext.2", "com.sublimetext.3"].contains(&app) {
        Rewrite::Prefix('\u{200c}')
    } else {
        Rewrite::Prefix('\u{202f}')
    }
}
pub fn hotkey_label(h: u32) -> String {
    let mut s = String::new();
    for (bit, c) in [(0x100, '⌃'), (0x200, '⌥'), (0x800, '⇧'), (0x400, '⌘')] {
        if h & bit != 0 {
            s.push(c)
        }
    }
    let key = h & 255;
    s.push_str(match key {
        0xfe => "",
        49 => "Space",
        36 => "↩",
        48 => "⇥",
        53 => "⎋",
        51 => "⌫",
        _ => "",
    });
    if ![0xfe, 49, 36, 48, 53, 51].contains(&key) {
        if let Some(c) = char::from_u32(sevi_engine::data::key_character(key))
            .filter(|c| *c != '\0' && key <= 50 && key != 10)
        {
            s.extend(c.to_uppercase())
        } else if let Some(n) = [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111]
            .iter()
            .position(|k| *k == key)
        {
            s.push_str(&format!("F{}", n + 1))
        } else {
            s.push_str(&format!("#{key}"));
        }
    }
    if s.is_empty() { "—".into() } else { s }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_policies() {
        assert!(!spotlight_window("Spotlight", 25, 25));
        assert!(spotlight_window("Spotlight", 0, 25));
        assert!(!spotlight_window("Finder", 0, 25));
        assert!(browser_script("org.mozilla.firefox").is_none());
        assert!(
            browser_script("com.apple.Safari")
                .unwrap()
                .contains("current tab")
        );
        assert!(
            browser_script("com.google.Chrome")
                .unwrap()
                .contains("active tab")
        );
        assert_eq!(
            rewrite("com.sublimetext.3", false),
            Rewrite::Prefix('\u{200c}')
        );
        assert_eq!(rewrite("com.google.Chrome", true), Rewrite::Selection);
        assert_eq!(layout_key("A", 255), 0);
        assert_eq!(layout_key("é", 255), 255);
        assert_eq!(hotkey_label(0x7a000206), "⌥Z");
        assert_eq!(hotkey_label(18), "1");
        assert_eq!(hotkey_label(123), "#123");
        assert_eq!(layout_key("ab", 255), 255);
        assert_eq!(EXCLUDED.len(), 13);
        let mut query = "ngựa👨‍👩‍👧‍👦".to_owned();
        delete_grapheme(&mut query);
        assert_eq!(query, "ngựa");
    }
}

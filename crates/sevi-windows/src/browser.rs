//! The existing address-bar heuristic; this intentionally does not reinterpret search text.
pub fn is_browser(exe: &str) -> bool {
    [
        "chrome.exe",
        "msedge.exe",
        "brave.exe",
        "vivaldi.exe",
        "opera.exe",
        "opera_gx.exe",
        "firefox.exe",
        "chromium.exe",
        "arc.exe",
    ]
    .contains(&exe.to_lowercase().as_str())
}
pub fn looks_like_url(value: &str) -> bool {
    if value.is_empty() || value.encode_utf16().count() > 2048 {
        return false;
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return true;
    }
    if value.contains(' ') {
        return false;
    }
    value
        .find('.')
        .is_some_and(|i| i > 0 && i + 1 < value.len())
}
pub fn host_of(value: &str) -> String {
    if value.starts_with("file:") {
        return String::new();
    }
    let s = value.split_once("://").map(|(_, s)| s).unwrap_or(value);
    let s = s
        .split(['/', '?', '#', ':'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let s = s.strip_prefix("www.").unwrap_or(&s);
    if !s.contains('.') && s != "localhost" {
        String::new()
    } else {
        s.to_owned()
    }
}
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
pub const PLAIN: u64 = 0;
pub const PASSWORD: u64 = 1;
pub const UNKNOWN: u64 = 2;
pub const ASSUMED_PLAIN: u64 = 3;
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct FocusKey {
    pub window: isize,
    pub object: i32,
    pub child: i32,
}
#[derive(Default)]
struct FocusData {
    event: FocusKey,
    ambiguous: Vec<FocusKey>,
}
/// Keep slow UI Automation answers from overwriting a newer focus event.
pub struct FocusGuard {
    state: AtomicU64,
    data: Mutex<FocusData>,
}
impl Default for FocusGuard {
    fn default() -> Self {
        Self {
            state: AtomicU64::new(ASSUMED_PLAIN),
            data: Mutex::default(),
        }
    }
}
impl FocusGuard {
    pub fn secure(&self) -> bool {
        matches!(self.seen() & 3, PASSWORD | UNKNOWN)
    }
    pub fn seen(&self) -> u64 {
        self.state.load(Ordering::SeqCst)
    }
    pub fn event(&self, key: FocusKey) {
        let mut data = self.data.lock().unwrap();
        let same = key == data.event;
        data.event = key;
        let ambiguous = data.ambiguous.contains(&key);
        drop(data);
        let mut state = self.seen();
        if same && state & 3 == PLAIN && !ambiguous {
            return;
        }
        loop {
            let next = ((state & !3).wrapping_add(4)) | UNKNOWN;
            match self
                .state
                .compare_exchange_weak(state, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(new) => state = new,
            }
        }
    }
    pub fn key(&self) -> FocusKey {
        self.data.lock().unwrap().event
    }
    pub fn publish(&self, seen: u64, state: u64) {
        let _ = self.state.compare_exchange(
            seen,
            (seen & !3) | state,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }
    pub fn ambiguous(&self, seen: u64, key: FocusKey) {
        if (self.seen() ^ seen) & !3 != 0 {
            return;
        }
        let mut data = self.data.lock().unwrap();
        if data.ambiguous.len() >= 64 {
            data.ambiguous.clear()
        }
        if !data.ambiguous.contains(&key) {
            data.ambiguous.push(key)
        }
    }
}
#[cfg(test)]
mod focus_tests {
    use super::*;
    #[test]
    fn stale_password_checks_cannot_override_new_focus() {
        let g = FocusGuard::default();
        let first = FocusKey {
            window: 1,
            object: 2,
            child: 3,
        };
        g.event(first);
        let old = g.seen();
        assert!(g.secure());
        g.event(FocusKey { window: 2, ..first });
        g.publish(old, PLAIN);
        assert!(g.secure());
        let current = g.seen();
        g.publish(current, PASSWORD);
        assert!(g.secure());
        g.event(first);
        g.publish(g.seen(), PLAIN);
        assert!(!g.secure());
        let before = g.seen();
        g.event(first);
        assert_eq!(g.seen(), before);
        g.ambiguous(before, first);
        g.event(first);
        assert!(g.secure());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_address_bar_policy() {
        assert!(is_browser("Chrome.EXE"));
        assert!(!is_browser("editor.exe"));
        for url in [
            "https://www.Example.com/path?q=1",
            "www.example.com:443/path",
            "example.com",
        ] {
            assert!(looks_like_url(url));
            assert_eq!(host_of(url), "example.com");
        }
        for value in [
            "file:///C:/vk/x.html",
            "C:/vk/x.html",
            "edge",
            "search terms",
        ] {
            assert_eq!(host_of(value), "");
        }
        assert_eq!(host_of("http://localhost:8080/"), "localhost");
        assert!(!looks_like_url("find example.com"));
    }
}
